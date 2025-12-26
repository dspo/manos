# 从前端调用 Rust（对标 Tauri v2）路线图与计划

本文档对照 Tauri v2 文档「从前端调用 Rust」，梳理 `gpui-manos-webview` 当前能力差距，并给出一个务实的补齐路线图与实施计划。

参考文档：
- https://v2.tauri.app/zh-cn/develop/calling-rust/
- https://v2.tauri.app/develop/calling-rust/

说明：
- `https://v2.tauri.org.cn/develop/calling-rust/` 在本地环境 TLS 握手失败，因此本对照以 `v2.tauri.app` 的同内容页面为准。

## 范围与目标

**目标**
- 在不实现完整 `tauri-runtime` 的前提下，尽可能让“从前端调用 Rust”的开发体验与行为接近 Tauri v2（包含文档中的命令与事件相关能力）。

**范围**
- ✅ 命令（invoke）：参数、返回值、错误处理、异步、通道（Channel）、原始请求访问。
- ✅ 事件系统：全局事件 / webview 事件 / listen-unlisten（文档涉及的最小集）。
- ✅ IPC 传输层：让上述能力在 `wry` 环境下可用且稳定。
- ❌ 不做：完整 Tauri 插件生态、capability/ACL/权限模型、tauri.conf 配置体系、updater、tray、menu 等（不属于本文档主题且工程量巨大）。

## 当前实现快照（我们现在有什么）

Rust 侧（核心入口）
- `#[gpui_manos_webview::command]`：同步/异步命令宏（async 通过 `pollster::block_on` 执行，`crates/webview-macros/src/lib.rs`）。
- `gpui_manos_webview::generate_handler![...]`：多命令路由（`crates/webview-macros/src/lib.rs`）。
- `Builder::invoke_handler(...)`：注册 invoke handler（`crates/webview/src/lib.rs`）。
- `ipc` 模块：`Request`/`Response`/`Channel` 与带 `Tauri-Response` 的 HTTP 响应构造（`crates/webview/src/lib.rs`）。

JS 侧（注入脚本）
- `window.__TAURI_INTERNALS__.invoke`：核心 invoke（`crates/webview/src/scripts/tauri/core.js`）。
- `window.__TAURI_INTERNALS__.ipc`：pattern/brownfield 调用入口（`crates/webview/src/scripts/tauri/ipc.js`）。
- custom-protocol fetch：通过 `ipc://` 发起请求（`crates/webview/src/scripts/tauri/ipc-protocol.js`）。

当前关键限制
- async runtime 仅为 `pollster::block_on`（不是完整 Tokio runtime）；且 postMessage fallback 仍在 IPC handler 线程内执行命令。
- `ipc::Response::binary(...)` 可在 custom-protocol 路径返回 `ArrayBuffer`；但 postMessage fallback 目前仍会回传 `number[]`（与 Tauri 行为不完全一致）。
- `ipc::Channel<T>` 已提供最小可用实现（JSON 消息 + drop 时发送 end）；但缺少 `__TAURI_CHANNEL__|fetch` 大 payload 快路径与二进制/分片优化。
- 命令函数仍无法注入 `WebviewWindow` / `AppHandle` / `State<T>` 等完整上下文（Tauri 文档支持）。
- 事件系统（Rust 侧 listen/emit）基本未实现，仅有 JS 分发函数骨架（`crates/webview/src/lib.rs`）。

## 能力差距对照（按文档结构）

### 命令（Commands / invoke）

