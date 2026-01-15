# RichText：插件系统与未来架构计划（激进重构版）

本文基于对本仓库 `gpui-manos-plate`（`crates/rich_text`）的实现细读，给出一份**面向未来与最佳实践**的重构计划：直接对齐 Plate/Slate 体系常见的设计（树模型、operations、normalize、可组合插件），不再规划 v1 过渡态。

相关背景与当前实现边界见：
- [RichText Editor：扩展性与重构方向（面向未来）](richtext-extensibility.md)
- 工具层 UI 组件（Toolbar）参考：[Plate Toolbar Buttons（GPUI 组件实现教程）](plate-toolbar-buttons.md)
- 暂不处理/已知问题清单：[RichText Known Issues / Deferred Work](richtext-known-issues.md)

> 说明：本文中历史命令 `cargo run -p gpui-manos-components-story --example richtext` 已废弃；请使用 `cargo run` 启动 Story Gallery，并在左侧选择 `Rich Text`。

---

## 1. 现状复盘（最终架构，已落地）

### 1.1 文档模型：树模型 + marks + path-based selection

- 文档：`gpui-plate-core::Document { children: Vec<Node> }`
- 节点：`Node = Element { kind, attrs, children } | Text { text, marks } | Void { kind, attrs }`
- 选区：`Selection { anchor: Point { path, offset }, focus: ... }`（path-based）

对应文件：
- `crates/plate-core/src/core.rs`
- `crates/plate-core/src/serde_value.rs`

### 1.2 内核：Editor + PluginRegistry（schema / normalize / command / query）

- `Editor` 统一承载：transaction apply、normalize、undo/redo、command/query dispatch
- `PluginRegistry` 收集插件贡献：
  - `NodeSpec`（schema/children 约束、void/inline/block）
  - `NormalizePass`（输出修复 ops）
  - `CommandSpec` / `QuerySpec`（稳定字符串 ID，供工具层调用）
- 内置 richtext 组合插件：paragraph/divider、marks、list、table、mention…

对应文件：
- `crates/plate-core/src/core.rs`
- `crates/plate-core/src/ops.rs`
- `crates/plate-core/src/plugin.rs`

### 1.3 视图：gpui-manos-plate 的 RichTextState（输入/IME/hit-test/渲染）

- `gpui-manos-plate::RichTextState` 将 gpui 输入映射为 `Editor` 的 command/transaction，并渲染为 gpui elements
- `layout_cache` 以 `block_path: Vec<usize>` 为 key：统一支撑 hit-test/caret/selection/IME bounds
- keybindings & actions：`gpui_manos_plate::init` 绑定到 `RichText` key context

对应文件：
- `crates/rich_text/src/state.rs`

### 1.4 序列化：无损 versioned JSON（PlateValue）

- `PlateValue` 负责 versioned JSON ↔ `Document` 互转；未知节点/attrs 可 round-trip

对应文件：
- `crates/plate-core/src/serde_value.rs`

### 1.5 工具层：story 示例只依赖 command/query

- `crates/story/src/richtext.rs` 的 toolbar/menu/dialog 只调用 `RichTextState` 的 command/query 接口（底层仍走 command id），避免直连内部树结构。

---

## 2. 改造的必要性（为什么要做插件系统）

### 2.1 现有结构的扩展成本会随功能线性甚至超线性增长

新增一个能力（例如：新块、新输入规则、新序列化字段、新渲染容器）通常会同时涉及：
- `crates/plate-core/src/plugin.rs`：schema/normalize/commands/queries（能力的“可组合本体”）
- `crates/rich_text/src/state.rs`：仅当涉及 gpui 输入/IME/hit-test/渲染时才需要扩展（尽量保持通用）
- `crates/plate-core/src/serde_value.rs`：仅当涉及持久化字段/格式升级时需要修改
- `crates/story/src/richtext.rs`：示例 UI 入口与验收

目标：让新增能力尽量收敛到“一个插件 + 少量 view 适配 + 一份验收样例”，避免特例在单体文件中持续堆叠。

### 2.2 undo/redo 与“状态归属”边界需要被系统化

现在 undo 快照不包含 columns 宽度等状态；未来插件更多时，“哪些状态应可撤销/可持久化/仅 UI 会话态”必须有统一规则，否则会出现大量一致性 bug。插件系统的三层分治能把这条线一次性画清楚。

---

## 3. 可行性与现实边界（能做到多像 Plate.js）

### 3.1 可行性：你们已有插件系统所需的三大“骨架”

- 内核命令集合（`PluginRegistry` 的 commands/queries + `Editor::run_command`）
- 渲染管线（`gpui-manos-plate::RichTextState`：block_path 布局缓存 + per-line element）
- 序列化层（`PlateValue` 的 versioned JSON，无损 round-trip）

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

> 新内核已以独立 crate 落地：`crates/plate-core`（crate 名称：`gpui-plate-core`），并通过 Story Gallery 的 `Rich Text` 页面提供验收入口（实现位于 `crates/story/src/richtext.rs`）；并已完成 `gpui-manos-plate`（`crates/rich_text`）的一次性替换壳层。

本项目采用的落地位置与命名约定：
- 新内核 crate：`crates/plate-core`
- crate 名称：`gpui-plate-core`
- 建议示例入口：`cargo run`，在 Story Gallery 左侧选择：`Rich Text`

内置命令与查询（便于工具层/插件协作）：
- Command IDs（示例）：`core.insert_divider`、`image.insert`、`marks.toggle_bold`、`marks.toggle_italic`、`marks.toggle_underline`、`marks.toggle_strikethrough`、`marks.toggle_code`、`marks.set_link`、`marks.unset_link`、`marks.set_text_color`、`marks.unset_text_color`、`marks.set_highlight_color`、`marks.unset_highlight_color`、`block.set_heading`、`block.unset_heading`、`code_block.toggle`、`block.set_align`、`blockquote.wrap_selection`、`blockquote.unwrap`、`toggle.wrap_selection`、`toggle.unwrap`、`toggle.toggle_collapsed`、`todo.toggle`、`todo.toggle_checked`、`block.indent_increase`、`block.indent_decrease`、`list.toggle_bulleted`、`list.toggle_ordered`、`list.unwrap`、`mention.insert`、`table.insert`、`table.insert_row_above`、`table.insert_row_below`、`table.insert_col_left`、`table.insert_col_right`、`table.delete_row`、`table.delete_col`、`table.delete_table`、`columns.insert`、`columns.unwrap`、`columns.set_widths`
- Query IDs（示例）：`marks.get_active`、`marks.is_bold_active`、`marks.is_italic_active`、`marks.is_underline_active`、`marks.is_strikethrough_active`、`marks.is_code_active`、`marks.has_link_active`、`block.heading_level`、`code_block.is_active`、`block.align`、`block.indent_level`、`blockquote.is_active`、`toggle.is_active`、`toggle.is_collapsed`、`todo.is_active`、`todo.is_checked`、`list.active_type`、`list.is_active`（参数：`{ "type": "bulleted" | "ordered" }`）、`table.is_active`、`columns.is_active`
- Transaction Transform IDs（示例）：`autoformat.on_space`
- 代码位置：`crates/plate-core/src/plugin.rs`

### 当前进度（阶段性）

截至目前，新内核与 `richtext` 示例已完成（或待验收）的主要能力：
- 已完成（可验收）：Iteration 1 ~ Iteration 5（树模型/ops/normalize/插件命令、EditorView 输入与 IME、List/Link/Marks、Inline runs 与范围 marks、Inline Void mention）。
- 已补齐（架构对齐）：工具层状态已通过 `Query API` 获取（与 “CommandRegistry + Query API” 约束一致），并校准了多行选区渲染与三击选段行为。
- 已记录问题：双击选词对中文边界不理想，见 [`docs/richtext-known-issues.md`](richtext-known-issues.md)。
- Iteration 6（Table）已实现：TablePlugin（schema + normalize + commands + query）与 `richtext` 的 table 可编辑闭环。
- Iteration 7（壳层替换）已实现：`gpui-manos-plate` 已从旧 `crates/rich_text` monolith 切换为 `gpui-plate-core` 驱动的最终架构实现。
- Iteration 8（Marks Pro）已实现：Italic/Underline/Strike/Code/Colors（命令/查询/渲染/工具栏）闭环。
- Iteration 9（Schema-driven TextBlock + Heading）已实现：text block 判定由 `node_specs` 驱动；新增 Heading 插件与 toolbar dropdown（H1/H2/H3/Paragraph）。
- Iteration 10（容器型 Block + Blockquote）已实现：BlockquotePlugin（schema + normalize + commands + query）+ view 递归渲染（包含 table cell 内嵌容器）+ toolbar Quote。
- Iteration 11（Todo/Task）已实现：TodoPlugin（schema + normalize + commands + query）+ checkbox 前缀可点击（command → tx → undo）+ toolbar Todo。
- Iteration 12（Indent）已实现：IndentPlugin（block.indent_* + normalize + query）+ list_level/outdent 支持 + toolbar Indent。
- Iteration 13（Toggle）已实现：TogglePlugin（schema + normalize + commands + query）+ view 折叠渲染 + chevron 命中测试 + toolbar Toggle/Collapse。
- Iteration 14（Transaction Hooks + Autoformat）已实现：新增 `transaction_transforms` 扩展点（apply 前可预览/改写 tx）+ AutoformatPlugin（Markdown shortcuts）端到端闭环。
- Iteration 15（Image Block Void）已实现：ImagePlugin（block void，attrs: `src/alt`）+ view 占位渲染 + toolbar “Insert image” dialog + 回删/删除移除 image block。
- Iteration 16（Block Void Selection）已实现：image/divider 可点击选中（高亮）+ Backspace/Delete 删除选中 block void（不再依赖“必须在相邻段落边界”）。
- Iteration 17（Block Void Clipboard）已实现：选中 image/divider 后可 Cmd/Ctrl+C/X/V（复制/剪切/粘贴），使用 gpui clipboard JSON metadata 在编辑器内无损移动 block void。
- Iteration 18（Block Subtree Clipboard）已实现：Alt-click 选中任意 block（element/void，含容器）并高亮，Cmd/Ctrl+C/X/V 复制/剪切/粘贴整棵 block 子树（gpui clipboard JSON metadata 无损 round-trip），Backspace/Delete 删除选中 block。
- Iteration 19（Block Actions）已实现：基于选中 block 提供 duplicate/move up/move down/delete，并接入 story toolbar（disabled 状态由 view query 决定）。
- Iteration 20（Selection Range Clipboard）已实现：普通选区 Cmd/Ctrl+C/X/V 写入/读取结构化 fragment（保留 marks/inline void/多段结构），跨应用仍以 plain text 降级。
- Iteration 21（Code Block）已实现：新增 `code_block` text block（plugin schema + command/query + 渲染样式）并接入 “Block type” 下拉菜单。
- Iteration 22（Block Align）已实现：新增 block-level 对齐属性（Left/Center/Right）的命令/查询/normalize，并接入 toolbar Align 菜单与渲染 `text_align`。
- Iteration 23（Block Font Size）已实现：字号（A-/A/A+）控件接入 toolbar，命令/查询/渲染闭环（block-level attrs）。
- Iteration 24（Link UX）已实现：Set Link 在无选区时可选词或插入 URL，caret 在 link 内可编辑整段链接；toolbar 增加 Unlink。
- Iteration 25（Heading Menu）已实现：Block type 下拉菜单补齐 Heading 4/5/6（与 core `level: 1..=6` 对齐）。
- Iteration 26（Command Palette）已实现：可搜索命令面板（来自 `CommandRegistry` 自动发现），支持可选 JSON args 执行命令。
- Iteration 27（Columns）已实现：ColumnsPlugin（schema + normalize + commands + query）+ view 分栏渲染与拖拽列宽（mouse up 单次提交，Undo/Redo 为 1 步）+ story toolbar Columns。
- Iteration 28（Command Metadata + Command Palette Pro）已实现：`CommandSpec` metadata（description/keywords/args_example/hidden）+ 命令面板键盘导航（↑/↓、Enter、Esc）+ args example 一键填充 + `Cmd/Ctrl+Shift+P` 打开。
- Iteration 29（Table Pro）已实现：新增 table 行/列能力（row above / col left / delete table）并把 table 工具栏升级为 row/col 两个菜单（可一次覆盖插入/删除）。
- Iteration 30（Toolbar Layout Pro）已实现：`richtext` toolbar 支持横向滚动，窄窗口下不再“静默裁剪”右侧按钮。
- Iteration 31（Emoji Inline Void）已实现：新增 `emoji` inline void 节点与 `emoji.insert` 命令，并在 toolbar 提供 Emoji 插入入口（可在复制/保存/打开中无损保留）。
- Iteration 32（Link Shortcut Pro）已实现：新增 `Cmd/Ctrl+K` 打开 Set Link 对话框，并在 Edit 菜单提供 `Set Link...` 入口。
- Iteration 33（Media/Mention UX Pro）已实现：image 支持真实渲染与 `Cmd/Ctrl+Click` 打开 src；mention 支持自定义 label 输入（dialog）。
- Iteration 34（Find Highlight Pro）已实现：`Cmd/Ctrl+F` 打开 Find，对匹配文本做高亮（不影响文档/undo）。
- Iteration 35（Find Navigation Pro）已实现：`Cmd/Ctrl+G` / `Cmd/Ctrl+Shift+G` 跳转到下一/上一匹配，Find dialog 展示 `current/total` 并高亮当前匹配。
- Iteration 36（Image Insert UX Pro）已实现：image 支持工具栏 file picker / URL 插入，本地路径在 view 渲染时以 `img(PathBuf)` 加载；`Cmd/Ctrl+Click` 可打开本地文件或 URL。
- Iteration 37（Media Input Pro）已实现：支持拖拽图片文件到编辑器插入 image block；支持粘贴剪贴板图片（截图）插入 image block。
- Iteration 38（Media File Picker Pro）已实现：Image 文件选择器支持多选并一次性插入；当文件选择器失败/选中非图片时给出明确通知，避免“点击无反应”的误解。
- Iteration 39（Embedded Clipboard Image Pro）已实现：粘贴剪贴板图片默认以内嵌 `data:` URL 的方式落在文档里，并在渲染层支持 `data:` URL 解码与缓存，确保保存/打开 JSON 后图片仍可显示。
- Iteration 40（Portable Export Pro）已实现：提供 “Embed Local Images” 一键把本地路径图片转换为内嵌 `data:` URL，使 drop/file picker 插入的图片也能随 JSON 一起保存与迁移。
- Iteration 41（Portable Export JSON Pro）已实现：新增 `Export Portable JSON...`，在不修改当前文档的前提下导出一份“内嵌本地图片”的可迁移 JSON（适合分发/备份）。
- Iteration 42（Plate Bundle Export Pro）已实现：新增 `Export Plate Bundle...`（JSON + `assets/`）与 `document_base_dir` 解析相对 `image.src`，让“文件夹级”可迁移成为一等能力（比 data URL 更轻）。
- Iteration 43（Relative Assets UX Pro）已实现：当文档有 base dir 时，插入本地图片默认写入相对 `src`；`Save As...` 会复制引用的相对 assets，避免保存后图片立刻丢失。
- Iteration 44（Bundle Externalize Data URLs Pro）已实现：`Export Plate Bundle...` 会把 `data:` 图片也外置到 `assets/`，显著缩小 JSON，便于代码 review 与版本管理。
- Iteration 45（Collect Assets In-Place Pro）已实现：新增 `Collect Assets into ./assets (Rewrite src)`，把当前文档里的本地图片与 `data:` 图片统一落到同目录 `assets/` 并改写 `image.src`（单步 undo）。
- Iteration 46（Portability Diagnostics Pro）已实现：新增 `Portability Report...`，在保存/导出时提示缺失资源，并在报告里提供一键 `Collect Assets`/`Export Bundle`/`Export Portable JSON`。
- Iteration 47（Image Dialog-First UX Pro）已实现：image 默认打开 URL/Path 对话框（支持 `Browse…` 选文件 + 展示 base dir），并提供 `Edit → Insert Image...` 与 `Cmd/Ctrl+Shift+I` 入口；Shift+Click 保留“多选 file picker 直接插入”。
- Iteration 48（Failure Visible Pro）已实现：任何 command/apply 失败都会在编辑器窗口弹出明确通知（含 command id / tx source），避免出现“没反应但不知道为什么”的验收阻塞。

