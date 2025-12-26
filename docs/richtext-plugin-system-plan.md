# RichText：插件系统与未来架构计划（激进重构版）

本文基于对本仓库 `gpui-manos-plate`（`crates/rich_text`）的实现细读，给出一份**面向未来与最佳实践**的重构计划：直接对齐 Plate/Slate 体系常见的设计（树模型、operations、normalize、可组合插件），不再规划 v1 过渡态。

相关背景与当前实现边界见：
- [RichText Editor：扩展性与重构方向（面向未来）](richtext-extensibility.md)
- 工具层 UI 组件（Toolbar）参考：[Plate Toolbar Buttons（GPUI 组件实现教程）](plate-toolbar-buttons.md)

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

> 新内核已以独立 crate 落地：`crates/plate-core`（crate 名称：`gpui-plate-core`），并通过 `crates/story/examples/richtext.rs` 提供验收入口；并已完成 `gpui-manos-plate`（`crates/rich_text`）的一次性替换壳层。

本项目采用的落地位置与命名约定：
- 新内核 crate：`crates/plate-core`
- crate 名称：`gpui-plate-core`
- 建议示例入口：`cargo run --example richtext`（位于 `crates/story/examples/richtext.rs`）

内置命令与查询（便于工具层/插件协作）：
- Command IDs（示例）：`core.insert_divider`、`marks.toggle_bold`、`marks.toggle_italic`、`marks.toggle_underline`、`marks.toggle_strikethrough`、`marks.toggle_code`、`marks.set_link`、`marks.unset_link`、`marks.set_text_color`、`marks.unset_text_color`、`marks.set_highlight_color`、`marks.unset_highlight_color`、`block.set_heading`、`block.unset_heading`、`blockquote.wrap_selection`、`blockquote.unwrap`、`todo.toggle`、`todo.toggle_checked`、`list.toggle_bulleted`、`list.toggle_ordered`、`list.unwrap`、`mention.insert`、`table.insert`、`table.insert_row_below`、`table.insert_col_right`、`table.delete_row`、`table.delete_col`
- Query IDs（示例）：`marks.get_active`、`marks.is_bold_active`、`marks.is_italic_active`、`marks.is_underline_active`、`marks.is_strikethrough_active`、`marks.is_code_active`、`marks.has_link_active`、`block.heading_level`、`blockquote.is_active`、`todo.is_active`、`todo.is_checked`、`list.active_type`、`list.is_active`（参数：`{ "type": "bulleted" | "ordered" }`）、`table.is_active`
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
- 能运行一个新的示例：`crates/story/examples/richtext.rs`
- 运行方式：`cargo run -p gpui-manos-components-story --example richtext`
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
- 将验收入口统一到：`cargo run -p gpui-manos-components-story --example richtext`

**怎么实现（关键做法）**
- 不为“兼容旧格式/旧模型”牺牲底层：旧 Slate JSON 如需支持，应以**独立适配器插件/独立 crate**实现（不进入内核默认路径）
- 壳层只做 gpui 视图适配与命令/查询桥接；结构约束与编辑语义全部留在 core（schema/normalize/ops）

**要验收什么**
- 编译：`cargo check -p gpui-manos-plate`
- 示例：`cargo run -p gpui-manos-components-story --example richtext`
  - app menu 的 Edit actions（Undo/Redo/Cut/Copy/Paste/SelectAll）在编辑器聚焦时可用
  - Open/Save/Save As 能读写 `gpui-plate` JSON（例如 `plate-core.example.json`）
- 文档：`docs/richtext-extensibility.md`、`docs/richtext-plugin-system-plan.md`、`docs/richtext-known-issues.md` 链接与入口说明准确

---

### Iteration 8：工具栏增强（Marks Pro：Italic/Underline/Strike/Code/Colors）

> 背景：旧实现的 toolbar 能力更丰富；当前 `richtext` 示例为验证最终架构（core/view/plugin/IME/serialize）只落了最小必要插件集，因此 UI 能力与旧示例不对齐是**阶段性现象**。本迭代目标是在**不牺牲底层设计**的前提下，用插件体系把“高频 inline 格式能力”补齐，并把 toolbar 丰富度拉回到可展示的水平。

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

## 9. 风险与对策

- **风险：只做 UI 插件化收益有限**：必须优先落地命令/Hook/渲染分组这三类“能把逻辑迁走”的扩展点。
- **风险：插件状态与 undo/redo 不一致**：先分层 EditorModel/Session/Layout，并定义“进入快照”的硬规则。
- **风险：树模型 + IME/hit-test 实现复杂度上升**：优先把“view 层 hit-test ↔ path/offset”抽成独立模块，并尽早用真实 IME 场景验证（这属于允许的技术验证）。