| 文档能力 | Tauri v2 | 当前实现 | 差距与影响 | 建议里程碑 |
| --- | --- | --- | --- | --- |
| 基础示例 | `#[command]` + `invoke()` | ✅ | 基本可用 | M0/M1 |
| 传递参数 | `Deserialize` + camelCase 映射 | ⚠️ | 支持 camelCase/`rename_all`；不支持借用参数（如 `&str`）等高级签名 | M1 |
| 返回数据 | `Serialize` -> Promise resolve | ✅ | 返回 JSON ok | M1 |
| 返回 ArrayBuffer | `tauri::ipc::Response` | ⚠️ | `ipc::Response::binary` 在 custom-protocol 路径可返回 `ArrayBuffer`；fallback 仍会变成 `number[]` | M3 |
| 错误处理 | `Result<T, E: Serialize>`（结构化） | ⚠️ | 默认 `E: ToString` 会 reject 为 JSON string；支持 `#[command(error = "json")]` 返回结构化 JSON | M1/M3 |
| 异步命令 | `async fn` / `#[command(async)]` | ⚠️ | 已支持 `async fn`（`pollster::block_on`）；仍缺少完整 runtime 与更一致的线程模型 | M2（困难） |
| 通道（Channel） | `tauri::ipc::Channel<T>` 流式传输 | ⚠️ | 已支持 `ipc::Channel<T>` 发送 JSON 消息；缺少大 payload/binary 优化与 `__TAURI_CHANNEL__` fast-path | M4（困难） |
| 访问 WebviewWindow | 参数注入 `WebviewWindow` | ❌ | 命令里拿不到发起方上下文（label/webview id 等） | M5（困难） |
| 访问 AppHandle | 参数注入 `AppHandle` | ❌ | 无法在命令内访问全局应用服务（事件/状态/窗口管理等） | M5（困难） |
| 访问托管状态 | `Builder::manage` + `State<T>` | ❌ | 无统一状态容器 & 注入机制 | M5（困难） |
| 访问原始请求 | `tauri::ipc::Request`（headers + body） | ✅ | 支持命令参数注入 `gpui_manos_webview::ipc::Request`（method/uri/headers/body） | M3 |
| 创建多个命令 | `generate_handler![a,b]` | ✅ | 已支持 | M0 |

### 事件系统（Events）

| 文档能力 | Tauri v2 | 当前实现 | 差距与影响 | 建议里程碑 |
| --- | --- | --- | --- | --- |
| 全局事件 emit/listen | `@tauri-apps/api/event` | ❌ | 需要 Rust 侧事件总线 + JS API 适配 | M6（困难） |
| Webview 事件 emitTo/listen | `WebviewWindow` 定向事件 | ❌ | 缺少 webview label/路由与隔离策略 | M6（困难） |
| 监听/取消监听 | `listen` 返回 `unlisten()` | ❌ | 生命周期管理缺失 | M6（困难） |

## 路线图（Milestones）

下面里程碑是建议拆分；可以按需要合并/拆开。每个里程碑都包含：交付物、验收点、风险。

### M0：传输层与兼容性打底（1–3 天）

交付物
- 让 invoke 的“请求-响应”链路在更多场景稳定工作（方法、CORS、fallback）。
- 明确并统一协议/URL 方案（至少对齐 `ipc://` 与静态资源 scheme 的可用性）。

建议任务
- [x] IPC custom protocol：支持 `OPTIONS` 预检，并限制只允许 `POST/OPTIONS`（对齐 Tauri 行为）。
- [x] `Tauri-Response`/`Access-Control-Expose-Headers` 等 header 行为对齐（目前部分已实现，但需要补全一致性）。
- [x] postMessage fallback：当 custom protocol fetch 失败时，提供可用的 `window.ipc.postMessage` 路径（当前缺失）。
- [x] 协议对齐：当前 `convertFileSrc()` 默认 `asset://`，但静态资源注册的是 `wry://`；需要统一（至少避免前端调用走到不存在的 scheme）。

验收点（建议）
- `window.__TAURI_INTERNALS__.invoke("cmd")` 在 custom-protocol 可用与不可用两种情况下都能返回。

风险/说明
- postMessage fallback 依赖 wry 的 `with_ipc_handler`；当前已启用并实现最小可用链路（解析消息 -> 调用 handler -> `eval` 回调）。

### M1：同步命令能力补齐与“可维护性”提升（3–7 天）

交付物
- 同步命令在参数/返回/错误上更接近 Tauri 文档的“开发感受”。

建议任务
- [x] 命令名唯一性与冲突提示：`generate_handler!` 在编译期检测重复命令名并报错。
- [x] 参数解析增强：检查 `Content-Type`（非 JSON / 二进制 payload 给出明确错误），并在反序列化失败时带上命令名。
- [x] 错误返回结构化（可选）：`#[command(error = "json")]` 时，`Result<T, E>` 的 `E: Serialize` 将以 `application/json` 返回；默认仍返回 JSON string（来自 `ToString`）。

