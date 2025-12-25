# RichText：插件系统与未来架构计划（激进重构版）

本文基于对本仓库 `gpui-manos-plate`（`crates/rich_text`）的实现细读，给出一份**面向未来与最佳实践**的重构计划：直接对齐 Plate/Slate 体系常见的设计（树模型、operations、normalize、可组合插件），不再规划 v1 过渡态。

相关背景与当前实现边界见：
- [RichText Editor：扩展性与重构方向（面向未来）](richtext-extensibility.md)
- 工具层 UI 组件（Toolbar）参考：[Plate Toolbar Buttons（GPUI 组件实现教程）](plate-toolbar-buttons.md)

---

## 1. 现状复盘（以代码为准）

### 1.1 文档模型：扁平 blocks + text leaf

- 文档：`RichTextDocument { blocks: Vec<BlockNode> }`（按“行”拆成 blocks）。
- 块：`BlockNode { format: BlockFormat, inlines: Vec<InlineNode> }`。
- inline 目前只有 `InlineNode::Text(TextNode { text, style })`，样式为 `InlineStyle`（bold/italic/underline/strikethrough/code/fg/bg/link）。
- 位置/选区：`Selection` 使用 UTF-8 byte offset（`start/end`），并用 Rope 进行 offset ↔ row/col、UTF-16 转换（IME 相关）。

对应文件：
- `crates/rich_text/src/document.rs`
- `crates/rich_text/src/style.rs`
- `crates/rich_text/src/selection.rs`
- `crates/rich_text/src/rope_ext.rs`

### 1.2 内核：RichTextState 集中承载命令、输入、undo、IME

`RichTextState` 是编辑器事实上的“内核”：
- KeyBinding 注册与 Action handler（backspace/enter/undo/redo/toggle_* 等）：`crates/rich_text/src/state.rs`
- 内容编辑：`replace_bytes_range`、按行增删 block、inline split/merge、`BlockNode::normalize`
- 块级能力：list/quote/todo/toggle/divider/columns/indent/align/size/link 等均为 state 方法
- undo/redo：快照只包含 `{document, selection, active_style}`（未覆盖所有“编辑结果相关状态”）
- IME：`EntityInputHandler` 实现，入口为 `replace_text_in_range / replace_and_mark_text_in_range`

### 1.3 渲染：element.rs 里按 BlockKind 特判 + 分组

渲染采取“按行渲染”的策略：
- `RichTextState` 实现 `Render`，遍历 `document.blocks` 输出 gpui element tree
- `RichTextLineElement` 负责一行文本 runs（inline 样式、链接下划线、code 字体），并绘制 selection/caret
- 复杂块特判：`BlockKind` 决定列表前缀、todo checkbox、toggle chevron、divider、quote 容器化、columns 分组 + resize handle
- toggle collapsed 通过“隐藏缩进栈”实现对子块的跳过渲染（属于“分组/布局规则”）

对应文件：
- `crates/rich_text/src/element.rs`

### 1.4 序列化：Slate 兼容 JSON 的互转层

`RichTextValue` 实现版本化包装与 Slate 风格节点（`SlateElement.kind` 字符串）互转：
- `RichTextValue::from_document`：将 blocks/inlines 转为 Slate 结构
- `RichTextValue::into_document`：将 Slate 结构解析回 blocks/inlines
- 对未知 `element.kind` 的处理：会降级到 Paragraph（信息丢失风险）

对应文件：
- `crates/rich_text/src/value.rs`

### 1.5 UI 工具：story 示例硬编码调用 state 方法

toolbar、dialog 等目前在 `crates/story/src/richtext.rs` 中直接调用 `RichTextState` 的各种方法，尚未形成“工具插件贡献”机制。

---

## 2. 改造的必要性（为什么要做插件系统）

### 2.1 现有结构的扩展成本会随功能线性甚至超线性增长

新增一个能力（例如：新块、新输入规则、新序列化字段、新渲染容器）通常会同时涉及：
- `state.rs`：编辑命令/输入行为/undo 一致性
- `element.rs`：渲染与交互 hit-test
- `value.rs`：读写格式与版本兼容
- `story`：UI 工具接入