### Iteration 1：新内核最小闭环（树模型 + ops + normalize + JSON + 渲染）

**要实现什么**
- 新 crate：`gpui-plate-core`（目录：`crates/plate-core`），包含：
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
- 能运行一个新的示例：Story Gallery 的 `Rich Text` 页面（实现位于 `crates/story/src/richtext.rs`）
- 运行方式：`cargo run`，在 Story Gallery 左侧选择：`Rich Text`
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
- `richtext` 示例的 EditorView 体验补齐（仍然处于同一套最终架构：tree+selection+ops+normalize）：
  - gpui `EntityInputHandler` 的 IME 集成完整可用（marked range、replace_text、replace_and_mark、bounds_for_range…）。
  - hit-test：鼠标点位 → `{path, offset}`（覆盖 paragraph/list_item 内的多 Text leaf，并保证 row byte offset ↔ leaf `{path, offset}` 的互转正确）。
  - selection/caret：单段与跨段选区渲染；双击选词、三击选段；Shift+点击/拖拽扩展选区。
  - 滚动：编辑区可滚动，且输入/移动光标/选择时 **caret 自动滚入视口**（scroll-to-caret）。
- 文本渲染继续复用 `StyledText/TextLayout`，但要形成可复用的 layout cache（用于 hit-test 与 IME bounds）。

**怎么实现（关键做法）**
- 将 layout 缓存视为 `EditorLayout`（纯派生态），不进入 undo/serialize。
- 建立统一的 `TextAnchor` 概念：`{path, byte_offset}` 与 gpui `TextLayout` 的 index 之间相互转换。
- **避免“拖拽时 layout cache 丢失”**：不要在每次 render 时清空 layout cache；layout cache 由每行的 prepaint 写回更新（以便 drag/shift 等连续事件中总有可用的 mapping）。
- **scroll-to-caret**：EditorState 持有 `ScrollHandle` 与 `viewport_bounds`；根据 `TextLayout.position_for_index` 得到 caret bounds，用与旧版一致的 margin 规则计算并更新 `scroll_handle.set_offset(...)`。

**要验收什么**
- 运行方式：`cargo run -p gpui-manos-components-story --example richtext`
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
    - 快捷键：`Cmd/Ctrl+C`/`Cmd/Ctrl+X`/`Cmd/Ctrl+V`/`Cmd/Ctrl+A` 可用（与菜单项一致）。
    - 断行合并：
      - 光标在行首按 Backspace：与上一段合并（上一块为 divider 时优先删除 divider）。
      - 光标在行尾按 Delete：与下一段合并（下一块为 divider 时优先删除 divider）。
    - 跨行光标与扩选：
      - 左右方向键在行首/行尾可跨行移动。
      - `Shift+Left/Right` 在行首/行尾可跨行扩展选区。
  - **IME bounds**：
    - 中文拼音预编辑下划线正常；候选框位置跟随 caret（依赖 `bounds_for_range`），提交后不残留拼音字母。
  - **已知问题**：
    - 双击选词对中文边界目前不理想（已记录）：[`docs/richtext-known-issues.md`](richtext-known-issues.md)

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
- 运行方式：`cargo run -p gpui-manos-components-story --example richtext`
- 回归样本：`plate-core.example.json`（建议用 Open 打开并 Save/Reload 验证 round-trip）
  - 手动验收清单（Iteration 3 通过标准）：
  - **Toolbar（命令化）**：
    - Bold：点击或快捷键 `Cmd/Ctrl+B` 可切换粗体（Undo/Redo 生效）。
    - List：`bulleted/ordered` toggle 可用（Undo/Redo 生效），ordered 会自动维护序号（由 normalize 写入 `list_index`，渲染不扫描相邻行）。
    - Link：点击 Link 打开对话框输入 URL（若已有 link 会预填 URL）；OK 后文本显示为 link 样式；清空 URL 并确认可移除；`Cmd/Ctrl+Click` 可打开 URL。
  - **编辑体验（list）**：
    - 在 list item 内按 Enter 会新增同类型 list item；在空 list item 上按 Enter 会退出列表（变回 paragraph）。
    - 在 list item 起始处按 Backspace 会退出列表（变回 paragraph）。
  - **JSON round-trip**：
    - 含 list/link/bold 的文档保存后再次打开，结构与样式保持（`list_item.attrs.list_type/list_index`、`text.marks`）。
  - **已知问题**：见 [`docs/richtext-known-issues.md`](richtext-known-issues.md)。

### Iteration 4：Inline Text Runs（多 Text leaf）+ 选区范围 marks（bold/link）

**要实现什么**
- 结构表达：paragraph/list_item 允许包含多个 `Text` leaf（inline runs），以支持“同一段内的局部 marks”。
- normalize：合并相邻 `Text` leaf（marks 相同则合并），防止编辑后碎片化。
- 命令：`marks.toggle_bold / marks.set_link / marks.unset_link` 对**选区范围**生效（跨 runs、跨段落可组合）。
- `richtext`：
  - 渲染按 runs 输出 `StyledText` runs（同一行/段内多样式共存）。
  - hit-test/selection/IME 统一采用 “row byte offset ↔ leaf `{path, offset}`” 的互转，不再假设 `selection.path == [row, 0]`。
- undo/redo：修复/覆盖 multi-op transaction 的可逆性与顺序正确性（典型：replace、paste、normalize merge）。

**怎么实现（关键做法）**
- marks 变更：按 selection 计算每段落的 `[start, end)`（row offset），对受影响 runs 做 split → apply marks → merge（尽量以 ops 表达，便于 undo/redo 与测试）。
- merge normalize：对 contiguous runs 进行“整段合并”，并从**右到左**生成 remove ops，避免 pairwise merge 在同一 pass 中造成索引漂移与文本丢失。
- view 层：所有 gpui `TextLayout` index 都先映射到 row offset，再映射到 leaf path；IME 的 replace/marked 也走 row-level replace。

**要验收什么**
- 运行方式：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 4 通过标准）：
  - **范围粗体**：选中部分文本点击 Bold/`Cmd/Ctrl+B`，仅选区变粗；再次 toggle 恢复；Undo/Redo 生效。
  - **范围链接**：选中部分文本 Set Link，仅选区呈现链接；Unlink 仅移除选区链接；Undo/Redo 生效。
  - **输入继承 marks**：光标处 toggle bold 后输入文本为粗体；再 toggle 关闭后输入为正常（验证“光标 marks”不污染整段）。
  - **跨 runs 选择/删除/复制**：跨粗体/普通 runs 拖拽选择，高亮正确；Copy 复制纯文本；Backspace/Delete 删除不丢字、光标位置合理。
  - **IME 与 marks 共存**：在粗体或链接文字中中文 IME 预编辑有下划线，提交不残留拼音字母。
  - **JSON round-trip**：包含多 runs（部分粗体/部分链接）的文档 Save → Open 后结构与 marks 保持（Text nodes 被分段但不丢内容）。

### Iteration 5：Inline Void（Mention）+ 原子导航（光标/删除/选中）

**要实现什么**
- Mention 插件（inline void）：
  - kind：`mention`
  - 命令：`mention.insert`（参数：`{ "label": "Alice" }`）
  - JSON round-trip：保存/加载不丢失（`node: "void", kind: "mention", attrs.label`）
- `richtext`：
  - 渲染：把 inline void 映射为显示文本（例如 `@Alice`）参与 `TextLayout`。
  - 交互：将 mention 视为**原子单元**：
    - 光标左右移动可跨过 mention（不进入内部字符）。
    - Backspace/Delete 可一次性删除整个 mention。
    - 双击可选中整个 mention（而不是只选中 `@` 或部分字母）。
  - 工具栏/快捷键：提供插入 mention 的入口（例如 `Cmd/Ctrl+Shift+M`）。

**怎么实现（关键做法）**
- core：
  - schema：`mention` 声明为 `Inline + Void`。
  - 命令：在光标所在 text leaf 处 split → insert void → 确保两侧存在 text leaf 承载 selection。
  - marks：范围计算需把 inline void 纳入线性偏移（长度取显示文本的 UTF-8 bytes），但 marks 不应修改 void 本体。
- view：
  - 线性化段落时同时输出 `combined_text` 与 `void_ranges`（用于“原子导航”）。
  - 光标移动/删除使用 `prev/next_cursor_offset_in_row` 跳过 void；replace/split 时保持 void 不可分割。

**要验收什么**
- 运行方式：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 5 通过标准）：
  - **插入 mention**：点击工具栏按钮或按 `Cmd/Ctrl+Shift+M` 插入 `@Alice`，光标落在其后。
  - **原子导航**：左右方向键跨过 mention；Backspace/Delete 一次删除整个 mention；Undo/Redo 生效。
  - **选区表现**：拖拽选区跨过 mention，高亮连续；双击 mention 会选中整个 mention。
  - **JSON round-trip**：Save → Open 后 mention 仍为 void 节点且展示一致。

### Iteration 6：面向未来节点类型（Table + 复杂 normalize）

**要实现什么**
- `gpui-plate-core`：新增 `TablePlugin`（schema + normalize + commands + query）
  - kinds：`table/table_row/table_cell`
  - normalize 目标（最终稳定态）：
    - `table` 至少 1 行；`table_row` 至少 1 个 cell；`table_cell` 至少 1 个 `paragraph("")`
    - `table` 保持矩形：所有 row 的列数对齐（补齐缺失 cell）
  - commands：
    - `table.insert`（args：`{ "rows": number, "cols": number }`，默认 2×2）
    - `table.insert_row_below`
    - `table.insert_col_right`
    - `table.delete_row`
    - `table.delete_col`
  - queries：
    - `table.is_active`：selection 是否位于 table 内（用于工具栏 enable/disable）
- `richtext`：支持 table 的“可编辑”闭环
  - 渲染：表格以网格容器渲染，cell 内允许多个 text block（paragraph/list_item）纵向堆叠
  - 输入/选区：view 以 `block_path: Vec<usize>` 识别任意深度 text block（使 cell 内编辑成立）
  - 工具栏：提供插入表格/增删行列入口

**怎么实现（关键做法）**
- core：
  - `TablePlugin` 位于 `crates/plate-core/src/plugin.rs`；normalize pass：`table.normalize_structure`
  - 命令统一生成 `Transaction { ops }` 并交由 `editor.apply(tx)` 应用（确保 normalize/undo 一致）
  - 单测锁定结构与命令行为：`crates/plate-core/tests/table.rs`
- view（`richtext`）：
  - 用 `text_block_paths()` 生成 DFS 顺序的 text block 列表，作为跨 block 导航/复制/全选的统一“线性视图”
  - `layout_cache` 以 `block_path` 为 key 缓存 `TextLayout`，hit-test/IME ↔ path/offset 统一走这份映射
  - 粘贴/全选等命令改为基于 `block_path`（保证 table cell 内行为正确）

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 单测：`cargo test -p gpui-plate-core`
- 手动验收清单（Iteration 6 通过标准）：
  - **插入表格**：点击工具栏 Table 插入 2×2，光标落在左上 cell 可输入。
  - **cell 内编辑**：在 cell 内输入、Enter 换行（生成新 paragraph）、Undo/Redo 生效。
  - **跨 cell 导航**：在 cell 文本末尾按右方向键进入下一个 cell；在开头按左方向键返回上一个 cell。
  - **增删行列**：
    - 在 table 内点击“Insert row below/Insert col right”，结构保持矩形且光标落到新增 cell。
    - 点击“Delete row/Delete col”，若删到最后一行/列则 table 被移除并插回 paragraph。
  - **跨 cell 选区 + 复制粘贴**：
    - 鼠标拖拽可跨 cell 选中多段文本，`Cmd/Ctrl+C` 得到带换行的纯文本。
    - `Cmd/Ctrl+V` 在 cell 内粘贴多行文本，会在该 cell 内生成多段 paragraph。
  - **Select All**：`Cmd/Ctrl+A` 会包含 table cell 内文本。
  - **JSON round-trip**：Save As → Open 后 table 结构不漂移（normalize 后稳定），cell 不会变空。

### Iteration 7：一次性替换现有实现（壳层替换，已完成）

**要实现什么**
- 将 `gpui-manos-plate`（`crates/rich_text`）从旧 monolith 一次性切换为 `gpui-plate-core` 驱动：
  - 对外保留 `RichTextState/RichTextEditor`（以及 `RichTextValue = PlateValue`），但内部不再使用旧 blocks 模型
  - 旧实现源码直接移除，避免双轨继续存在
- 将验收入口统一到：`cargo run`

**怎么实现（关键做法）**
- 不为“兼容旧格式/旧模型”牺牲底层：旧 Slate JSON 如需支持，应以**独立适配器插件/独立 crate**实现（不进入内核默认路径）
- 壳层只做 gpui 视图适配与命令/查询桥接；结构约束与编辑语义全部留在 core（schema/normalize/ops）

**要验收什么**
- 编译：`cargo check -p gpui-manos-plate`
- 示例：`cargo run`
  - app menu 的 Edit actions（Undo/Redo/Cut/Copy/Paste/SelectAll）在编辑器聚焦时可用
  - Open/Save/Save As 能读写 `gpui-plate` JSON（例如 `plate-core.example.json`）
- 文档：`docs/richtext-extensibility.md`、`docs/richtext-plugin-system-plan.md`、`docs/richtext-known-issues.md` 链接与入口说明准确

---

### Iteration 8：工具栏增强（Marks Pro：Italic/Underline/Strike/Code/Colors）

> 背景：旧实现的 toolbar 能力更丰富；当前 `richtext` 示例为验证最终架构（core/view/plugin/IME/serialize）只落了最小必要插件集，因此 UI 能力与旧示例不对齐是**阶段性现象**。本迭代目标是在**不牺牲底层设计**的前提下，用插件体系把“高频 inline 格式能力”补齐，并把 toolbar 丰富度拉回到可展示的水平。
>
> 说明：旧 toolbar 中的“字号（A-/A/A+）”属于 **block-level 属性**，不属于本迭代的 inline marks 范畴；将以独立迭代补齐（见 Iteration 23）。
>
> 状态：已实现（`crates/plate-core/src/plugin.rs`、`crates/rich_text/src/state.rs`、`crates/story/src/richtext.rs`）。
>
> 旧版示例截图（参考）：![Legacy RichText toolbar](richtext.example.png)

**要实现什么**
- `gpui-plate-core`（data/model + commands/queries）：
  - 扩展 `Marks`：`italic/underline/strikethrough/code`（bool）与 `text_color/highlight_color`（可选颜色，建议存 RGBA hex 字符串，如 `#rrggbbaa`）。
  - 扩展 marks commands（均为稳定 string id）：
    - `marks.toggle_italic`
    - `marks.toggle_underline`
    - `marks.toggle_strikethrough`
    - `marks.toggle_code`
    - `marks.set_text_color` / `marks.unset_text_color`（args：`{ "color": "#rrggbbaa" }`）
    - `marks.set_highlight_color` / `marks.unset_highlight_color`（args：`{ "color": "#rrggbbaa" }`）
  - 扩展 marks queries（至少提供 bool query 供工具层直取）：
    - `marks.is_italic_active` / `marks.is_underline_active` / `marks.is_strikethrough_active` / `marks.is_code_active`
- `gpui-manos-plate`（view）：
  - 渲染：将上述 marks 映射到 gpui `TextStyle`（`font_style/underline/strikethrough/background_color/font_family/text color`）。
  - 快捷键（Mac/Win-Linux）：`Cmd/Ctrl+I/U`（italic/underline），`Cmd/Ctrl+Shift+X`（strike），`Cmd/Ctrl+E`（code）。（如有冲突可调整，但需写入文档）
