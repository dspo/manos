# RichText Editor：扩展性（最终架构版）

当前 RichText 已完成“一次性替换”：编辑内核与扩展机制以 `gpui-plate-core` 为中心（树模型 + ops/transaction + normalize + 插件/命令/查询），`gpui-manos-plate` 仅负责 gpui 视图层与输入/IME/hit-test 的适配。

相关文档：
- [RichText：插件系统与未来架构计划（激进重构版）](richtext-plugin-system-plan.md)
- [RichText：已知问题（Tracked Issues）](richtext-known-issues.md)

## 代码分层（扩展点在哪里）

- **Core（可复用、可测试）**：`crates/plate-core`（crate：`gpui-plate-core`）
  - 文档模型：`Document/Node/Marks/Attrs`
  - 编辑语义：`Op/Transaction`、`Editor::apply`、undo/redo
  - 插件系统：`PluginRegistry`（node spec / normalize passes / commands / queries）
- **View（gpui 交互、IME、渲染）**：`crates/rich_text`（crate：`gpui-manos-plate`）
  - `RichTextState`：把 gpui 输入映射为 `Editor` 的 command/transaction，并渲染为 gpui elements
- **验收入口（示例 UI）**：`crates/story`
  - `cargo run -p gpui-manos-components-story --example richtext`

## 如何新增能力（推荐做法）

1) 在 `gpui-plate-core` 里新增/扩展插件（推荐：新增独立 plugin struct）
   - `node_specs()`：声明新节点 kind、role、children 约束、是否 void
   - `normalize_passes()`：声明结构修复逻辑（输出 ops）
   - `commands()`：声明可组合命令（稳定字符串 ID），供工具栏/快捷键/菜单触发
   - `queries()`：声明状态查询（例如 marks/list/table active），供 UI 做 enable/selected

2) 在 `gpui-manos-plate` 的视图层只做“通用适配”
   - 优先通过 `Editor::run_command(...)` 触发行为（避免在 view 层堆特例）
   - 只有当涉及 gpui 输入/布局/hit-test/IME 时才在 view 增加逻辑（并保持与 `block_path` 对齐）

3) 在 story 示例里补 UI 入口与验收用例
   - toolbar/menu/dialog 仅依赖 command/query，不直接依赖插件内部实现