随着能力增多，`match BlockKind` 与“特例规则”会持续堆叠，代码维护成本快速上升。

### 2.2 undo/redo 与“状态归属”边界需要被系统化

现在 undo 快照不包含 columns 宽度等状态；未来插件更多时，“哪些状态应可撤销/可持久化/仅 UI 会话态”必须有统一规则，否则会出现大量一致性 bug。插件系统的三层分治能把这条线一次性画清楚。

---

## 3. 可行性与现实边界（能做到多像 Plate.js）

### 3.1 可行性：你们已有插件系统所需的三大“骨架”

- 内核命令集合（`RichTextState` 方法）
- 渲染管线（`Render for RichTextState` + per-line element）
- 序列化层（Slate-compatible 的互转）

插件化主要是把这些能力抽成“可注册、可组合、可覆写”的扩展点，并将内置功能迁移为“内置插件集合”。

### 3.2 边界：gpui Action/KeyBinding 的静态类型特征

gpui 的 Action 是静态类型体系：运行期动态加载“未知插件并新增 Action 类型”的模式不现实。

因此建议定位为：
- **编译期可插拔**（features/构建时选择插件集合）
- **运行期可配置**（同一构建产物内启用/禁用某些插件、调整优先级）

### 3.3 我们的选择：直接上“树模型 + operations + normalize + 插件”

由于项目处于早期且无历史负担，我们建议直接采用面向未来的编辑器内核形态：
- **树模型**：`Node = Element | Text | Void`，Element 持有 children；不再以“按行 block”作为一等结构。
- **Path-based selection**：selection 以 `{anchor: Point { path, offset }, focus: ... }` 表达，避免把整棵树压扁为 `\n` 文本来计算光标。
- **Operation / Transaction**：所有编辑都被表达为 operations（insert/remove/split/merge/set_node/set_mark…），并通过 transaction 应用；undo/redo 记录 operations（或反向操作）。
- **Normalize 管线**：插件声明 schema/约束与 normalize passes；任何 transaction 后由 normalize 收敛到合法结构。
- **无损序列化**：未知节点/属性必须 round-trip，保证“未启用插件也不丢数据”。

---

## 4. 目标内核：数据结构与编辑语义（未来架构的地基）

这一节不是“中间态”，而是你们未来编辑器最核心的公共语言；插件系统必须围绕它设计。

### 4.1 文档与节点类型（Schema 由插件贡献）

建议的最小数据模型（概念上）：
- `Document { children: Vec<Node> }`
- `Node`：
  - `Element { kind: ElementKind, attrs: Attrs, children: Vec<Node> }`
  - `Text { text: Rope/String, marks: Marks }`
  - `Void { kind: ElementKind, attrs: Attrs }`（图片、divider、embed、mention 等）
- `Attrs`：`BTreeMap<String, serde_json::Value>` 或内部更强类型的属性系统（插件可声明强类型并做校验）
- `Marks`：可用结构化位集 + map 承载（bold/italic/underline/strikethrough/code/color/bg/link…）

关键点：
- `kind` 与 `attrs` 使未知节点可保留（为插件体系与兼容性服务）
- 结构约束（哪些节点可作为 children、哪些是 inline/void）由插件 schema 决定，而不是硬编码在 enum match 里

### 4.2 位置与选区（Path-based）

建议：
- `Point { path: Vec<usize>, offset: usize }`（offset 指向 Text 节点内的字符位置）
- `Selection { anchor: Point, focus: Point }`
- 统一提供 `normalize_selection` 以应对结构变化（split/merge/remove 后修复 path）

### 4.3 Operations（插件可拦截/补全/normalize）

建议的 operation 集合（可按需裁剪）：
- `InsertText { path, offset, text }`
- `RemoveText { path, range }`
- `InsertNode { path, node }`
- `RemoveNode { path }`
- `SplitNode { path, position }`（split text 或 split element children）
- `MergeNode { path }`
- `SetNodeAttrs { path, patch }`
- `SetMark { range/selection, mark, value }`
- `MoveNode { from, to }`