- `story`（示例）：
  - toolbar 增加对应按钮与颜色选择器（`PlateToolbarIconButton` + `PlateToolbarColorPicker`），并保持“只依赖 command/query”。
  - 颜色选择器行为：选择颜色→ set；清空颜色→ unset。

**怎么实现（关键做法）**
- core：
  - 复用现有“range marks”机制（`toggle_mark_at_caret` + `apply_mark_range`），新增一个通用的 `toggle_bool_mark`/`set_optional_mark` 级别 helper，避免为每个 mark 复制一份“跨 blocks 扫描”的逻辑。
  - 颜色值不引入 gpui 类型：落到 `String`（RGBA hex）即可；view 负责解析到 `Hsla/Rgba`。
- view：
  - `link` 的渲染优先级高于 `text_color`（链接永远蓝色 + underline）；其余 marks 叠加。
  - `code` 的 monospace 使用 `cx.theme().mono_font_family`，背景色建议使用 theme 的 muted（若用户显式设置 highlight_color 则以用户值为准）。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 单测：`cargo test -p gpui-plate-core`
- 手动验收清单（Iteration 8 通过标准）：
  - **四种格式按钮**：Bold/Italic/Underline/Strike/Code 均可切换；对选区生效、对 caret（collapsed selection）会影响后续输入。
  - **快捷键**：`Cmd/Ctrl+I/U/Shift+X/E` 行为与按钮一致，Undo/Redo 可回滚。
  - **颜色**：
    - 选择 Text color → 选区文字变色；清空 → 恢复默认色。
    - 选择 Highlight → 背景高亮；清空 → 取消高亮。
  - **跨 blocks**：跨两段（paragraph 或 list_item）拖拽选区后执行 italic/underline/strike/color，两个 block 都被正确分割并应用 marks。
  - **表格 cell**：在 table cell 内选择文本后应用 marks（含颜色），行为与普通段落一致。
  - **JSON round-trip**：Save As → Open 后 marks（含颜色）不丢失，展示一致。

### Iteration 9：视图层泛化（TextBlock 由 Schema 驱动）+ Heading

> 目标：新增一个“heading”这类 text block 时，不再需要在 view 层到处硬编码 `paragraph/list_item`；并通过 Heading 插件验证“新增 block 能力 = core 插件 + 少量 UI”，让扩展路径更接近 Plate。

**要实现什么**
- `gpui-manos-plate`（view）：
  - 将 “text block 判定” 从硬编码改为由 `PluginRegistry::node_specs()` 驱动：
    - `NodeRole::Block` + `ChildConstraint::InlineOnly` + `!is_void` ⇒ 视为 text block
  - 将 `text_block_paths()`、`row_offset_for_point()`、IME range 计算等全部迁移到“schema-driven text block”判断。
  - 支持对不同 text block kind 注入“block-level base TextStyle”（为 heading/quote 等做准备）。
- `gpui-plate-core`（model + plugins）：
  - 新增 `HeadingPlugin`：
    - kind：`heading`（attrs：`level: 1..=6`，默认为 1）
    - schema：Block + InlineOnly
    - commands：`block.set_heading`（args：`{ "level": 1..6 }`）、`block.unset_heading`
    - query：`block.heading_level -> Option<u64>`（selection 是否在 heading 内）
- `story`：
  - toolbar 增加 Heading dropdown（Heading 1/2/3/Paragraph），仅通过 command/query 驱动。

**怎么实现（关键做法）**
- view：
  - 引入 `is_text_block_kind(kind)`，只依赖 core 的 `node_specs`；未来新增 text block kind 不再修改一堆 `if el.kind == ...`。
  - `RichTextLineElement` 增加可选 `base_text_style: Option<TextStyle>`（或等价参数），用于 heading 调整字号/字重/行高。
- core：
  - `block.set_heading/unset_heading` 本质是“替换当前 block kind + attrs”，必须走 ops/transaction，避免 bypass normalize/undo。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 单测：`cargo test -p gpui-plate-core`
- 手动验收清单（Iteration 9 通过标准）：
  - **Heading 设置**：Heading dropdown 可切换 H1/H2/H3/Paragraph；渲染字号变化明显。
  - **编辑一致性**：Heading 内输入/删除/Undo/Redo 正常；跨段落选区切换 heading 不会破坏 selection。
  - **跨容器**：在 table cell 内设置 heading，行为与普通段落一致。
  - **JSON round-trip**：Save As → Open 后 heading 的 `kind/attrs` 不丢失，展示一致。

### Iteration 10：容器型 Block（递归渲染 + hit-test）+ Blockquote

> 目标：补齐 Plate/Slate 常见的“容器节点”能力（blockquote、callout、columns…），并把 view 层从“仅支持顶层 blocks + 特例 table”升级为“通用递归 block 渲染”。

**要实现什么**
- `gpui-manos-plate`（view）：
  - 支持递归渲染 container element（children 为 block），并让其中的 text blocks 仍使用统一的 `block_path` 缓存与 hit-test/IME 映射。
- `gpui-plate-core`：
  - `BlockquotePlugin`：
    - kind：`blockquote`（schema：Block + BlockOnly）
    - commands：`blockquote.wrap_selection`、`blockquote.unwrap`（selection 所在 quote）
    - normalize：quote 至少包含 1 个 paragraph（空 quote 自动补齐）
- `story`：
  - toolbar 增加 Quote 按钮（wrap/unwrap）。

**怎么实现（关键做法）**
- core：
  - `blockquote.wrap_selection`：将 selection 覆盖的 block 区间整体抽出，插入为 `blockquote(children=selected)`；并 remap selection 的 `path` 指向 quote 内部对应节点。
  - `blockquote.unwrap`：从 selection 向上找最近 `blockquote`，移除 quote 并将 children 依序插回父容器；并 remap selection。
  - normalize pass：保证 `blockquote` 永远至少包含一个 `paragraph("")`，避免空容器导致 selection 无落点。
- view：
  - 将渲染从“顶层 match + table 特判”重构为 `render_block(node, path)` 的递归渲染：
    - text block：`RichTextLineElement::new(state, path)`（`path` 为该 block 的 element path）
    - list_item：前缀 marker + line element（仍复用同一套 layout_cache/hit-test）
    - blockquote：渲染为容器（左侧竖线 + children 的 block 列表），children 继续递归
    - table：保留布局特判，但 cell 内 block 渲染仍走 `render_block`（允许嵌套 quote 等容器）
- story：
  - Quote 按钮只读 `blockquote.is_active`（query）决定 wrap/unwrap，只调用 command，不直连树结构。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 单测：`cargo test -p gpui-plate-core`
- **Wrap/Unwrap**：选中 1~N 个段落点击 Quote → 变成 blockquote 容器；再点一次可还原。
- **容器内编辑**：quote 内输入/换行/Undo/Redo 正常；鼠标点击可正确定位 caret。
- **JSON round-trip**：Save As → Open 后 quote 结构稳定（normalize 后不漂移）。

### Iteration 11：Task/Todo（checkbox）与可交互前缀

> 目标：恢复旧示例中最有辨识度的 “todo item + checkbox” 体验，并验证“可交互 inline UI 前缀”在新架构下的落地方式（click hitbox → command → tx → undo）。

**要实现什么**
- `gpui-plate-core`：
  - `TodoPlugin`：
    - kind：`todo_item`（schema：Block + InlineOnly，attrs：`checked: bool`）
    - commands：`todo.toggle`（切换当前 block）、`todo.toggle_checked`
    - query：`todo.is_active`、`todo.is_checked`
- `gpui-manos-plate`（view）：
  - todo 前缀渲染 checkbox，并提供可点击 hitbox（点击触发 `todo.toggle_checked`）。
- `story`：
  - toolbar 增加 Todo 按钮。

**怎么实现（关键做法）**
- core：
  - `todo_item` 是标准 text block（`InlineOnly`），用 `attrs.checked: bool` 表达勾选状态；normalize 负责补齐/纠正 `checked` 字段（默认 `false`）。
  - `todo.toggle`：
    - selection 的起止 block 在同一 parent 容器内时，对范围内的 `paragraph`/`todo_item` 批量切换（保持 selection path 不变）。
    - selection 跨 parent 时，降级为仅切换 focus block（避免跨容器重排导致 selection remap 复杂化）。
  - `todo.toggle_checked`：
    - 默认切换 selection 所在 `todo_item` 的 `checked`。
    - 支持可选参数 `{ "path": [...] }`，供 view 的 checkbox 命中测试直接定位并切换指定 block。
- view：
  - 渲染 `todo_item`：checkbox 前缀 + `RichTextLineElement`；checked 时给整行注入 muted+strikethrough base style。
  - 点击 checkbox：用 `layout_cache` 的 text bounds 反推 checkbox 区域（`bounds.left() - 常量宽度`），命中则触发 `todo.toggle_checked({path})` 并阻止进入拖拽选区模式。
  - Enter/粘贴换行：在 `todo_item` 内拆分/插入新行时，新增的 block 继续使用 `todo_item` 且默认 `checked=false`；空 todo_item 上回车退出为 paragraph。
- story：
  - Todo 按钮只读 `todo.is_active`（query）并触发 `todo.toggle`（command）。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 单测：`cargo test -p gpui-plate-core`
- **插入/切换**：点击 Todo 将 paragraph 变为 todo_item，再点还原。
- **勾选**：点击 checkbox 勾选/取消；Undo/Redo 可回滚。
- **跨 blocks**：跨多行选区 toggle todo：
  - 若 selection 在同一 parent 容器内（root/blockquote/table cell 等），范围内 paragraph/todo_item 会批量切换，selection 不乱跳。
  - 若 selection 跨 parent，则只切换 focus block（行为可预测，不做隐式跨容器重排）。
- **Enter 行为**：在 todo_item 内回车会生成新的 todo_item；空 todo_item 上回车退出为 paragraph。
- **JSON round-trip**：Save As → Open 后 `checked` 状态与渲染一致。

### Iteration 12：Block-level Indent（通用缩进 + List level）

> 目标：提供通用的 block 缩进语义（对齐 Plate 常见的 Indent 插件能力），并与 list 的层级缩进打通：list_item 使用 `list_level`，其他 text block 使用 `indent`。

**要实现什么**
- `gpui-plate-core`：
  - `IndentPlugin`：
    - commands：`block.indent_increase`、`block.indent_decrease`
    - query：`block.indent_level`（对 list_item 返回 `list_level`，对其他 text block 返回 `indent`）
    - normalize：`indent/list_level` 只允许 `1..=MAX`，0/非法值移除，超限 clamp。
  - list 与 indent 的语义对齐：
    - `list.unwrap`：若 `list_level > 0` 则先 outdent（`list_level -= 1`），否则退出为 paragraph。
    - `list.toggle_*`：paragraph↔list_item 时在 `indent` 与 `list_level` 之间做映射，避免缩进丢失。
- `gpui-manos-plate`（view）：
  - paragraph/heading/todo_item 根据 `attrs.indent` 做左侧 padding（16px * level）。
  - list_item 继续使用 `attrs.list_level`（现有渲染已支持）。
- `story`：
  - toolbar 增加 IndentIncrease/IndentDecrease 按钮。

**怎么实现（关键做法）**
- core：
  - 通过 `text_blocks_in_order` 找到 selection 覆盖的 text blocks，批量生成 `Op::SetNodeAttrs`（不会移动节点，selection path 不变）。
  - list_item 特判：只改 `list_level`；其他 text block 只改 `indent`。
- view：
  - 缩进只影响布局（padding），不进入 undo/serialize 之外的状态；命中测试仍以 `layout_cache`（text bounds）为准。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 单测：`cargo test -p gpui-plate-core`
- 手动验收清单（Iteration 12 通过标准）：
  - **Indent 按钮**：Indent/Outdent 可对 paragraph/heading/todo_item 生效；多段选区时整段一起缩进/反缩进。
  - **List level**：对 list_item 点击 Indent 会增加层级缩进（marker 跟随右移）；Outdent 会减少层级；空 list_item 回车/Backspace 触发 `list.unwrap` 时若有层级会先 outdent。
  - **Todo + indent**：todo 行缩进会同时移动 checkbox 与文本；回车拆分出的新 todo 行继承 indent 但默认 unchecked。
- **JSON round-trip**：Save As → Open 后 `indent/list_level` 仍然生效（渲染一致）。

### Iteration 13：Toggle（可折叠容器 Block）

> 目标：实现 Toggle（可折叠容器）节点，验证“容器节点 + attrs 折叠渲染 + 可交互前缀命中测试 + selection/undo 语义”的完整闭环。

**要实现什么**
- `gpui-plate-core`：
  - `TogglePlugin`：
    - schema：`toggle` 为 block container（`BlockOnly` children）。
    - attrs：`collapsed: bool`。
    - normalize：
      - 补齐/纠正 `collapsed`（默认 `false`）。
      - 保证 `toggle` 至少有一个 title child；若为空或首 child 不是 `paragraph/heading`，插入 `paragraph("")` 作为 title。
    - commands：`toggle.wrap_selection`、`toggle.unwrap`、`toggle.toggle_collapsed`（支持 args：`{ "path": [...] }` 直接定位目标 toggle）。
    - queries：`toggle.is_active`、`toggle.is_collapsed`。
  - selection 语义：当折叠导致 selection 落在隐藏内容（children[1..]）时，将 anchor/focus 迁移到 title 末尾（避免 view 出现“光标在不可见区域”）。
- `gpui-manos-plate`（view）：
  - 渲染：
    - title 行：chevron 前缀 + title text block（heading 样式与 indent 正常生效）。
    - collapsed 时隐藏 children[1..]（仅渲染 title）。
  - hit-test：点击 title 行左侧 chevron 区域触发 `toggle.toggle_collapsed({path})`，并阻止进入拖拽选区模式。
  - 视图层 text block order：`text_block_paths` 跳过折叠 toggle 的隐藏内容，保证键盘导航/多段选择不会进入不可见 blocks。
- `story`：
  - Toolbar：新增 Toggle（wrap/unwrap）与 Collapse/Expand 按钮，只走 command/query。

**怎么实现（关键做法）**
- core：
  - wrap/unwrap 复用 blockquote 的“同 parent 范围选区包装/拆包 + selection remap”思路。
  - 折叠命令在 core 内完成 selection 修正（迁移到 title 末尾），避免把“可见性”耦合进 EditorView。
- view：
  - chevron 命中测试复用 todo checkbox 的方式：基于 `layout_cache` 的 text bounds，使用 `bounds.left() - 常量宽度` 推导前缀区域。
  - 折叠只影响渲染与可见 block order，不修改文档结构；undo/redo 通过 command→tx 自动覆盖。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 单测：`cargo test -p gpui-plate-core`
- 手动验收清单（Iteration 13 通过标准）：
  - **Wrap/Unwrap**：选中多段（至少 2 行）后点 Toggle，将选区 blocks 包进一个 toggle；再点 Toggle 还原（selection 不乱跳）。
  - **折叠/展开**：点击 title 左侧 chevron 或 toolbar 的 Collapse/Expand：
    - 折叠后只显示 title；展开后恢复显示内容。
    - 若折叠时光标在内容区，折叠后光标自动移动到 title 末尾（可见且可继续输入）。
  - **键盘导航**：折叠状态下用上下方向键移动光标，不会落入 toggle 的隐藏内容。
  - **Undo/Redo**：折叠/展开、wrap/unwrap 都可被 undo/redo 正确回滚。
- **JSON round-trip**：Save As → Open 后 `toggle` 的 children 结构与 `collapsed` 状态保持一致（未启用插件也不丢数据）。

### Iteration 14：Transaction Hooks（tx transform）+ Autoformat（Markdown shortcuts）

> 目标：补齐 Plate/Slate 常见的“输入规则/Autoformat”能力所必需的 **Transaction Hook** 扩展点：插件可在 `apply(tx)` 之前基于“预览结果”改写 transaction；并用 AutoformatPlugin 做端到端验证（不在 view 层硬编码规则）。