验收点（建议）
- JS 侧 `.catch(e => ...)` 能拿到稳定结构的错误对象（或至少稳定字符串）。

风险/说明
- 结构化错误会影响已有用户前端处理逻辑，需要设计兼容策略（例如保留 string，新增 opt-in）。

### M2：异步命令（高价值，但实现难度较高）（1–2 周）

目标
- 支持 `async fn` 命令，或支持 `#[command(async)]` 将同步命令 offload 到后台执行，避免 UI 卡顿。

当前状态（已落地第一版）
- ✅ `#[command]` 支持 `async fn`（通过 `gpui_manos_webview::async_runtime::block_on` 执行）。
- ✅ `ipc://` custom-protocol 路径的命令执行会 offload 到后台线程，并在完成后再 `respond(...)`，避免阻塞处理线程。
- ⚠️ `block_on` 不是完整的 async runtime（当前基于 `pollster`）；如果命令依赖 Tokio（如 `tokio::time`/IO），仍需要后续引入 runtime（见下方方案 A）。
- ⚠️ postMessage fallback 仍在 IPC handler 线程内执行命令（仅在 custom protocol 被阻断时才会走到）。

建议实现路径（择一或组合）
- 方案 A（可控但侵入）：新增可选 feature 引入 `tokio`（或其它 executor），在 IPC handler 里 `spawn` 并延后 `responder.respond(...)`。
- 方案 B（更贴 GPUI）：复用 GPUI 的任务/线程池能力（如果存在稳定 API），在后台执行并回到主线程响应。

验收点（建议）
- 执行 100ms–1s 级别耗时任务，UI 不明显卡顿，invoke 可正确 resolve/reject。

难点/风险（为什么“困难”）
- 当前命令宏将函数包成“立即返回 http::Response”的同步 wrapper；要支持 async，需要宏生成异步 wrapper 或改变调用栈设计。
- 需要明确“异步运行时”的选择，否则会引入全局依赖与线程模型复杂度。

### M3：二进制响应（Response）与原始请求访问（1 周）

目标
- 对齐文档中“返回 ArrayBuffer”的能力，并为上传/headers 校验等场景提供原始 request 访问。

建议任务
- [x] 定义 `gpui_manos_webview::ipc::Response` 来表达“原始字节响应 + content-type”。
- [x] 允许命令返回该 Response（以及 `Result<Response, E>`），在 `ipc://` custom-protocol 路径下前端可走 `arrayBuffer()` 分支。
- [x] 引入 `gpui_manos_webview::ipc::Request`（method/uri/headers/body），并支持作为命令参数注入（用于读取 headers 与 raw body bytes）。

验收点（建议）
- `read_file() -> Response` 前端能拿到 `ArrayBuffer`，且不会被 JSON 序列化成巨大数组。
- `upload(request: Request)` 能读取 `Authorization` header 与 raw body bytes。

难点/风险
- postMessage fallback 目前仍会把二进制结果回传为 `number[]`（`Vec<u8>` 的 JSON 序列化），与 custom-protocol 的 `ArrayBuffer` 不完全一致。

### M4：Channel（流式传输）（高价值，但复杂）（1–2 周）

目标
- 对齐文档的 `tauri::ipc::Channel<T>`：Rust 可多次发送消息，前端持续接收（用于下载/大文件/进度）。

建议任务
- [x] 最小实现：支持命令参数注入 `ipc::Channel<T>`，可多次 `send(T)`，并在 drop 时发送 `{ end: true }`。
- [x] 多 webview 兼容：通过 `Invoke.webview_label` + `ipc::IpcContextGuard` 贯通上下文，确保 Channel 发送到正确 webview。
- [ ] 大 payload / 二进制优化：实现 `plugin:__TAURI_CHANNEL__|fetch`（Tauri 的 fast-path），避免通过 `eval` 直接塞入超大 JSON/bytes。

难点/风险（为什么“困难”）
- 这不仅是“返回一个响应”，而是要维护跨多次消息的状态与回调映射。
- wry 不同平台对 “回调/JS eval/消息通道” 的能力差异会带来大量边界情况。

### M5：上下文注入（WebviewWindow/AppHandle/State）——“类 Tauri”体验核心，但工程大（2–4 周）