交易模型：
- `Transaction { ops: Vec<Op>, selection_after: Option<Selection>, meta: TransactionMeta }`
- `Editor::apply(tx)` 负责：
  1) 验证/预处理（before hooks）
  2) 应用 ops（得到新 Document）
  3) normalize（循环直到稳定或达到迭代上限）
  4) 产出反向 ops 以支持 undo（或记录 redo 栈）

### 4.4 Normalize（插件声明约束 + 提供修复策略）

normalize 不应是“到处 if”，而应成为插件体系的一等公民：
- 插件可贡献：
  - schema 规则（例如 paragraph 只能包含 inline；table 只能包含 table_row；table_cell 至少有一个 paragraph…）
  - normalize pass（函数：扫描局部结构并生成修复 operations）
- normalize 的输出应仍然是 operations（以便可审计/可撤销/可测试）

---

## 5. 插件系统机制（对标 Plate 的“可组合、可覆写、可声明贡献”）

### 5.1（历史实现对照）统一命令系统：从“写死方法调用”到“命令注册表”

目标：新增 tool/快捷键/菜单项，不再需要改 `RichTextEditor` 的 `.on_action(...)` 列表，也不需要在 story 里硬编码调用 state 私有能力。

建议做法：
- 新增一个通用 Action：`RunCommand { id, args }`
- `RichTextState` 内维护 `CommandRegistry`：`id -> handler`
- 插件通过 `plugin.commands()` 注册命令（并声明 metadata：label/icon/enable_when/shortcut 等）
- toolbar/menu 只触发 `RunCommand(id)`；命令实现由插件提供

迁移策略：
- 保留现有 actions（Backspace/Enter/...）作为兼容层，内部逐步转发为 `dispatch_command("backspace")`

> 上述“迁移策略”描述的是从当前实现走向目标架构的**理解桥梁**；实际实施遵循本文第 7/8 节“同构验证 + 一次性替换”，不会长期保留双轨。

### 5.2 命令系统（Command Registry）

建议命令作为插件的主要对外 API：
- `CommandId`：稳定字符串（例如 `core.toggle_bold`、`list.toggle`、`table.insert_row`）
- `Command`：`fn run(&mut Editor, &mut CommandContext) -> CommandResult`
- UI（toolbar/menu/shortcut）只依赖命令 id 与 query API，不直接依赖具体插件实现

在 gpui 层的落地方式建议：
- 保留静态 Action 体系，但将按键绑定指向“通用 RunCommand”或“少量固定 Action + command id”
- 通过 `EditorView` 将 gpui input 事件转译为 command/transaction

### 5.2 Hook 管线（可组合的编辑语义扩展点）

推荐的 hook 集合（与 operations/transaction 对齐）：
- `before_apply(tx)`：改写/补全 tx（例如输入规则把 `- ` 转换为 list item）
- `after_apply(tx, result)`：派发事件/更新派生状态
- `decorate(doc, selection)`：产出 decorations（拼写、搜索、诊断）
- `normalize(doc)`：产出修复 ops（或参与 schema 校验）
- `serialize/deserialize`：对特定 kind 进行无损编解码

返回值语义建议：
- hooks 尽量返回“数据”（ops/patch/decorations），由内核统一合并与应用；避免 hook 直接 mutate editor，降低借用与顺序耦合复杂度。

### 5.3 渲染层插件化（Node Renderer + Layout/Container Rules）

在树模型下，渲染应从“按行 blocks 遍历”转为“按节点渲染 + 布局容器规则”：
- Node renderer：`render_element(kind, attrs, children)`、`render_text(text, marks)`
- layout/container 规则：如列表、引用、表格、columns、toggle 的结构本身应体现在节点树中（通过 schema + normalize 保证），而非渲染时临时扫描连续行。