**要实现什么**
- `gpui-plate-core`：
  - 新增插件扩展点：`transaction_transforms()`：
    - `Editor::apply(tx)` 在真正 apply 之前，按注册顺序执行 tx transforms。
    - transform 可按需调用 `editor.preview_transaction(tx)` 拿到 `TransactionPreview { doc, selection }`（模拟 apply 当前 tx + normalize 后的预览结果），避免为每次输入都强制克隆整棵 doc。
    - transform 可返回新的 tx（通常为“原 tx ops + 追加 ops”，并覆盖 `selection_after`），以表达输入规则/语义改写；返回 `None` 则不改写。
  - AutoformatPlugin：
    - transform id：`autoformat.on_space`
    - 触发条件（保守且可预测）：
      - 仅对 `tx.meta.source == "ime:replace_text"` 生效（避免 IME preedit `ime:replace_and_mark_text` 被误改写）。
      - selection 必须 collapsed。
      - 当前 paragraph 的全文必须**恰好等于 marker**，且 caret 位于 marker 末尾（避免在非空行误触发）。
    - 支持规则（可扩展）：
      - `- ` / `* ` → bulleted list
      - `1. ` → ordered list
      - `> ` → blockquote
      - `# `…`###### ` → heading level 1..6
      - `[ ] ` / `[x] ` → todo（checked false/true）
    - 语义要求：改写后 caret 必须落在“转换后的内容起点”，并可 undo/redo。
- `gpui-manos-plate`（view）：
  - 无需新增输入规则逻辑：继续按现有方式产出 tx（`source: "ime:replace_text"`），由 core transform 统一做 autoformat。

**怎么实现（关键做法）**
- 在 core 内实现 `editor.preview_transaction(tx)`：克隆 doc/selection，模拟 apply tx + normalize，生成 `TransactionPreview`（仅在 transform 需要时调用）。
- AutoformatOnSpace 优先从 tx 的“block children replace ops”还原当前块文本与 caret 全局 offset（无 doc clone）；必要时 fallback 到 `preview_transaction`；转换用“追加 Remove/Insert block node ops”表达（保证 undo 语义与审计性）。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 单测：`cargo test -p gpui-plate-core`
- 手动验收清单（Iteration 14 通过标准）：
  - 在空段落行首依次键入下列 marker，并确认自动转换且光标在内容起点：
    - `- ` / `* ` → 列表（bulleted）
    - `1. ` → 列表（ordered）
    - `> ` → 引用（blockquote）
    - `# ` / `## ` / `### ` → heading level 1..3（可继续扩到 6）
    - `[ ] ` / `[x] ` → todo（checked false/true）
  - **Undo/Redo**：转换动作可被 undo/redo 正确回滚。
  - **IME 安全**：中文输入法 composition（预编辑态）不应被 autoformat 打断（本阶段用单测覆盖 `ime:replace_and_mark_text` 不触发；实际中文 IME 可在 story 里手动验证）。

### Iteration 15：Block Void（Image）端到端闭环

> 目标：补齐富文本编辑器“媒体节点”的最小闭环：一个 **block-level void** 的 `image` 节点（schema + command + 渲染 + undo/redo + JSON round-trip），为后续图片上传/拖拽/resize 等上层能力打底。

**要实现什么**
- `gpui-plate-core`：
  - ImagePlugin：
    - `NodeSpec { kind: "image", role: Block, is_void: true }`
    - attrs：`src: string`（必填），`alt: string`（可选）
  - command：`image.insert({ src, alt? })`：
    - 插入一个 `image` block（void）到当前 block 之后
    - 自动插入一个空 paragraph 在 image 之后，并将 caret 移动到该 paragraph（保证插入后立刻可继续输入）
- `gpui-manos-plate`（view）：
  - 渲染：`image` 暂以占位卡片渲染（显示 “Image” + src/alt），后续可替换为真实图片渲染。
  - 删除体验：`Backspace`/`Delete` 在相邻段落边界可移除 `image` block（与 divider 一致）。
- `story`：
  - Toolbar：新增 “Insert image” 按钮，弹窗输入 URL 后调用 `image.insert`。

**怎么实现（关键做法）**
- core：
  - 复用 divider 的“block 后插入 void + paragraph”插入策略，确保 selection 始终落在 text block 上（避免 view 层需要处理“选区落在 void block 内”的复杂语义）。
- view：
  - 在 block renderer 中新增 `Node::Void(kind == "image")` 分支渲染占位卡片。
  - 在 `Backspace/Delete` 的“边界移除 divider”逻辑中同样覆盖 `image`。
- story：
  - 复用 link 的 dialog/input 模式：只负责获取 src 并触发 command，不直接触碰树结构。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 单测：`cargo test -p gpui-plate-core`
- 手动验收清单（Iteration 15 通过标准）：
  - **插入**：点击 toolbar 的 Image → 输入一个 URL → OK：
    - 编辑区出现 image 占位块（可看到 URL/alt 文本）。
    - 光标自动落到 image 下方的新段落，可继续输入。
  - **删除**：
    - 光标在 image 下方段落行首按 Backspace → image 被删除。
    - 光标在 image 上方段落行尾按 Delete → image 被删除。
  - **Undo/Redo**：插入/删除 image 都可被 undo/redo 正确回滚。
  - **JSON round-trip**：Save As → Open 后 image 节点仍存在且 attrs（src/alt）不丢。

### Iteration 16：Block Void Selection（点击选中 + 删除）

> 目标：补齐“block void 节点”的基础交互：可以像常见编辑器一样 **点击选中 image/divider**（有可见高亮），并用 Backspace/Delete 直接删除（不要求光标必须在相邻段落边界）。

**要实现什么**
- `gpui-manos-plate`（view）：
  - 新增 `block_bounds_cache`：在 prepaint 阶段为每个渲染出来的 block 记录 `Bounds + kind`，用于命中测试。
  - 新增 `selected_block_path`（会话态）：记录当前选中的 block void（目前覆盖 `image` / `divider`）。
  - Mouse：
    - 单击命中 `image/divider` → toggle 选中（不进入“拖拽选区”模式）。
    - 点击文本/拖拽选区/键盘移动光标 → 自动清除 block 选中态。
  - Key：
    - 当 `selected_block_path` 存在时：Backspace/Delete 删除该 block（`Op::RemoveNode`），并清空选中态。
  - Render：
    - `image/divider` 被选中时显示 ring outline（可视化确认）。

**怎么实现（关键做法）**
- 用 `RichTextBlockBoundsElement` 包裹 `render_block(...)` 的输出：不改变布局，只在 prepaint 时把 bounds 写回 `RichTextState.block_bounds_cache`。
- 在 `RichTextInputHandlerElement` 的 mouse down 中，优先用 `void_block_path_for_point` 命中 `image/divider`，命中则只更新 `selected_block_path` 并提前 return。
- `Backspace/Delete` 在进入文本删除逻辑前先处理 `selected_block_path`，保证行为可预测。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 16 通过标准）：
  - **点击选中**：插入一个 image 或 divider 后，用鼠标单击该块 → 出现高亮边框（ring）。
  - **键盘删除**：保持该块处于选中态：
    - 按 Backspace → 该块被删除。
    - Undo/Redo 可以正确回滚/重做。
  - **清除选中态**：点击任意文本位置/拖拽选择文本/用方向键移动光标 → block 高亮消失。

### Iteration 17：Block Void Clipboard（复制/剪切/粘贴 block void）

> 目标：补齐 block-void 的“可移动性”闭环：选中 `image/divider` 后可 **复制/剪切/粘贴**（Cmd/Ctrl+C/X/V），并用 gpui clipboard 的 JSON metadata 在编辑器内 **无损 round-trip**（跨应用则降级为纯文本）。

**要实现什么**
- `gpui-manos-plate`（view）：
  - 定义 clipboard payload：`gpui-manos-plate/fragment`（version: 1；JSON metadata；携带 `Vec<Node>`）。
  - Copy/Cut：
    - 若当前存在 `selected_block_path` 且命中 `image/divider`：写入 clipboard（metadata 为 fragment，text 为 Markdown fallback）。
    - Cut 额外移除该 block（`Op::RemoveNode`），可 undo/redo。
  - Paste：
    - 若 clipboard metadata 为 fragment（且 nodes 为支持的 block-void）：在当前 caret 所在 block 之后插入这些 nodes；
    - 若当前选中了 block-void：paste 会替换该 block；
    - 若插入点之后没有 text block，则自动插入空 paragraph 并把 caret 移动到该 paragraph（保证可继续输入）。

**怎么实现（关键做法）**
- 使用 `ClipboardItem::new_string_with_json_metadata(...)` 写入 payload，并在 paste 时用 `ClipboardString::metadata_json()` 解析。
- 保持“编辑器外粘贴”的可用性：image fallback 为 `![alt](src)`，divider fallback 为 `---`。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 17 通过标准）：
  - **复制/粘贴**：
    - 插入 image/divider → 单击选中（出现 ring）→ Cmd/Ctrl+C
    - 点击任意段落 → Cmd/Ctrl+V：应插入相同 image/divider（attrs 不丢）。
  - **剪切/粘贴移动**：
    - 选中 image/divider → Cmd/Ctrl+X：该 block 消失
    - 在另一处段落 → Cmd/Ctrl+V：该 block 出现在新位置
    - Undo/Redo 可回滚/重做上述操作。
  - **跨应用降级**：复制 image/divider 到系统剪贴板后，粘贴到其它文本编辑器：
    - image 应表现为 `![alt](src)` 或至少包含 `src`
    - divider 应表现为 `---`

### Iteration 18：Block Subtree Clipboard（Alt-click 选中任意 block 子树）

> 目标：从“只支持 block void”升级为“支持任意 block 子树”（`Node::Element | Node::Void`），覆盖 paragraph/heading/list_item 等文本 block，以及 blockquote/toggle/table 等容器 block：允许 Alt-click 选中整个 block，并对其进行复制/剪切/粘贴/删除；为后续 toolbar 的 “duplicate/move/convert block” 打底。

**要实现什么**
- `gpui-manos-plate`（view）：
  - Alt-click（不带 Shift/Cmd/Ctrl）命中并选中一个 block（支持循环选择）：
    - 同一位置重复 Alt-click 会在“容器 block → 更深层子 block → ... → 取消选中”之间循环；
    - 选择策略优先级：容器（table/blockquote/toggle…） > block void（image/divider…） > text block（paragraph/heading/list item…）。
  - `selected_block_path` 统一适配 `Node::Element | Node::Void`（不再限定 image/divider）。
  - Render：任何被选中的 block 都显示 ring outline（统一在 `RichTextBlockBoundsElement` 绘制，避免节点渲染特判）。
  - Copy/Cut/Paste：
    - Copy：若存在 `selected_block_path`：写入 `gpui-manos-plate/fragment`（version: 1；nodes: `Vec<Node>`）+ plain text fallback（递归收集文本；void 用 markdown fallback）。
    - Cut：Copy + `Op::RemoveNode`（可 undo/redo）。
    - Paste：若 clipboard metadata 为 fragment：
      - 若当前选中了一个 block：paste 替换该 block（remove + insert nodes）。
      - 否则：在当前 caret 所在 block 之后插入 nodes。
      - 若插入点之后没有可落 caret 的 text descendant：自动插入空 paragraph 并把 caret 移动到该 paragraph（保证可继续输入）。

**怎么实现（关键做法）**
- 在 `selectable_block_path_for_point` 中基于 `block_bounds_cache` + `is_text_block_kind` 做 scoring（容器优先、深度优先）。
- 统一高亮：`RichTextBlockBoundsElement::paint` 读取 `RichTextState.selected_block_path` 并绘制 ring outline。
- Clipboard payload 复用 Iteration 17 的 `PlateClipboardFragment`，但允许节点类型扩展为 `Element | Void`，并在 paste 中用 op 序列插入/替换。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 18 通过标准）：
  - **选中可见**：Alt-click paragraph / blockquote / toggle / table：
    - 对应 block 出现 ring 高亮；重复 Alt-click 可在同一位置的嵌套 block 间切换，最终可取消选中。
  - **复制/粘贴子树**：
    - Alt-click 选中一个 toggle 或 table → Cmd/Ctrl+C
    - 在其它位置 Cmd/Ctrl+V：应插入同结构的 toggle/table（内部子块完整、attrs 不丢）。
  - **剪切/粘贴移动**：
    - Alt-click 选中 blockquote/toggle/table → Cmd/Ctrl+X：该 block 消失
    - 在其它位置 Cmd/Ctrl+V：该 block 出现在新位置；Undo/Redo 可回滚。
  - **删除选中 block**：Alt-click 选中任意 block → Backspace/Delete 删除；Undo/Redo 可回滚。
  - **跨应用降级**：复制选中的 table/toggle 等粘贴到其它文本编辑器：至少得到其文本内容（按 `\\n` 拼接），void 节点按 markdown fallback 显示。

### Iteration 19：Block Actions（duplicate/move/delete 选中 block）

> 目标：让 Iteration 18 的 “block selection” 立刻产出用户可见价值：对选中 block 提供常见的块级操作（复制一份、上移/下移、删除），并通过 toolbar 暴露出来；同时验证这些操作在容器内（toggle/table cell）也能工作。

**要实现什么**
- `gpui-manos-plate`（view）新增基于 `selected_block_path` 的 block 操作：
  - `duplicate_selected_block`：clone 当前 `Node::Element | Node::Void`，插入到其后一个 sibling 位置（`Op::InsertNode`）。
  - `move_selected_block_up/down`：在同一 parent children 内做 remove+insert（注意索引变化），并把选中态移动到新位置。
  - `delete_selected_block`：复用现有 RemoveNode 能力（与 Backspace/Delete 一致）。
- `gpui-manos-components-story`（example）：
  - toolbar 新增 “Duplicate / Move Up / Move Down / Delete Block” 按钮：
    - 仅当存在 `selected_block_path` 时可用。
    - Move Up/Down 在到达边界时 disabled。
  - tooltip 明确：Alt-click 先选中 block，再执行操作。

**怎么实现（关键做法）**
- 在 view 内实现 `selected_block_parent_and_index()` 辅助函数：
  - 解析 `selected_block_path.split_last()` 得到 `(index, parent_path)`，并从 doc 获取 parent children 长度。
  - 构造 ops：`RemoveNode` + `InsertNode { path: parent_path + new_index }`（up/down 的插入 index 规则不同）。
- 选中态更新策略（避免被 `push_tx` 清空）：
  - 在 apply tx 前先计算 `selected_block_path_after`，在 apply 成功后重新写回并 `cx.notify()`（保证可连续多次 move/duplicate）。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 19 通过标准）：
  - **Duplicate**：Alt-click 选中一个 paragraph/toggle/table → 点击 Duplicate → 该 block 在其后复制一份；Undo/Redo 正确。
  - **Move**：Alt-click 选中一个 block → 点击 Move Up/Down → 该 block 在同一 parent 内上移/下移一格；边界按钮 disabled；Undo/Redo 正确。
  - **容器内可用**：在 toggle 内部、table cell 内对“子 block”执行上述操作同样成立（必要时重复 Alt-click 从容器切换到子 block；不会把 block 移出其父容器）。

### Iteration 20：Selection Range Clipboard（结构化选区剪贴板：保留 marks/inline void/多段结构）

> 目标：让“普通文本选区”的 Copy/Cut/Paste 在编辑器内部也能**无损**保留 rich 信息（marks、inline void、跨段落结构），而不是仅 plain text；同时不影响 Iteration 17/18 的 block clipboard 语义（Alt-click 选中 block 子树仍然走 block fragment）。

**要实现什么**
- Clipboard payload：继续使用 `gpui-manos-plate/fragment`（version: 1），新增 `mode` 字段：
  - `mode: block`（默认）：用于 Iteration 17/18 的 block subtree clipboard（Alt-click 选中 block）。
  - `mode: range`：用于普通选区 clipboard；payload `nodes` 为一组 “text block 片段”（每个节点是 `Node::Element`，其 children 为被选中的 inline 子序列）。
- Copy/Cut（普通选区）：
  - Cmd/Ctrl+C：当不存在 `selected_block_path` 且选区非 collapsed：写入 `mode: range` fragment（保留 marks/inline void/多段结构），并在 string 部分保留 plain text fallback（跨应用可粘贴）。
  - Cmd/Ctrl+X：Copy + 删除选区（保持与现有 delete selection 逻辑一致）。