目标
- 命令签名支持注入“调用上下文”，接近 Tauri：`WebviewWindow` / `AppHandle` / `State<T>`。

建议折中（更现实）
- 先实现最小上下文：`CommandContext { webview_id, webview_label?, window_label?, headers, origin }`。
- `State<T>` 先实现只读访问（`Arc<T>`），避免可变借用与并发复杂度。

难点/风险（为什么“困难”）
- 我们不具备 tauri 的 manager/label/窗口-多 webview 管理体系；要 1:1 复刻 `AppHandle`/`WebviewWindow` 会逼近“实现一个 tauri-runtime/tauri manager”。
- 泛型 state 注入需要全局类型映射/Any 容器，并定义线程安全边界（Send/Sync）。

### M6：事件系统（全局事件 + webview 定向事件）（2–4 周）

目标
- 对齐文档的事件 API：`listen`, `emit`, `emitTo`, `unlisten`，并支持全局与指定 webview。

建议任务
- [ ] Rust 侧实现事件总线（name -> listeners），并暴露 IPC 命令供 JS 侧调用。
- [ ] JS 侧实现 `@tauri-apps/api/event` 的最小子集（或提供兼容层）。
- [ ] 与 M5 的上下文/label 体系打通（否则无法 `emitTo`）。

难点/风险（为什么“困难”）
- 事件系统天然涉及生命周期管理（webview 关闭、热重载、重复注册、内存泄漏）。
- 若要兼容 `@tauri-apps/api/event`，需要严格对齐 payload 与 target 语义。

## 实施计划（建议顺序）

建议按“先把地基打牢，再逐步抬高能力”的顺序推进：

1) M0：IPC 打底（custom protocol + OPTIONS + fallback + scheme 对齐）
2) M1：同步命令体验增强（更好的错误/参数处理）
3) M3：二进制响应 + 原始请求对象（能覆盖大量真实业务：读文件/上传）
4) M2：异步命令（引入 executor 选择，并在宏与 handler 上打通）
5) M4：Channel（若确有流式需求再做）
6) M5：上下文注入 + state（根据业务需要逐步扩展）
7) M6：事件系统（最后做，因为依赖 label/上下文，且维护成本高）

## 明确列出：可能“做不了/性价比极低”的事项（及原因）

> 即使暂时放弃，也建议记录在案，避免反复讨论。

- 完整兼容 Tauri 的 `AppHandle/WebviewWindow`：
  - 原因：这些类型背后依赖 tauri 的窗口/webview 管理、插件系统、资源路由等；在 gpui+ wry 的架构中强行对齐会逼近“重做 Tauri manager/运行时”。
  - 折中：提供我们自己的 `CommandContext` 与少量必要 API。
- 完整兼容 `@tauri-apps/api` 全量模块：
  - 原因：`@tauri-apps/api` 背后是大量 plugin 命令与协议约定（window/webview/fs/http/...）。仅为“调用 Rust”场景不值得全量复刻。
  - 折中：优先保证 `core.invoke` +（可选）`event` 子集；其他模块按需实现。
- Isolation pattern / 安全沙箱：
  - 原因：涉及新协议、iframe 隔离、加密、CSP 与平台差异；远超“调用 Rust”的最小范围。

## 验收与回归测试建议

建议每个里程碑至少提供以下验证：
- 最小 demo：1 个 sync 命令、1 个返回大二进制的命令、（可选）1 个 async 命令。
- 失败用例：参数缺失/类型错误/命令不存在/内部错误，前端能稳定处理。
- 性能用例：10MB+ 文件读取（M3），确认不会 JSON 化导致内存暴涨。
- 兼容用例：custom-protocol 被阻断时（模拟 CSP 或禁用 scheme），fallback 路径仍可用（M0）。

## 需要提前拍板的决策点

- 是否引入 `tokio`（或其它 executor）来支持异步命令与 Channel？（影响依赖、二进制大小、线程模型）
- 最终静态资源 scheme 选型：统一成 `asset://` / `wry://` / `tauri://` 之一？（影响前端工具链与 `convertFileSrc` 行为）
- 对错误返回是否引入结构化 envelope？是否默认开启还是 opt-in？