建议渲染插件扩展点：
- `NodeRenderer`：first-match by kind（可 override）
- `InlineRenderer/MarkRenderer`：将 marks 映射到 gpui `StyledText` runs（或你们自定义绘制）
- `OverlayRenderer`：selection/caret/decoration overlay（与 hit-test/IME 对齐）

### 5.4 序列化插件化（无损 round-trip）

目标：
- 任何节点 `kind + attrs + children` 必须能无损落盘并读回
- 未启用某插件时，仍能保留未知节点（渲染可降级，但保存不能丢）

建议：
- 统一存储格式：versioned JSON（类似你们现有 `RichTextValue` 但改为通用 Node）
- Slate 兼容层作为“可选适配器插件”，而不是内核强绑定

### 5.5 工具层插件（Toolbar/Menu/Dialog 只依赖命令与 query）

工具层建议直接对接：
- `CommandRegistry`（触发命令）
- `Query API`（例如：当前 selection 是否在 bold、当前 block 类型、是否可执行某命令）

可复用 UI 组件可参考：
- [Plate Toolbar Buttons（GPUI 组件实现教程）](plate-toolbar-buttons.md)

---

## 6. 合并与冲突规则（必须先定死）

为保证插件可组合且可预测，建议提前确立：
- `PluginId` 与 `PluginPriority`：优先级越高越先执行（或越后执行），需全局一致
- 命令冲突：默认报错；只允许显式 override（并记录来源插件）
- renderer 冲突：允许 override（例如主题/自定义渲染替换默认渲染）
- schema 冲突：同一 kind 的 schema 只能由一个插件声明（否则报错）
- serialize/deserialize：按 kind first-match；未命中走通用无损节点编码
- normalize：多 pass 迭代直到稳定；设置上限避免死循环，并记录调试信息

---

## 7. 实施策略（无“过渡架构”，只允许同构验证）

你们不接受“中间态架构”，因此实施应遵循：
- **先在最终架构下实现最小闭环**（Document/Selection/Transaction/Renderer/Serialize + 2~3 个基础插件），作为技术验证
- **验证通过后一次性替换旧实现**：旧 `crates/rich_text` 的 monolith 不应与新内核长期并存
- **所有里程碑都应是最终架构的子集**：每一步都在同一套 Node/Op/Normalize/Plugin 体系上叠加能力

### Milestone A：内核最小闭环（最终架构的最小子集）

范围建议：
- Document/Node/Attrs/Marks + Selection
- Transaction/Op apply + undo/redo
- normalize pass 框架（即使规则很少也要走同一通道）
- JSON 存储格式（无损 round-trip）
- 2~3 个基础插件：
  - Paragraph/Text + 基础 marks（bold/italic/link 至少一个）
  - List（最能验证 schema+normalize 的价值）
  - 一个 void 节点（divider 或 image）验证 void 语义

验收标准：
- 文档结构不依赖“按行文本拼接”
- 所有编辑通过 operations 表达，undo/redo 可用
- 插件可以通过 schema/normalize 维持结构合法

### Milestone B：编辑体验与渲染能力补齐

范围建议：
- IME 适配（EntityInputHandler）与 hit-test：从 view 层把 gpui 输入映射到 selection/transaction
- selection/caret/decoration overlay 体系（用于未来拼写检查/搜索高亮）
- 工具层（toolbar）通过命令与 query API 工作

### Milestone C：面向未来的节点类型（表格/mention/图片/嵌套）

范围建议：
- table（row/cell 的 schema + normalize）
- mention 或 image（inline void）与其序列化
- 复杂 normalize 示例（例如 table cell 至少一个 paragraph）

### 一次性替换策略（建议）

- 在新内核闭环完成后：
  - 用新 crate/模块提供新的 public API（例如 `RichTextEditor`/`RichTextState` 的对外壳层，但内部是新 `EditorCore`）
  - 删除旧实现（或至少从 workspace default-members 里移除并标记废弃），避免双轨长期存在

---

## 8. 迭代计划拆解（每一步都“可实现 + 可验收”）