- Paste（普通选区）：
  - 若 clipboard 为 `mode: range`，且未选中 block：在当前 caret 所在 text block 内按 fragment 插入（必要时拆分为前缀/后缀块），并将光标置于“插入内容末尾”。
  - 若 clipboard 为 `mode: range` 但当前存在 `selected_block_path`：按 block 语义替换选中 block（与 `mode: block` 一致）。

**怎么实现（关键做法）**
- Copy：将当前 selection 映射为一组 `text_block_paths`（深度优先顺序），对首/尾 block 做 offset slice（复用 `split_inline_children_at_offset`，确保 inline void 原子性），将 slice 后的 children 写入 fragment 的 `nodes`（保留 `TextNode.marks` 与 `VoidNode.attrs`）。
- Paste：
  - `mode: range`：
    - 1 block 且 kind 与插入点 block 相同：直接在该 block children 内合并（prefix + inserted + suffix）。
    - 多 block 或 kind 不同：将插入点 block 作为“切分锚点”，构造 `RemoveNode + InsertNode*` 把它替换为（prefix block?）+ fragment blocks +（suffix block?），并计算 caret point（若存在 suffix，caret 停在 suffix 之前）。
  - `mode: block`：复用现有 block fragment paste（插入/替换 block 子树 + 保证后继可落 caret）。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 20 通过标准）：
  - **marks 无损**：输入一段文字，分别设置 bold/italic/underline/strike/code/link/color → 选中其中一部分 Cmd/Ctrl+C → 在另一处 Cmd/Ctrl+V：应保留 marks（不是只粘 plain text）。
  - **inline void 无损**：插入 mention（或其它 inline void）→ 框选包含 mention 的范围 Cmd/Ctrl+C/V：mention 仍为 inline void（不是退化为 `@label` 文本）。
  - **多段结构**：选中跨 2~3 段（含不同 marks）Cmd/Ctrl+C → 在一段中间粘贴：应产生多段（段落边界保留），且 marks 不丢；光标位于插入内容末尾（若原段有后缀文本，光标应在后缀文本之前）。
  - **跨应用降级**：复制一段带 marks/mention 的内容，粘贴到其它纯文本编辑器：应至少得到 plain text（mention 以 `@label` 或 `□` 降级）。

### Iteration 21：Code Block（块级代码块）

> 目标：新增一个典型的“块级 text block”能力：`code_block`（单独 kind + attrs），并通过插件系统提供 schema/command/query；view 层给出明显的渲染样式（mono + container）；工具层（toolbar）以 “Block type” 下拉菜单方式暴露（与 heading/paragraph 同一入口）。

**要实现什么**
- `gpui-plate-core`（core）：
  - 新插件：`CodeBlockPlugin`：
    - node spec：`code_block`（Block + InlineOnly）
    - command：`code_block.toggle`
    - query：`code_block.is_active`
  - toggle 语义：在当前 text block 上切换 `paragraph/heading/list_item/...` ↔ `code_block`（并清理不再有意义的 attrs，如 heading `level`、list `list_*`、todo `checked`）。
- `gpui-manos-plate`（view）：
  - 渲染：`code_block` 使用 mono 字体并显示 container（背景/边框/圆角），与普通 paragraph/heading 形成明显区分。
  - 命中测试：点击在 code block 的 container 内（包含 padding）也应落到该 text block（保证体验不被 padding 削弱）。
- `gpui-manos-components-story`（example）：
  - “Block type” 下拉菜单新增 “Code Block” 选项，并在 code block 激活时显示为当前选中项。

**怎么实现（关键做法）**
- core：通过新增 `NodeSpec + CommandSpec + QuerySpec` 完成“无侵入扩展”（不需要修改 Editor 内核）。
- view：在 `render_text_block` 对 `el.kind == "code_block"` 应用专属 base text style，并使用容器元素包裹；`text_block_path_for_point` 在无法命中精确 text bounds 时，回退到 block bounds（仅限 text block kind）以覆盖 padding/前缀区域。
- story：下拉菜单点击时确保先退出 code block 再设置 heading（或反之），从而保持语义一致、命令可组合。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 21 通过标准）：
  - **创建/取消 code block**：在任意段落中打开 “Block type” → 选择 “Code Block”：
    - 该段应切换为 code block；再次选择 “Paragraph” 应恢复为普通段落。
  - **渲染区分明显**：code block 为 mono 字体，且有背景/边框容器。
  - **点击容器可落光标**：点击 code block 的背景/padding 区域也能把光标放进该 block（不是只能点到文字）。
  - **与 heading 互转**：Heading 1/2/3 → Code Block（attrs 不应残留 `level`）；Code Block → Heading（应先回到 paragraph 再 set heading）。

### Iteration 22：Block Align（段落对齐：Left/Center/Right）

> 目标：补齐一个典型“块级 attrs + 命令/查询 + 纯渲染映射”的插件能力：对齐（Left/Center/Right）。这类功能在 Plate/Slate 生态中通常属于“块级属性驱动渲染”，非常适合用插件系统的 schema/command/query/normalize 来承载（避免 view/state 里散落 if/match）。

**要实现什么**
- `gpui-plate-core`（core）：
  - 新插件：`AlignPlugin`：
    - 命令：`block.set_align`（args：`{ "align": "left" | "center" | "right" }`）
    - 查询：`block.align`（返回 `null | "center" | "right"`；`null` 视为 `left` 缺省态）
    - normalize：保证 attrs canonical（`left` 作为缺省态：`align == "left"` 时移除该属性；非法值移除）。
- `gpui-manos-plate`（view）：
  - 渲染：读取 text block attrs `align`，映射到 `TextStyle.text_align`（paragraph/heading/code_block/list_item/todo_item 等所有 text block）。
  - wrapper：新增 `command_set_align(BlockAlign)` 与 `block_align()`（供 toolbar 使用）。
- `gpui-manos-components-story`（example）：
  - toolbar 新增 Align 下拉菜单（复用 `PlateToolbarDropdownButton + Popover` 模式）：
    - icon 根据当前 align 展示（Left/Center/Right）。
    - 点击 item 执行 `block.set_align`，并在执行后将 focus 还给 editor（与其它 toolbar 行为一致）。

**怎么实现（关键做法）**
- core 命令按 **selection 范围内的 text blocks** 批量设置（与 indent 一致），保证多段选区也能一键对齐。
- view 渲染只做纯映射：attrs → `TextStyle.text_align`，不引入额外 state；对齐属于可序列化、可撤销的文档态。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 22 通过标准）：
  - **单段对齐**：光标在段落内 → Align 菜单依次选择 Left/Center/Right：文本对齐实时变化；Undo/Redo 正确。
  - **多段对齐**：跨 2~3 段落框选 → 选择 Center：所有被选段落均居中；再次选择 Right/Left 同理。
  - **不同 block kind**：对 heading、list item、todo item、code block 做对齐：应同样生效（list marker 不必跟随对齐，但文本区域对齐要正确）。
  - **持久化**：Open/Save/Save As 保存为 JSON 后重新打开：对齐状态应被保留。

### Iteration 23：Block Font Size（字号：A-/A/A+）

> 目标：补齐旧 toolbar 中高频的“字号”能力，并验证一种典型的“块级 attrs（可序列化/可撤销）→ 纯渲染映射”路径可以完全走插件体系落地（command/query/normalize/render），而不是在 view/state 中硬编码特例。

**要实现什么**
- `gpui-plate-core`：已具备 `FontSizePlugin`（`block.set_font_size` / `block.unset_font_size` / `block.font_size` + normalize），本迭代不新增底层结构。
- `gpui-manos-plate`（wrapper/view）：
  - wrapper：对外暴露 `command_set_font_size(u64)` / `command_unset_font_size()` / `block_font_size() -> Option<u64>`（供工具层使用）。
  - 渲染：text block 读取 `font_size` attrs 并映射到 `TextStyle.text_size`（heading 可覆盖其默认字号）。
- `gpui-manos-components-story`（example）：
  - toolbar 增加字号控件（推荐与旧 UI 对齐为 A-/A/A+ 三段）：
    - A-：字号 -1（clamp 到 `8..=72`），执行 `block.set_font_size`。
    - A：重置（执行 `block.unset_font_size`，回到缺省字号/heading 默认字号）。
    - A+：字号 +1（clamp 到 `8..=72`），执行 `block.set_font_size`。
  - 当存在 block selection（Alt-click 选中 block 子树）时，字号控件应 disabled（避免对“块选中态”产生误导）。

**怎么实现（关键做法）**
- toolbar 显示与行为的字号基准：
  - 若 `block.font_size` 返回 `Some(size)`，则显示/增减基于该值；
  - 若返回 `None`：
    - heading：基于 heading 的默认字号（与 view 渲染一致）作为增减基准；
    - 其它 text block：使用 editor 默认字号（可先取 16）作为增减基准。
- 保持工具层“只依赖 command/query”：字号控件不直接读写 doc；仅调用 wrapper 命令并在执行后把 focus 还给 editor。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 23 通过标准）：
  - **单段字号**：光标在 paragraph 内点击 A+/A-：字号变化明显；Undo/Redo 正确。
  - **重置**：点击 A：回到缺省字号（heading 回到其默认字号），Undo/Redo 正确。
  - **跨段落范围**：跨 2~3 段落拖拽选区后点 A+：所有被选段落字号都变化（命令按 selection 范围内 text blocks 批量设置）。
  - **表格 cell**：在 table cell 内做字号调整与重置，行为与普通段落一致。
  - **持久化**：Save As → Open 后字号设置不丢失（JSON round-trip）。

### Iteration 24：Link UX（Set Link/Unlink：可靠可见的链接编辑）

> 目标：把 Link 的用户体验补齐到“符合常见编辑器直觉”的程度，解决“Set Link 看起来没生效”的反馈；同时补齐旧 toolbar 中常见的 Unlink 快捷按钮。

**要实现什么**
- `gpui-manos-plate`（wrapper/view）：
  - 改进 `command_set_link(url)` 行为（不改变 core 的 marks 语义，只改 UX 策略）：
    1) 若当前 selection 非 collapsed：对选区范围应用 link（现状保持）。
    2) 若 collapsed 且 caret 位于 link 内：选中该 link run 的连续范围并更新 URL（避免只改到一小段）。
    3) 若 collapsed 且 caret 不在 link 内：
       - 优先选中光标附近“词”并应用 link（给出立即可见反馈）；
       - 若找不到可用范围（空段/纯空白）：插入 URL 文本并带 link marks（确保用户看到结果）。
  - `command_unset_link()` 不变。
- `gpui-manos-components-story`（example）：
  - toolbar 增加 Unlink 按钮（仅当 link active 时可用），并在 block selection（Alt-click 选中 block 子树）时禁用 Link/Unlink。

**怎么实现（关键做法）**
- “link run” 的范围判定不依赖 layout cache：直接按 block 的 inline children 扫描 `TextNode.marks.link`，以相同 URL 的连续段为一个 run（允许在 run 内混入 bold/italic 等其它 marks）。
- “找词”仍使用现有的 `word_range + boundary` 近似规则（中文边界仍然不完美，属于已记录问题范畴），但确保在无法命中时落到 “插入 URL” 的兜底路径。
- 插入 URL 使用一次性 transaction（避免 “先 set marks 再 insert text” 导致 Undo 需要按两次）。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 24 通过标准）：
  - **对选区 Set Link**：框选一段文字 → 点击 Link → 输入 URL → OK：选区变为蓝色下划线；Undo/Redo 正常。
  - **对 caret 附近 Set Link（选词）**：光标落在一个英文单词中间 → Set Link：应把该单词整段变为链接（不要求中文分词完美）。
  - **空段落 Set Link（插入兜底）**：光标在空 paragraph → Set Link：应插入 URL 文本并带链接样式；Undo 一次即可撤销该插入。
  - **编辑已存在链接**：
    - 让一段链接文本中间包含不同 marks（例如部分加粗）；
    - 将 caret 放在链接中间 → Set Link 输入新 URL：整段连续链接（含加粗部分）URL 都应被更新。
  - **Unlink**：当 link active 时，点击 Unlink：该链接范围应被移除（颜色恢复）；Undo/Redo 正常。

### Iteration 25：Heading Menu（H4/H5/H6：补齐 block type 下拉）

> 目标：`HeadingPlugin` 的 schema/command/query 已支持 `level: 1..=6`，但示例 UI 只暴露了 H1~H3；本迭代将 block type 下拉补齐 H4/H5/H6，让“能力支持范围”与“可发现的 UI”一致。

**要实现什么**
- `gpui-manos-components-story`（example）：
  - Block type 下拉菜单新增 `Heading 4/Heading 5/Heading 6` 三个条目，行为与 H1~H3 完全一致：
    - 若当前是 code block：先 toggle 回 paragraph，再 set heading level（保持互转规则一致）。
    - 点击后 focus 还给 editor。

**怎么实现（关键做法）**
- 不新增新的 core API：复用现有 `command_set_heading(level)` 与 `heading_level()` query。
- 仅补齐 UI 暴露面，避免引入新的“半功能状态”。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 25 通过标准）：
  - **菜单项可用**：Block type 下拉中出现 Heading 4/5/6。
  - **设置与渲染**：选择 Heading 4/5/6 后，渲染字号/字重变化明显；再次切换 Paragraph 可恢复。
  - **与 code block 互转**：Heading 5 → Code Block → Heading 6：不应残留 `level` 等不相干 attrs；Undo/Redo 正常。
  - **JSON round-trip**：Save As → Open 后 heading level 不丢失。

### Iteration 26：Command Palette（命令面板：从 CommandRegistry 自动发现并执行）

> 目标：验证“插件命令 = 可发现的工具能力”：不再把工具层能力写死在 toolbar 上，而是通过 `CommandRegistry` 自动发现命令，并提供一个可搜索的命令面板（Command Palette）来执行；这也是 Plate/Slate 生态中非常常见的 UX（并为未来的“插件贡献工具项/菜单项”铺路）。

**要实现什么**
- `gpui-manos-plate`（wrapper）：
  - 提供 `command_list()`（供工具层展示命令列表）。
  - 提供 `command_run(id, args_json)`（允许工具层以 string id + 可选 args 执行任意命令）。
- `gpui-manos-components-story`（example）：
  - 新增 “Command Palette” 入口（toolbar 按钮 + Edit 菜单项），打开一个对话框：
    - Search input：按 `id/label` 过滤命令。
    - Args input：可选 JSON（空则 `None`），用于执行需要 args 的命令（如 `block.set_heading` / `table.insert` / `marks.set_link`）。
    - 命令列表：点击即可执行该命令；执行成功后关闭 dialog 并把 focus 还给 editor；失败则以 Notification 提示错误并保持 dialog 打开。

**怎么实现（关键做法）**
- 不要求 core 为每条命令声明 args schema（当前阶段以“通用 JSON args”先打通闭环）；但 **不牺牲底层**：命令执行仍然只走 `Editor::run_command`，不会绕过 normalize/undo。
- 过滤策略采用简单 case-insensitive substring（后续可升级为 fuzzy，属于纯工具层改进，不影响内核）。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 26 通过标准）：
  - **打开面板**：点击 toolbar “Command Palette” 或菜单 `Edit → Command Palette` 会出现对话框。
  - **搜索过滤**：输入 `bold`/`table`/`toggle` 等关键字，列表会即时过滤。
  - **无 args 命令**：留空 args，点击 `core.insert_divider`、`marks.toggle_bold` 等可立即生效；Undo/Redo 正常。
  - **带 args 命令**：
    - 输入 args `{"level":4}` 后点击 `block.set_heading`：应切到 Heading 4。
    - 输入 args `{"rows":2,"cols":3}` 后点击 `table.insert`：应插入 2×3 表格。
    - 输入 args `{"url":"https://example.com"}` 后点击 `marks.set_link`：应把当前选区/光标附近内容设置为 link（配合 Iteration 24 的 UX）。
  - **错误提示**：args JSON 非法或缺参时应提示 Notification，且 dialog 不应关闭。