下面的迭代拆解遵循两个原则：
1) **每一步都产出一个“端到端可运行”的编辑器体验**（哪怕功能集不大，但闭环完整：输入/选择/撤销/渲染/保存加载/插件组合）。
2) **每一步都处于最终架构之内**（同构验证），不会引入未来要推翻的临时框架。

> 建议新内核以独立 crate 落地（例如 `crates/rich_text_next` 或 `crates/editor_core`），并提供一个新的示例入口（例如 `cargo run --example richtext_next`）用于验收；通过后再替换现有 `crates/rich_text` 的 public API 壳层。

本项目采用的落地位置与命名约定：
- 新内核 crate：`crates/plate-core`
- crate 名称：`gpui-plate-core`
- 建议示例入口：`cargo run --example richtext_next`（位于 `crates/story/examples/richtext_next.rs`）

### Iteration 1：新内核最小闭环（树模型 + ops + normalize + JSON + 渲染）

**要实现什么**
- 新 crate：`editor_core`（命名可调整），包含：
  - `Document/Node/Attrs/Marks`（树模型 + 未知节点可保留）
  - `Selection`（path-based）
  - `Op/Transaction`（apply + 反向 ops 或可逆记录）
  - `Normalize` 框架（pass 产出修复 ops，迭代至稳定/上限）
  - `Plugin` 框架（registry：schema/normalize/commands/serialize/renderer）
  - 统一 JSON 存储格式（versioned；无损 round-trip）
- 最小插件集合（必须走插件机制，不写死在 core）：
  - `core.paragraph`：Paragraph 作为默认 block
  - `core.text`：Text leaf + marks（至少支持 `bold` 与 `link` 之一）
  - `core.divider`：一个 void 节点（用于验证 void + normalize + 渲染）
- 最小渲染（先不追求与旧实现等价，但要完整闭环）：
  - 递归渲染 Node（Element/Text/Void）到 gpui elements（可先输出简单层级结构）
  - selection/caret 的最小可视化（能看见光标与选区即可）

**怎么实现（关键做法）**
- 将“结构合法性”全部下放到 schema + normalize（例如：Document.children 只能是 block；Paragraph.children 只能是 inline；Text 只能出现在 inline context；Divider 是 void 且必须是 block）。
- `Attrs`/`kind` 作为第一等字段：未知 kind 不报错、不丢弃，至少能 round-trip。
- 所有编辑都必须通过 `Transaction { ops }` 应用；禁止在 core 里提供“直接 mutate 树”的后门 API（避免绕过 normalize/undo）。

**要验收什么**
- 能运行一个新的示例：`crates/story/examples/richtext_next.rs`
- 运行方式：`cargo run -p gpui-manos-components-story --example richtext_next`
- 手动验收清单（Iteration 1 通过标准）：
  - **可输入**：启动后无需点击即可直接输入（英文/符号），文本出现在首段（paragraph）。
  - **IME 中文**：拼音预编辑时显示下划线；选字提交后不残留拼音字母。
  - **光标与选区**：
    - 鼠标点击编辑区可落光标。
    - 鼠标按下拖拽可创建/调整选区；Shift+点击可扩展选区。
    - 说明：这里的“拖拽”仅指“拖拽选择文本（selection）”，**不包含**“拖拽重新排列（drag to reorder）”能力。
  - **基础编辑**：Backspace/Delete 删除字符或删除选区；Enter 在光标处分段（split paragraph）。
  - **Undo/Redo**：
    - 工具栏 Undo/Redo 按钮可用，且可撤销/重做：输入、删除、分段、插入 divider。
    - 快捷键：macOS `Cmd+Z`/`Cmd+Shift+Z`，Windows/Linux `Ctrl+Z`/`Ctrl+Shift+Z`。
  - **命令示例（divider）**：
    - 工具栏 “-” 可插入 divider，并自动在其后插入一个空段落，光标落在空段落。
    - 快捷键：macOS `Cmd+Shift+D`，Windows/Linux `Ctrl+Shift+D`。
  - **JSON 打开/保存**：
    - 工具栏 Open/Save 可用（Save 首次会要求选择路径）。
    - 顶部菜单 `Open.../Save/Save As...` 可用（与工具栏行为一致）。
    - 保存为 `PlateValue`（versioned JSON），重新打开后结构保持（paragraph/text/divider 不丢失）。

### Iteration 2：EditorView（gpui 输入/IME/hit-test）与“文本布局 → path/offset”映射闭环

**要实现什么**
- `richtext_next` 示例的 EditorView 体验补齐（仍然处于同一套最终架构：tree+selection+ops+normalize）：
  - gpui `EntityInputHandler` 的 IME 集成完整可用（marked range、replace_text、replace_and_mark、bounds_for_range…）。
  - hit-test：鼠标点位 → `{path, offset}`（至少覆盖 root paragraph text 节点）。
  - selection/caret：单段与跨段选区渲染；双击选词、三击选段；Shift+点击/拖拽扩展选区。
  - 滚动：编辑区可滚动，且输入/移动光标/选择时 **caret 自动滚入视口**（scroll-to-caret）。
- 文本渲染继续复用 `StyledText/TextLayout`，但要形成可复用的 layout cache（用于 hit-test 与 IME bounds）。

**怎么实现（关键做法）**
- 将 layout 缓存视为 `EditorLayout`（纯派生态），不进入 undo/serialize。
- 建立统一的 `TextAnchor` 概念：`{path, byte_offset}` 与 gpui `TextLayout` 的 index 之间相互转换。
- **避免“拖拽时 layout cache 丢失”**：不要在每次 render 时清空 layout cache；layout cache 由每行的 prepaint 写回更新（以便 drag/shift 等连续事件中总有可用的 mapping）。
- **scroll-to-caret**：EditorState 持有 `ScrollHandle` 与 `viewport_bounds`；根据 `TextLayout.position_for_index` 得到 caret bounds，用与旧版一致的 margin 规则计算并更新 `scroll_handle.set_offset(...)`。

**要验收什么**
- 运行方式：`cargo run -p gpui-manos-components-story --example richtext_next`
- 手动验收清单（Iteration 2 通过标准）：
  - **滚动可用**：
    - 连续 Enter 生成大量段落后，编辑区出现纵向滚动条；滚轮/触控板滚动可用。
    - 光标移动/输入时，caret 会自动滚入视口（不会“打字打到看不见”）。
  - **鼠标选区完整**：
    - 单击落光标；按下拖拽可创建/调整选区；Shift+点击/拖拽可扩展选区。
    - 双击：选词（中文边界见“已知问题”）；三击：选中当前段落（若段内存在 `\\n`，则选中当前行）。
  - **跨段选区（root paragraphs）**：
    - 从一个段落拖拽到另一个段落可形成跨段选区，并在每个段落行上可见高亮。
    - Backspace/Delete/Cut 可删除跨段选区，并将起始段与末段尾部内容合并为一个段落。
    - Copy 会以 `\\n` 拼接段落内容写入剪贴板（起始段选区片段 + 中间整段 + 末段片段）。
    - Paste（包含 `\\n`）会拆分为多个段落（避免单段内塞入大量换行导致“三击像全选”的体验问题）。
  - **IME bounds**：
    - 中文拼音预编辑下划线正常；候选框位置跟随 caret（依赖 `bounds_for_range`），提交后不残留拼音字母。
  - **已知问题**：
    - 双击选词对中文边界目前不理想（已记录）：[`docs/richtext-next-known-issues.md`](richtext-next-known-issues.md)

### Iteration 3：插件化能力集扩展（List + Link + Marks + 工具层对接）

**要实现什么**
- 插件体系扩展到“可用的基础写作编辑器”：
  - List 插件（bulleted/numbered）：通过 schema + normalize 表达结构（不允许渲染时扫描连续行）
  - Link 插件：link mark + “set link” 命令（以及 query：是否在 link 上）
  - Marks 插件：bold/italic/underline/strikethrough/code + color/bg（按需）