### Iteration 27：Columns（多列容器 + 拖拽调整列宽）

> 目标：补齐旧示例里辨识度很强的 “columns/分栏” 能力，并验证新架构对 “可交互布局状态（列宽）= 文档 attrs + 单次可撤销操作” 的支撑；同时为后续更复杂的布局插件（callout、layout grid、嵌套容器）打底。

**要实现什么**
- `gpui-plate-core`（model + plugin）：
  - 新增 `ColumnsPlugin`：
    - kinds：`columns`、`column`（schema：Block + BlockOnly）
    - attrs（columns）：`widths: number[]`（长度=列数，sum≈1）
    - commands：
      - `columns.insert`（args：`{ "columns": 2..6 }`）：插入 columns（默认 2 列）并把光标放进第一列首段
      - `columns.unwrap`：移除最近 columns，把列内 blocks 按列顺序摊平回父容器
      - `columns.set_widths`（args：`{ "path": number[], "widths": number[] }`）：设置指定 columns 的列宽（用于拖拽提交）
    - query：`columns.is_active`
    - normalize：
      - columns 至少 2 列；每个 column 至少 1 个 paragraph
      - widths 不合法/长度不匹配时自动修复为平均分配，并保证 sum≈1
- `gpui-manos-plate`（view）：
  - 渲染 `columns`：横向 flex 布局，列间显示可拖拽的 resize handle。
  - 拖拽交互：
    - 拖拽时只更新 view 的 “preview widths”（不写入 doc，不污染 undo 栈）。
    - mouse up 时通过 `columns.set_widths` 一次性提交（undo/redo 为 1 步）。
- `gpui-manos-components-story`（example）：
  - toolbar 增加 Columns 插入/移除入口（并保持 focus 还给 editor）。

**怎么实现（关键做法）**
- 核心原则：**拖拽是连续交互，但提交必须是单次 Transaction**（避免 undo 栈被 mouse move 填满）。
- view 侧用 `block_bounds_cache` 获取各列真实 bounds 来计算拖拽 delta（兼容 handle 宽度/间距，不依赖“纯数学推导”）。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 单测：`cargo test -p gpui-plate-core`
- 手动验收清单（Iteration 27 通过标准）：
  - **插入**：点击 Columns 按钮后出现 2 列；光标在第一列可直接输入；Tab/方向键/鼠标点击可在两列间移动光标。
  - **列宽拖拽**：拖拽列间 handle 可实时调整两侧列宽；mouse up 后生效并保持光标/选区不丢。
  - **Undo/Redo**：一次拖拽提交应只产生 1 个 undo step（Undo 回滚列宽，Redo 恢复）。
  - **JSON round-trip**：Save As → Open 后 columns 结构与 widths 不丢失、展示一致。

### Iteration 28：Command Metadata（keywords/description/args example）+ Command Palette Pro

> 目标：让“插件命令”不仅可执行，还**可发现、可检索、可理解**。本迭代在不牺牲底层原则（命令执行仍只走 `Editor::run_command`）的前提下，为命令引入更丰富的 UI-neutral metadata，并将其用于命令面板的键盘优先体验。
>
> 状态：已实现（`crates/plate-core/src/plugin.rs`、`crates/rich_text/src/state.rs`、`crates/story/src/richtext.rs`）。

**要实现什么**
- `gpui-plate-core`：
  - 扩展 `CommandSpec`（保持向后兼容的默认值）：
    - `description: Option<String>`（更长说明）
    - `keywords: Vec<String>`（用于搜索；例如 bold/strong/mark）
    - `args_example: Option<serde_json::Value>`（给 command palette 的参考输入）
    - `hidden: bool`（内部命令不出现在 palette）
  - 选择性为现有命令补齐 metadata（至少覆盖 toolbar 已暴露的命令）。
- `gpui-manos-plate`：
  - `command_list()` 返回 `id/label/description/keywords/args_example`（按需）。
- `gpui-manos-components-story`：
  - Command Palette 升级：
    - 支持键盘上下选择、Enter 执行、Esc 关闭。
    - 搜索同时匹配 `id/label/keywords`（可保留 substring，后续再升级 fuzzy）。
    - 选中命令时显示 `description` 与 `args_example`（一键填充到 args input）。
    - 增加快捷键：macOS `Cmd+Shift+P`，Windows/Linux `Ctrl+Shift+P`。

**怎么实现（关键做法）**
- metadata 保持 **UI-neutral**：core 不依赖 icon/UI 类型，tool 层自行决定展示。
- command palette 的键盘交互不影响 editor 输入：打开 dialog 时 focus 在 search input；执行成功后 focus 还给 editor。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 28 通过标准）：
  - **快捷键打开**：`Cmd/Ctrl+Shift+P` 打开命令面板。
  - **键盘执行**：上下键选择命令，Enter 执行并关闭；Esc 关闭不执行。
  - **metadata 可见**：选中 `table.insert` 等命令时能看到 args 示例并可一键填充；执行失败时仍提示 Notification 且 dialog 不关闭。

### Iteration 29：Table Pro（row above / col left / delete table）+ Toolbar Menus

> 目标：把 table 的编辑能力补齐到“像个编辑器”：不止能插入 table，还要能在任意位置 **向上/向下插入行、向左/向右插入列、删除整张表**，并把 toolbar 从“堆按钮”升级为 **Row/Column 两个菜单**（更接近 plate 的工具结构，也更可扩展）。
>
> 状态：已实现（`crates/plate-core/src/plugin.rs`、`crates/rich_text/src/state.rs`、`crates/story/src/richtext.rs`）。

**要实现什么**
- `gpui-plate-core`：
  - 新增 table commands：
    - `table.insert_row_above`
    - `table.insert_col_left`
    - `table.delete_table`
- `gpui-manos-plate`：
  - 新增 wrapper API：`command_insert_table_row_above`、`command_insert_table_col_left`、`command_delete_table`
- `gpui-manos-components-story`：
  - toolbar 的 table 区域升级为两个下拉菜单：
    - Row：Insert row above / Insert row below / Delete row / Delete table
    - Column：Insert column left / Insert column right / Delete column
  - 仅当 `table.is_active` 时可用（disabled 状态正确），执行后 focus 还给 editor。

**怎么实现（关键做法）**
- core 侧保持最终架构原则：仍然只通过 `Transaction { ops }` 修改文档；selection_after 统一由 command 决定（避免 view 侧再做“二次修正”）。
- row/col insert 对 table 的所有 row 做结构化修改，保证表格始终保持矩形（normalize 仍可兜底）。
- delete table 采用“remove + insert paragraph 同位替换”，保证 block parent 结构合法且选区有落点。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 单测：`cargo test -p gpui-plate-core`
- 手动验收清单（Iteration 29 通过标准）：
  - **Row menu**：光标在 table 内，打开 Row 菜单：
    - Insert row above / below：能插入新行，光标落在新行对应 cell 中，Undo/Redo 正常。
    - Delete row：能删除当前行；删除到只剩 1 行时仍能继续删除（table 会被替换为 paragraph）。
    - Delete table：整张 table 被替换为 paragraph，光标落在 paragraph，Undo/Redo 正常。
  - **Column menu**：光标在 table 内，打开 Column 菜单：
    - Insert column left / right：能插入新列，所有行列数一致，Undo/Redo 正常。
    - Delete column：能删除当前列；删除到只剩 1 列时仍能继续删除（table 会被替换为 paragraph）。
  - **非 table 禁用**：光标不在 table 内时 Row/Column 菜单为 disabled。

### Iteration 30：Toolbar Layout Pro（横向滚动，避免按钮被截断）

> 目标：随着 toolbar 功能变多，窄窗口下不能再“静默裁剪”右侧按钮；用户必须始终能访问完整工具集（哪怕需要滚动）。
>
> 状态：已实现（`crates/story/src/richtext.rs`）。

**要实现什么**
- `gpui-manos-components-story`（example）：
  - `richtext` toolbar 容器固定为 `w_full`，并开启横向滚动（overflow-x scroll）。

**怎么实现（关键做法）**
- 将 toolbar 根容器设置为：`.id("richtext-toolbar").w_full().overflow_x_scroll()`（`overflow_*_scroll` 需要 stateful element），避免子元素撑开导致“滚动容器失效”。
- 不改动任何 core/view/插件逻辑：这是纯工具层 UI 体验修复。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 30 通过标准）：
  - **窄窗口可访问全部按钮**：将窗口宽度缩到不足以显示全部 toolbar 按钮时，toolbar 可横向滚动访问右侧按钮（触控板横向滚动；鼠标可尝试 Shift+滚轮）。
  - **不影响编辑体验**：横向滚动 toolbar 不应影响编辑区输入、选择、快捷键与 Undo/Redo。

### Iteration 31：Emoji Inline Void（emoji 节点 + 插入命令 + toolbar 入口）

> 目标：在不引入新依赖的前提下，用一个“足够小但完整闭环”的能力验证：**新增一个 inline void 节点**（emoji），并打通 command/serialize/copy/paste/tooling 全链路。这类能力是 plate/slate 插件生态里非常常见的基础积木（mention/emoji/tag/variable…）。
>
> 状态：已实现（`crates/plate-core/src/plugin.rs`、`crates/plate-core/src/core.rs`、`crates/rich_text/src/state.rs`、`crates/story/src/richtext.rs`）。

**要实现什么**
- `gpui-plate-core`：
  - 新增 `EmojiPlugin`：
    - kind：`emoji`（inline void）
    - attrs：`emoji: string`（例如 `😀`、`🥳`）
    - command：`emoji.insert`（args：`{ "emoji": "😀" }`；无参默认 `😀`）
  - `VoidNode::inline_text/inline_text_len` 支持 `emoji`（用于渲染、hit-test、plain text、selection snapping）。
- `gpui-manos-plate`：
  - 新增 wrapper：`command_insert_emoji(emoji, cx)`（对外保持“命令驱动”的调用方式）。
- `gpui-manos-components-story`：
  - toolbar 新增 Emoji 按钮（`Smile`），打开 dialog 输入 emoji 字符并插入；执行后 focus 还给 editor。

**怎么实现（关键做法）**
- emoji 作为 inline void：不允许光标落在其内部；offset 落入 void “中间”时按最近边界 snap（复用现有 void 原子化逻辑）。
- attrs 直接存 JSON：不引入 UI 类型或 emoji 库；未来如需 shortname/skin tone/搜索，可在工具层独立增强。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 单测：`cargo test -p gpui-plate-core`
- 手动验收清单（Iteration 31 通过标准）：
  - **插入**：点击 toolbar Emoji → 输入 `😀` → OK，emoji 出现在光标处，Undo/Redo 正常。
  - **原子性**：鼠标点击/拖拽选区穿过 emoji 时，光标不会落在 emoji“内部”；Backspace/Delete 会以“整颗节点”为单位删除它。
  - **复制/粘贴**：选中包含 emoji 的文本后 `Cmd/Ctrl+C`、在同编辑器内粘贴应保留 emoji；跨应用粘贴至少能得到 plain text 的 emoji 字符。
  - **JSON round-trip**：Save As → Open 后 emoji 节点不丢失、显示一致。

### Iteration 32：Link Shortcut Pro（Cmd/Ctrl+K 打开 Set Link）

> 目标：把“链接”能力从“只能点 toolbar”提升到更像编辑器的交互：`Cmd/Ctrl+K` 打开 Set Link（业界惯例），并在菜单中提供入口，方便发现与验收。
>
> 状态：已实现（`crates/story/src/app_menus.rs`、`crates/story/src/richtext.rs`）。

**要实现什么**
- `gpui-manos-components-story`（example）：
  - 新增 action：`SetLink`。
  - keybinding：macOS `Cmd+K`；Windows/Linux `Ctrl+K`（仅在 `gpui_manos_plate::CONTEXT` 下生效）。
  - Edit 菜单新增 `Set Link...`（触发同一 action）。
  - action handler：打开现有 Set Link dialog，并自动填充当前 selection/caret 下的 link URL（如存在）。

**怎么实现（关键做法）**
- 保持“命令驱动”：dialog 仅收集 URL，最终仍调用 `RichTextState::command_set_link/command_unset_link`（底层仍走 `marks.set_link/marks.unset_link`）。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 32 通过标准）：
  - **快捷键打开**：在编辑器聚焦时按 `Cmd/Ctrl+K` 打开 Set Link dialog。
  - **编辑已存在链接**：光标在 link 内按 `Cmd/Ctrl+K`，对话框应自动填充当前 URL；修改后 OK 生效，Undo/Redo 正常。
  - **菜单入口**：Edit 菜单 `Set Link...` 可打开同一对话框，并行为一致。

### Iteration 33：Media/Mention UX Pro（Image 真渲染 + Mention 自定义 label）

> 目标：把 “看起来没反应” 的上层体验补齐到可交付状态：
> - Image：不仅是占位卡片，而是能**真实加载并渲染**（失败时有 fallback），并支持 `Cmd/Ctrl+Click` 打开 `src`。
> - Mention：不再硬编码插入 `Alice`，而是提供 label 输入（dialog），验证 “inline void 的可配置插入”。
>
> 状态：已实现（`crates/rich_text/src/state.rs`、`crates/story/src/richtext.rs`）。

**要实现什么**
- `gpui-manos-plate`（view）：
  - image 渲染：`Node::Void(kind == "image")` 使用 gpui `img(src)` 实际渲染（loading/fallback），并保留 caption（src/alt）。
  - 交互：对 image block 支持 `Cmd/Ctrl+Click` 打开 `src`（不影响普通点击的 block 选中语义）。
- `gpui-manos-components-story`（example）：
  - Mention toolbar：点击后打开 dialog 输入 label，OK 后插入（仍走 `mention.insert` 命令）。

**怎么实现（关键做法）**
- Image 仍是 block void：编辑语义不变，只提升渲染与交互；底层仍通过 attrs `src/alt` 做无损持久化。
- Mention dialog 只负责收集 label：最终调用 `RichTextState::command_insert_mention`（底层仍走 command/undo/serialize）。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 33 通过标准）：
  - **Image 真渲染**：插入 image（输入一个可访问的 png/jpg/gif/webp 或 svg URL）后，image 区域应显示实际图片；加载中/失败时显示 fallback。
  - **打开 src**：对 image 区域按 `Cmd/Ctrl+Click` 应打开该 `src`（浏览器）。
  - **Mention 自定义**：点击 toolbar Mention → 输入 `@bob` → OK，应插入 mention（显示 `@bob`）；Undo/Redo 正常；Save As → Open 后 label 不丢失。

### Iteration 34：Find Highlight Pro（Cmd/Ctrl+F 查找 + 高亮匹配）

> 目标：引入一个“纯派生的 overlay/decoration”能力，用于搜索高亮、拼写检查、诊断提示等；本迭代用 Find（查找）做端到端验证：**不修改文档、不进入 undo 栈**，仅在视图层绘制高亮。
>
> 状态：已实现（`crates/rich_text/src/state.rs`、`crates/story/src/app_menus.rs`、`crates/story/src/richtext.rs`）。

**要实现什么**
- `gpui-manos-plate`（view）：
  - 在每个 text block line 上支持绘制 “find matches” 高亮（半透明背景），并保证 selection/caret 仍然清晰可见。
  - find query 属于 session state：不进入 JSON、也不进入 undo/redo。
- `gpui-manos-components-story`（example）：
  - 新增 action `Find`：
    - 快捷键：macOS `Cmd+F`、Windows/Linux `Ctrl+F`（仅在编辑器 key context 生效）。
    - Edit 菜单新增 `Find...` 入口。
  - 打开 Find dialog（输入 query），输入变化会实时更新编辑器高亮；清空 query 可取消高亮。