- 工具层插件（最少一个 toolbar 实现）：
  - 使用 `plate_toolbar` 组件构建 toolbar
  - toolbar 只调用 `CommandRegistry + Query API`（不直连内部结构）

**怎么实现（关键做法）**
- 明确 schema ownership：每个 kind 的 schema 只能由一个插件声明。
- 规范化全部走 normalize pass：例如 list 的“段落 → list item 包裹”、退出列表、合并/拆分列表等，都用 ops 表达并可撤销。

**要验收什么**
- `richtext_next` 示例功能验收：
  - toolbar：bold、link、ul/ol toggle 至少可用，并可 undo/redo
  - list 结构在保存/加载后保持一致（JSON round-trip）
  - 未启用 list 插件时：仍能加载含 list 的 JSON 且不丢数据（渲染可降级显示占位）
- 验收方式：
  - `cargo run --example richtext_next`
  - 额外建议：添加一个 `richtext_next.example.json` 作为回归样本（用于手动/自动加载）

### Iteration 4：面向未来节点类型（Table + Inline Void + 复杂 normalize）

**要实现什么**
- Table 插件：
  - kinds：`table/table_row/table_cell`
  - normalize：cell 至少一个 paragraph；row 只能包含 cell；table 只能包含 row；插入/删除行列命令
- Inline void 插件（mention 或 image）：
  - 可在文本中插入 inline void，并在 selection 移动/删除时表现正确

**怎么实现（关键做法）**
- inline vs block 的约束由 schema 定义，normalize 负责修复越界结构。
- 光标移动策略（上下左右、跨 void）应在 core 提供可覆写的“navigation policy” hook（插件可定制）。

**要验收什么**
- `richtext_next` 示例验收：
  - table 的插入行/列、删除行/列、在 cell 内输入/换行、跨 cell 移动光标
  - inline void 的插入、选中、删除、跨节点移动
  - 保存/加载后结构稳定（normalize 不会产生漂移）
- 验收方式：
  - `cargo run --example richtext_next`
  - 建议新增 `cargo run --example richtext_next_table`（专门覆盖 table/void 的交互回归）

### Iteration 5：一次性替换现有实现（对外 API 兼容壳层）

**要实现什么**
- 将现有对外入口（`gpui_manos_plate::RichTextEditor/RichTextState/RichTextValue` 等）替换为新内核驱动：
  - 可以保留类型名，但内部不再使用旧 `RichTextDocument/BlockKind` 模型
  - 提供兼容的序列化适配器（Slate JSON ↔ 新 JSON ↔ 旧 `RichTextValue` 的过渡策略由“适配器插件”承担）
- 移除旧 monolith 或至少从默认构建路径移出，避免双轨继续存在。

**怎么实现（关键做法）**
- 替换应当是“功能上不回退”的：以你们认为必须支持的能力集为准（建议至少包含 Iteration 3 的能力）。
- 旧格式兼容建议：
  - Slate 兼容作为插件：启用则支持 Slate JSON 导入导出；不启用也不影响内核。

**要验收什么**
- 原有示例可继续工作（或提供等价新示例并更新 README）：
  - `cargo run --example richtext`（若保留该入口）
  - 或 `cargo run --example richtext_next` 成为新的主入口并更新 `README.md`
- `docs` 与示例同步更新：
  - `docs/richtext-extensibility.md`、`docs/richtext-plugin-system-plan.md` 的链接与入口说明准确

---

## 9. 风险与对策

- **风险：只做 UI 插件化收益有限**：必须优先落地命令/Hook/渲染分组这三类“能把逻辑迁走”的扩展点。
- **风险：插件状态与 undo/redo 不一致**：先分层 EditorModel/Session/Layout，并定义“进入快照”的硬规则。
- **风险：树模型 + IME/hit-test 实现复杂度上升**：优先把“view 层 hit-test ↔ path/offset”抽成独立模块，并尽早用真实 IME 场景验证（这属于允许的技术验证）。