**怎么实现（关键做法）**
- 使用现有 `RichTextLineElement::paint_selection_range` 的布局计算能力复用绘制逻辑：同样按 `TextLayout` wrap 分段，避免自己重新实现换行与对齐。
- Find matches 使用 “block_path -> Vec<Range<usize>>（byte offsets）” 缓存，文档变更时刷新（不依赖 layout_cache）。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 34 通过标准）：
  - **打开 Find**：在编辑器聚焦时按 `Cmd/Ctrl+F`，或 Edit 菜单 `Find...`，打开对话框。
  - **高亮匹配**：输入 `Heading` / `task` / `@` 等文本，编辑区所有匹配处出现高亮；输入 `😀` 可高亮 emoji 文本（作为 inline void 的 display text）。
  - **清空关闭**：将 query 清空，高亮应立刻消失；关闭对话框不影响编辑器输入与快捷键。
  - **不影响 undo**：输入/删除文本的 undo/redo 语义不受 find 高亮影响（find 不产生 undo step）。

### Iteration 35：Find Navigation Pro（Next/Prev + current/total）

> 目标：让 Find 从“能高亮”升级为“可用的查找体验”：支持 next/prev 导航、展示匹配统计，并区分“当前匹配”高亮。
>
> 状态：已实现（`crates/rich_text/src/state.rs`、`crates/story/src/app_menus.rs`、`crates/story/src/richtext.rs`）。

**要实现什么**
- `gpui-manos-plate`（view）：
  - 维护 active match（block_path + range），并提供 `find_next/find_prev/find_stats`（不修改 doc，不进 undo）。
  - 渲染层：当前匹配使用更强的高亮色（区别于其他匹配）。
- `gpui-manos-components-story`（example）：
  - 新增 actions：`FindNext` / `FindPrev`：
    - 快捷键：macOS `Cmd+G` / `Cmd+Shift+G`；Windows/Linux `Ctrl+G` / `Ctrl+Shift+G`
    - Edit 菜单增加 `Find Next` / `Find Previous`
  - Find dialog：
    - 显示 `current/total`（例如 `3 / 10 matches`）
    - 提供 Next/Prev 按钮（不关闭 dialog），并与快捷键行为一致

**怎么实现（关键做法）**
- match 顺序由 `text_block_order` 决定：跨容器/表格也能稳定遍历。
- 初次导航以 caret/selection focus 作为锚点：如果 caret 在某个匹配内，next/prev 会先命中当前匹配；之后按 active match 继续遍历并支持 wrap-around。
- 导航仅更新 selection（collapsed 到 match.start）与 active match：不修改 doc、不产生 undo step。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 35 通过标准）：
  - **导航快捷键**：打开 Find 输入 query 后（Find input 聚焦也可），按 `Cmd/Ctrl+G` 依次跳到下一匹配并 wrap 到首个；`Cmd/Ctrl+Shift+G` 反向遍历。
  - **统计与当前高亮**：Find dialog 显示 `current/total`；当前 match 高亮更明显（且 selection/caret 仍清晰）。
  - **不影响 undo**：find next/prev 不产生 undo step；输入/删除仍可正常 undo/redo。

### Iteration 36：Image Insert UX Pro（File picker + 本地图片渲染）

> 目标：把 image 从“能插入一个 void block”升级为“可用的媒体插入体验”：默认从文件选择器插入本地图片，同时保留 URL 插入路径；并确保本地图片可可靠渲染与打开。
>
> 状态：已实现（`crates/story/src/richtext.rs`、`crates/rich_text/src/state.rs`）。

**要实现什么**
- `gpui-manos-components-story`（example）：
  - toolbar 的 Image 按钮：Click 打开文件选择器，选中文件后插入 image；Shift+Click 打开原有 “Insert Image（URL）” 对话框。
- `gpui-manos-plate`（view）：
  - 插入 image 时对 `~/` 做展开，并在可用时 canonicalize（保持 doc 内 `src` 的稳定性）。
  - 渲染 image 时：`src` 为绝对路径且存在 → 使用 `img(PathBuf)`（文件系统资源）；否则使用 `img(String)`（URL/embedded）。
  - `Cmd/Ctrl+Click` 打开 image：URL 直接打开；本地绝对路径用 `file://...` 打开。

**怎么实现（关键做法）**
- 利用 gpui `img()` 的 `ImageSource`：`PathBuf` 会走 `Resource::Path`，而纯字符串会走 `Resource::Uri/Embedded`，从而区分“本地文件”与“URL/资源键”。
- 该迭代只改 view/example：不引入新的 core 节点/operation，也不改变 undo/redo 语义。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 36 通过标准）：
  - **文件插入**：点击 toolbar 的 Image 按钮，弹出文件选择器；选择一张图片后，编辑器插入 image block，并将光标放到其后的新段落。
  - **URL 插入**：Shift+Click Image 按钮，输入一个 `https://...` 图片 URL 并确认，可插入 image block（加载失败时显示 fallback 也可接受）。
  - **打开资源**：对插入的 image 执行 `Cmd/Ctrl+Click`：
    - URL image 打开浏览器；
    - 本地 image 打开默认图片查看器（或系统处理）。

### Iteration 37：Media Input Pro（Drop/Paste Image）

> 目标：补齐“媒体输入”的两个高频路径：拖拽图片文件、粘贴剪贴板图片（截图）。这两条路径打通后，图片插入不再强依赖 toolbar/file picker，对用户更自然。
>
> 状态：已实现（`crates/plate-core/src/plugin.rs`、`crates/rich_text/src/state.rs`）。

**要实现什么**
- `gpui-plate-core`（core）：
  - 新增 `image.insert_many` 命令：一次性插入 N 个 image block，并自动追加一个空 paragraph（caret 落在该 paragraph）。
- `gpui-manos-plate`（view）：
  - 支持从 Finder/资源管理器拖拽 1~N 个本地图片文件到编辑器：插入对应 image blocks。
  - 支持 `Cmd/Ctrl+V` 粘贴剪贴板图片（`ClipboardEntry::Image`）：插入 image block。
  - 允许粘贴“绝对路径文本”（每行一个图片路径）时自动识别为图片并插入（可选但实现成本低）。

**怎么实现（关键做法）**
- Drop：
  - 在 editor 根容器挂 `.on_drop(...)`，从 `ExternalPaths` 里筛选图片扩展名（png/jpg/jpeg/gif/webp/bmp/svg/tif/tiff）。
  - drop 时基于 mouse position 尽量做一次 hit-test，把 caret 移到落点对应的 text block，再执行 `image.insert_many`（保证插入位置符合直觉）。
- Paste（clipboard image）：
  - 在 `command_paste` 内优先检测 `clipboard.entries()` 中的 `ClipboardEntry::Image`。
  - 将图片 bytes 编码为 `data:<mime>;base64,...` 并作为 `src` 执行 `image.insert_many`（完整持久化与渲染细节见 Iteration 39）。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 37 通过标准）：
  - **拖拽插入**：从 Finder/资源管理器拖拽 1 张图片到编辑器任意位置 → 插入 1 个 image block，光标落到其后新段落；拖拽多张图片 → 插入多个 image block。
  - **拖拽定位**：拖到文档中部（某段落附近） → 图片应插入在该位置附近（基于 drop hit-test 设置 caret）。
  - **粘贴截图**：将截图复制到剪贴板后在编辑器聚焦时 `Cmd/Ctrl+V` → 插入 image block。
  - **粘贴路径**：剪贴板为“绝对路径文本”（每行一个图片路径）时 `Cmd/Ctrl+V` → 插入对应 image blocks；非图片文本仍按文本粘贴。
  - **Undo/Redo**：对拖拽/粘贴插入的图片执行 undo/redo，文档结构与 selection 行为合理（允许与文本替换一样是多步 undo）。

### Iteration 38：Media File Picker Pro（多选 + 明确反馈）

> 目标：把 “toolbar → file picker 插图” 从 demo 级提升到“可用”：支持多选一次插入多张图，并在失败/无效选择时给出可见反馈，解决“点击没有任何反应”的体验问题。
>
> 状态：已实现（`crates/story/src/richtext.rs`、`crates/rich_text/src/state.rs`）。

**要实现什么**
- `gpui-manos-components-story`（example）：
  - Image 按钮的文件选择器允许多选，选中 1~N 张图片后一次性插入（单个操作）。
  - file picker 打开失败时显示 notification（Linux/平台错误等）。
  - 若用户选中非图片文件，显示 “No supported image files selected.” 的 notification（避免静默无反应）。
- `gpui-manos-plate`（view）：
  - 新增 `command_insert_images(Vec<String>)`：统一走 `image.insert_many`，保持与 Drop/Paste 一致的插入语义与 selection 行为。

**怎么实现（关键做法）**
- story 层 `prompt_for_paths` 改为 `multiple: true`，在回调中筛选图片扩展名并集中调用 `command_insert_images`。
- 对 `prompt_for_paths` 的返回结果做显式 match：
  - `Ok(Ok(None))`（用户取消）→ 安静返回；
  - `Ok(Err(err))`（平台错误）→ notification 提示；
  - 选中路径但无任何图片 → notification 提示。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 38 通过标准）：
  - **多选插图**：点击 toolbar Image → file picker 可多选；选择多张图片后确认 → 依次插入多个 image block，并将光标放到末尾新段落。
  - **无效选择反馈**：只选择非图片文件（或混合但无图片）→ 弹出通知 “No supported image files selected.”
  - **失败反馈**：若平台返回错误（例如 Linux 无法打开 picker）→ 弹出 “Failed to open file picker: …” 通知（不再静默失败）。

### Iteration 39：Embedded Clipboard Image Pro（Paste as `data:` URL + Render Decode）

> 目标：让“粘贴截图”成为真正可保存/可迁移的内容：不再依赖临时文件路径，而是把剪贴板图片以内嵌 `data:<mime>;base64,...` 的形式写入 `image.src`；并在渲染层显式支持 `data:` URL 解码与缓存，保证保存/打开 JSON 后依然可显示。
>
> 状态：已实现（`crates/rich_text/src/state.rs`）。

**要实现什么**
- `gpui-manos-plate`（view）：
  - `Cmd/Ctrl+V` 粘贴 `ClipboardEntry::Image` 时：生成 `data:` URL 并插入 image block（不写临时文件）。
  - 渲染 image 时：当 `src` 为 `data:` URL，解码为 `gpui::Image` 并用内存图片渲染；对同一个 `src` 做缓存，避免每次 render 都重复 base64 decode。
- `gpui-manos-components-story`（example）：
  - 无需新增 UI；直接复用现有 Open/Save/Save As 进行 round-trip 验收。

**怎么实现（关键做法）**
- `command_paste` 里优先收集 `ClipboardEntry::Image`：
  - `src = "data:{mime};base64,{...}"`（mime 从 `image.format.mime_type()` 获取）。
  - 将 `src -> Arc<Image>` 写入 `embedded_image_cache`，并调用 `image.insert_many` 插入。
- 渲染层：
  - 识别 `src.starts_with("data:")`，解析 mime + base64 payload → `Image::from_bytes(format, bytes)`。
  - 缓存：`HashMap<String, Arc<Image>>`（key 为 data URL，本地持久化一致）。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 39 通过标准）：
  - **粘贴截图即插入**：复制一张截图到剪贴板 → 编辑器聚焦后 `Cmd/Ctrl+V` → 插入 image block，且能正常显示图片内容。
  - **保存后仍可显示**：执行 `Save As...` 保存 JSON，关闭窗口/重启示例后 `Open...` 打开该 JSON → 图片仍可显示（不依赖临时目录/外部文件）。
  - **不污染 UI**：图片 caption 不应把整段 `data:` URL 显示出来（应显示 `Embedded image` 或 alt）。

### Iteration 40：Portable Export Pro（Embed Local Images）

> 目标：补齐“可迁移文档”的最后一块：drop/file picker 插入的本地路径图片，默认仍是文件路径引用；通过一键命令把这些本地图片转换为内嵌 `data:` URL，让文档可以单文件（JSON）携带所有媒体内容并跨机器打开。
>
> 状态：已实现（`crates/rich_text/src/state.rs`、`crates/story/src/app_menus.rs`、`crates/story/src/richtext.rs`）。

**要实现什么**
- `gpui-manos-plate`（view）：
  - 新增 `command_embed_local_images`：遍历文档内所有 image void nodes，对符合条件的本地路径图片做 `src` 替换（`/abs/path` / `file://...` → `data:<mime>;base64,...`），并保持 undo/redo 为 1 步。
- `gpui-manos-components-story`（example）：
  - File 菜单新增入口 `Embed Local Images (Data URL)`，执行后给出 notification（包含 embedded/skipped/failed 统计）。

**怎么实现（关键做法）**
- 使用 `Op::SetNodeAttrs { patch: AttrPatch { set: {src}, remove: [] } }` 批量更新节点属性，不改变树结构与 selection（天然适配 undo/redo）。
- 读取文件并按扩展名映射 MIME（png/jpeg/webp/gif/svg/bmp/tiff），再 base64 编码生成 data URL。
- 渲染层复用 Iteration 39 的 data URL 解码与缓存：转换后无需额外适配即可显示。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 40 通过标准）：
  - **插入本地图片**：通过 toolbar Image（file picker）或拖拽插入至少 1 张本地图片（此时 caption 通常会展示路径）。
  - **一键内嵌**：执行菜单 `File → Embed Local Images (Data URL)`：
    - image caption 变为 `Embedded image`（或 alt），图片仍可显示；
    - 弹出 notification，embedded > 0。
  - **可迁移**：`Save As...` 保存 JSON → 重启示例 `Open...` 打开该 JSON → 图片仍可显示（即使原始文件路径不存在也不影响显示）。
  - **Undo/Redo**：对 “Embed Local Images” 执行 undo/redo，应能在“路径引用 ↔ 内嵌 data URL”之间切换（作为单一步骤）。

### Iteration 41：Portable Export JSON Pro（Non-mutating Export）

> 目标：把 “内嵌化” 从“修改文档的编辑操作”升级为“导出能力”：导出一份可迁移 JSON（本地图片自动内嵌为 data URL），但**不改变**当前编辑器文档状态（不产生 undo step，不影响继续编辑）。
>
> 状态：已实现（`crates/rich_text/src/state.rs`、`crates/story/src/app_menus.rs`、`crates/story/src/richtext.rs`）。

**要实现什么**
- `gpui-manos-plate`（view）：
  - 提供一个纯函数/工具方法：输入 `PlateValue`，输出“portable PlateValue”（本地图片 `src` 内嵌为 data URL）以及统计信息（embedded/skipped/failed）。
- `gpui-manos-components-story`（example）：
  - File 菜单新增 `Export Portable JSON...`：
    - 弹出保存路径选择；
    - 在后台完成本地图片读取 + base64 编码 + 写文件；
    - 完成后弹出 notification（包含 embedded/skipped/failed）。

**怎么实现（关键做法）**
- 在 view 层实现 `RichTextState::make_plate_value_portable(value, document_base_dir)`：对 `value.document` 做递归遍历并原地替换 image void 的 `attrs.src`（支持绝对路径、`file://` 与**相对路径（基于 base dir）**）。
- example 层在触发导出时先 snapshot 当前 `PlateValue`，再在 `spawn_in` 异步任务中转换与写入，避免阻塞 UI。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 41 通过标准）：
  - **不修改当前文档**：插入一张本地图片（src 为路径），执行 `File → Export Portable JSON...`：
    - 导出成功后，编辑器里的图片仍保持“路径 src”（caption 仍显示路径），并且 undo/redo 栈不增加额外步骤。
  - **导出的 JSON 可迁移**：用 `Open...` 打开刚导出的 JSON（或拷贝到另一台机器打开）：
    - 图片可显示，即使原路径文件不存在也不影响显示；
    - JSON 内 `image.attrs.src` 为 `data:image/...;base64,...`。
  - **错误提示**：若某些图片路径读取失败，notification 中 `failed` > 0，且导出文件仍写出（失败的图片保持原 src）。

### Iteration 42：Plate Bundle Export Pro（Relative Assets + Export Bundle）

> 目标：提供 “不膨胀 JSON、但仍可迁移” 的第二条路径：导出为 `plate.bundle.json` + `assets/` 文件夹，并让编辑器能够在 `Open...` 时基于 JSON 所在目录解析相对 `image.src`（例如 `assets/xxx.png`），实现 **folder-level portability**。
>
> 状态：已实现（`crates/rich_text/src/state.rs`、`crates/rich_text/src/types.rs`、`crates/story/src/app_menus.rs`、`crates/story/src/richtext.rs`）。

**要实现什么**
- `gpui-manos-plate`（view）：
  - 引入 `document_base_dir: Option<PathBuf>`，用于渲染与交互时解析相对资源路径。
  - image 渲染与 `Cmd/Ctrl+Click` 打开行为：支持 `data:`、绝对路径、`file://`、以及相对路径（`base_dir.join(src)`）。
  - 提供 `RichTextState::make_plate_value_bundle(value, document_base_dir)`：将可解析的本地图片重写为 `assets/<hash>.<ext>`，并返回需要写出的 assets（bytes）与统计信息（bundled/skipped/failed）。
- `gpui-manos-components-story`（example）：
  - `Open...` 读取 JSON 时，把 `path.parent()` 作为 `document_base_dir` 传给 editor；`Save As...` 写出后同步更新 base dir。
  - File 菜单新增 `Export Plate Bundle...`：选择导出 JSON 路径后，在同目录创建 `assets/`，写出 assets 与 JSON，并以 notification 展示统计信息。

**怎么实现（关键做法）**
- base dir：
  - `RichTextState` 保存 `document_base_dir`，并在渲染递归与 `Cmd/Ctrl+Click` 中统一使用 `local_image_path_for_src(src, base_dir)` 解析本地路径（支持相对路径）。
- bundle：
  - 遍历文档中所有 `image` void 节点：
    - `src` 可解析为本地文件 → 读取 bytes → `DefaultHasher` 对 bytes 求 hash → 生成 `assets/<hash>.<ext>`；
    - 节点 `src` 替换为相对路径，并将 `{relative_path, bytes}` 收集到 assets 列表（按 relative path 去重）；
    - 失败/不可解析的保持原 src（计入 failed/skipped）。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 42 通过标准）：
  - **Bundle 导出闭环**：插入至少 1 张本地图片（file picker/拖拽均可）→ 执行 `File → Export Plate Bundle...`：
    - 选择保存路径后导出成功；
    - 导出目录生成 `assets/`，其中包含若干图片文件；
    - notification 显示 assets 数量与 bundled/skipped/failed 统计。
  - **相对路径可打开**：用 `Open...` 打开刚导出的 bundle JSON：
    - 图片可显示（`src` 为相对路径）；
    - `Cmd/Ctrl+Click` 图片可打开本地文件。
  - **不影响当前文档**：导出 bundle 不应修改当前编辑器文档内容，也不应产生额外 undo step。

### Iteration 43：Relative Assets UX Pro（Prefer Relative Insert + Safe Save As）

> 目标：把 “folder-level portability” 做到默认好用：
> 1) 文档已关联到一个目录（base dir）时，插入本地图片优先存相对 `src`（例如 `assets/logo.png`），便于整个文件夹搬迁；
> 2) 对包含相对 `src` 的文档执行 `Save As...` 时，自动复制被引用的相对 assets 到新目录，避免保存后编辑器立刻找不到图片。
>
> 状态：已实现（`crates/rich_text/src/state.rs`、`crates/story/src/richtext.rs`）。

**要实现什么**
- `gpui-manos-plate`（view）：
  - 新增 `normalize_image_src_for_document`：当 `document_base_dir` 存在且图片位于 base dir 内时，把绝对路径归一化为相对 `src`（并统一为 `/` 分隔符）。
  - 统一所有插图入口都走同一套逻辑（file picker / dialog / drag&drop）。
  - 提供 `referenced_relative_image_paths()` 供上层在保存时收集需复制的相对 assets。
- `gpui-manos-components-story`（example）：
  - `Save As...`：若从旧 base dir 保存到新 base dir，且文档引用了相对图片资源，则把这些文件按相对路径复制到新目录（并在 notification 中展示 copied/failed/skipped）。

**怎么实现（关键做法）**
- 相对插入：
  - 基于 `document_base_dir` + `canonicalize` 判断“图片是否在 base dir 内”，成立则 `strip_prefix` 后写入相对 `src`；
  - drop 事件改为调用 `command_insert_images`，避免绕开归一化逻辑。
- Save As 复制：
  - 仅对 `image.src` 为“相对路径”（非 `data:`、非 `://`、非绝对路径）的引用做复制；
  - 复制时 `canonicalize(src)` 并要求其仍位于旧 base dir 内（避免 `..` 越界），再复制到新目录对应位置。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 43 通过标准）：
  - **相对插入默认生效**：
    - `Open...` 打开一个 JSON（确保 base dir 已设置），并在同目录创建/准备一个 `assets/` 图片文件；
    - 插入该图片（file picker 选中同目录内图片即可）→ 保存 JSON → 检查 JSON 中 `image.attrs.src` 为相对路径（例如 `assets/...png`）。
  - **Save As 不丢图**：
    - 打开包含相对图片引用（`assets/...`）的 JSON；
    - 执行 `Save As...` 保存到一个新目录；
    - 编辑器中图片应仍可显示；
    - 新目录下应生成对应的 `assets/...` 文件（被复制）。

### Iteration 44：Bundle Externalize Data URLs Pro（Shrink JSON + Assetize Embedded）

> 目标：解决 “portable JSON 太大不适合版本管理” 的问题：在导出 bundle 时，把 `data:`（剪贴板截图 / Embed Local Images / Export Portable JSON 产生的内嵌图片）也外置为 `assets/<hash>.<ext>`，让 bundle JSON 仍保持轻量可读。
>
> 状态：已实现（`crates/rich_text/src/state.rs`、`crates/rich_text/src/types.rs`、`crates/story/src/richtext.rs`）。

**要实现什么**
- `gpui-manos-plate`（view）：
  - `make_plate_value_bundle` 除了处理本地文件路径图片，还应处理 `data:` URL：
    - decode base64 得到 bytes；
    - 根据 mime 推断扩展名（尽可能保持常见格式）；
    - 写出 `assets/<hash>.<ext>` 并把 `image.src` 替换为相对路径。
  - `BundleExportReport` 增加 `bundled_data_url` 统计（用于 UX 反馈）。
- `gpui-manos-components-story`（example）：
  - `Export Plate Bundle...` notification 展示 `data` 计数（当 bundled_data_url > 0）。

**怎么实现（关键做法）**
- 在 bundle 导出遍历时：
  - `src.starts_with(\"data:\")` → parse meta + base64 payload → bytes；
  - mime → ext 映射（png/jpg/webp/gif/svg/bmp/tiff）；
  - 与本地文件路径走同一套 hash + 去重写入逻辑。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 44 通过标准）：
  - **截图→bundle→小 JSON**：
    - 复制一张截图到剪贴板 → 编辑器 `Cmd/Ctrl+V` 插入（此时 `src` 为 `data:`）；
    - 执行 `File → Export Plate Bundle...`；
    - 打开导出的 JSON：不应包含大段 `data:image/...;base64,...`（应变为 `assets/...`）。
  - **bundle 可打开**：
    - `Open...` 打开刚导出的 bundle JSON → 图片应可显示；
    - `Cmd/Ctrl+Click` 图片可打开本地文件。

### Iteration 45：Collect Assets In-Place Pro（Mutating Attach / Assetize）

> 目标：把 “文件夹级可迁移” 变成日常编辑的默认工作流：提供一个**会修改当前文档**的命令，把所有本地图片引用（绝对路径/`file://`/相对路径）与 `data:` 图片统一写入到当前文档目录的 `./assets/` 中，并把 `image.src` 重写为 `assets/<hash>.<ext>`。
>
> 状态：已实现（`crates/rich_text/src/state.rs`、`crates/rich_text/src/types.rs`、`crates/story/src/app_menus.rs`、`crates/story/src/richtext.rs`）。

**要实现什么**
- `gpui-manos-plate`（view）：
  - 新增 `command_collect_assets_into_assets_dir`：
    - 要求 `document_base_dir` 已设置（通常意味着文档已 `Open...` 或已 `Save As...`）；
    - 遍历所有 `image` void：
      - 本地路径 → 读取 bytes → `assets/<hash>.<ext>`；
      - `data:` → base64 decode → `assets/<hash>.<ext>`；
    - 写文件到 `base_dir/assets/`（按 hash 去重），并用单个 `Transaction` 批量改写 `image.src`（单步 undo）。
- `gpui-manos-components-story`（example）：
  - File 菜单新增 `Collect Assets into ./assets (Rewrite src)`；
  - 若文档未保存（无 base dir）则提示先 `Save As...`。

**怎么实现（关键做法）**
- `data:` decode 与 mime→ext 映射复用统一 helper；与 bundle export 共用同一套 hash 命名逻辑。
- 文件写入先于 `SetNodeAttrs`：写入失败则不改写对应节点 src，避免文档指向不存在的 asset。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 45 通过标准）：
  - **本地路径 → 资产化**：
    - `Open...` 打开一个 JSON（确保 base dir 已设置）；
    - 插入 1 张本地图片（src 为路径或相对路径）；
    - 执行 `File → Collect Assets into ./assets (Rewrite src)`：
      - 目录下出现 `assets/<hash>.<ext>` 文件；
      - 文档 JSON 中 `image.attrs.src` 变为 `assets/<hash>.<ext>`；
      - undo 一步可回到改写前的 src（注意：写出的 asset 文件不会被 undo 删除）。
  - **截图（data URL）→ 资产化**：
    - 粘贴一张截图（生成 `data:` src）；
    - 执行 Collect Assets → `image.src` 变为 `assets/...`，且图片仍可显示。

### Iteration 46：Portability Diagnostics Pro（Report + Quick Actions）

> 目标：把“可迁移性”从“靠经验”升级为“可验收/可提示”：
> - 保存/导出时对缺失资源给出明确 warning；
> - 提供 `Portability Report...` 对话框集中展示（errors/warnings + 分类统计）；
> - 在报告中提供一键动作：`Collect Assets` / `Export Bundle…` / `Export Portable JSON…`。
>
> 状态：已实现（`crates/rich_text/src/state.rs`、`crates/rich_text/src/types.rs`、`crates/story/src/app_menus.rs`、`crates/story/src/richtext.rs`）。

**要实现什么**
- `gpui-manos-plate`（view）：
  - 新增 `portability_report()`：扫描文档中所有 `image` 的 `src` 并分类统计（data/http/absolute/relative），输出 errors/warnings 与问题列表（缺失文件、绝对路径、外链、data URL 等）。
  - 公开 `document_base_dir()` 供上层展示 base dir 与解析语义对齐。
- `gpui-manos-components-story`（example）：
  - File 菜单新增 `Portability Report...`，打开报告对话框（含一键动作按钮）。
  - `Save`/`Save As...`：保存成功后若仍存在 errors/warnings，在 notification 中附带 portability 摘要与入口提示。
  - `Export Portable JSON...` / `Export Plate Bundle...`：导出前若检测到缺失图片，先弹 warning 提示用户检查报告（导出仍继续）。

**怎么实现（关键做法）**
- 在 view 层做“资源语义统一”：复用已有 `normalize_image_src` / base dir 规则，对 `src` 做一致分类与存在性检查。
- 报告 UI 复用现有 dialog 模式（类似 Find/Command Palette）：
  - 统计 + issue list（最多展示前 200 条）；
  - 直接调用现有导出/collect 的实现（不引入新的中间状态）。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 46 通过标准）：
  - **报告入口**：`File → Portability Report...` 可打开对话框，展示 base dir、统计与 issues 列表。
  - **一键动作**：在报告里点击 `Collect Assets` / `Export Bundle…` / `Export Portable JSON…` 均可正常执行并有 notification。
  - **保存提示**：包含绝对路径或 data/http 图片时执行 `Save`/`Save As...`，通知应附带 portability 摘要与 “File → Portability Report...” 提示。
  - **缺失资源提示**：让文档引用一个不存在的图片路径（或删掉被引用的 `assets/...` 文件）→ 执行导出 → 应先提示 warning（导出仍可继续）。

### Iteration 47：Image Dialog-First UX Pro（Always-visible Insert + Browse）

> 目标：解决 “Insert image 点击无反应” 的 UX 死角——即便系统 file picker 在某些环境下不可用/不弹窗，也必须给用户一个**必定可见**且可用的插入入口，并把 base dir（相对路径语义）显式提示出来。
>
> 状态：已实现（`crates/story/src/app_menus.rs`、`crates/story/src/richtext.rs`）。

**要实现什么**
- `richtext` 示例：
  - 新增 `Edit → Insert Image...` 菜单项与 `Cmd/Ctrl+Shift+I` 快捷键，打开 Insert Image 对话框。
  - Image toolbar 按钮默认打开该对话框；Shift+Click 保留“多选 file picker 直接插入”。
  - 对话框内提供 `Browse…`：选中本地图片后自动填充路径；并展示当前 `base dir`，让用户理解相对路径会如何解析/写入。

**怎么实现（关键做法）**
- 用独立 dialog view（`InsertImageDialogView`）承载异步 file picker 与 input 填充逻辑，保持 `open_dialog` 壳层简洁。
- 插入仍复用 `RichTextState::command_insert_image` 的 normalize（base dir 存在时写相对 `src`）与渲染逻辑，不引入新的中间状态。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 47 通过标准）：
  - **Toolbar 入口**：点击 Image 按钮必定弹出对话框；Shift+Click 仍可打开系统 file picker 并插入图片。
  - **菜单/快捷键入口**：`Edit → Insert Image...` 与 `Cmd/Ctrl+Shift+I` 均可打开对话框。
  - **Browse 可用**：在对话框点击 `Browse…` 选择一张图片 → Input 自动填充路径 → 点击 OK 后文档出现 image block（能看到占位/或实际图片）。
  - **Base dir 提示**：打开一个已设置 base dir 的文档后再次打开对话框，应显示 base dir；插入本地图片并保存后，`image.src` 应按 base dir 规则写为相对路径（便于迁移）。

### Iteration 48：Failure Visible Pro（Command/Apply Errors → Notification）

> 目标：把 “失败静默” 彻底消灭。任何插件 command 或 tx apply 失败，都必须在 UI 上有明确反馈（含 `command id` / `tx source`），从而让验收与问题定位不再依赖猜测。
>
> 状态：已实现（`crates/rich_text/src/state.rs`）。

**要实现什么**
- `gpui-manos-plate`（view）：
  - 当 `run_command_and_refresh(...)` 执行失败时，自动弹出 notification（包含 `command id` 与错误信息）。
  - 当 `editor.apply(tx)` 失败时，自动弹出 notification（包含 `tx.meta.source` 与错误信息）。

**怎么实现（关键做法）**
- 在 `RichTextState` 内增加 `pending_notifications` 队列：
  - command/apply 失败时把 message 放入队列，并 `cx.notify()`；
  - 在 `Render for RichTextState` 开头 flush 队列并 `window.push_notification(...)`；
  - 通过 `std::mem::take` 确保每条消息只弹一次，不会在重绘中重复刷屏。

**要验收什么**
- 运行入口：`cargo run -p gpui-manos-components-story --example richtext`
- 手动验收清单（Iteration 48 通过标准）：
  - **命令失败可见**：打开 Command Palette（`Cmd/Ctrl+Shift+P`），选择 `marks.set_link`，不填 args 直接执行 → 应提示类似 `Command failed (marks.set_link): Missing args.url`。
  - **apply 失败可见（开发校验）**：当出现内部 apply/normalize 错误时，应弹出 `Apply failed (source): ...`（此项通常不应在正常使用中触发；目的是避免一旦触发时静默无提示）。

## 9. 风险与对策

- **风险：只做 UI 插件化收益有限**：必须优先落地命令/Hook/渲染分组这三类“能把逻辑迁走”的扩展点。
- **风险：插件状态与 undo/redo 不一致**：先分层 EditorModel/Session/Layout，并定义“进入快照”的硬规则。
- **风险：树模型 + IME/hit-test 实现复杂度上升**：优先把“view 层 hit-test ↔ path/offset”抽成独立模块，并尽早用真实 IME 场景验证（这属于允许的技术验证）。
