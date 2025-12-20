# RichText Editor 扩展性总结

## 现状：哪些扩展“已经可做”

- **更多文字/背景颜色**：核心已支持任意颜色；`RichTextState::set_text_color(Option<Hsla>)` / `set_highlight_color(Option<Hsla>)` 可接收任何 `Hsla`，目前示例里只有 3 种只是 toolbar 按钮少。后续接 `gpui-component` 的 `color_picker` 或自定义调色板，不需要改文本模型。
- **新增块类型（TodoList 等）**：块级用 `BlockKind + BlockFormat`（按行存 `Vec<BlockFormat>`）。新增 `BlockKind::Todo { checked: bool }` 这类“单行块”很顺：渲染时像列表前缀一样绘制 checkbox；交互上可在 `RichTextInputHandlerElement` 中根据点击区域切换 checked 状态。
- **对齐（左/中/右）**：可在 `BlockFormat` 增加 `align: TextAlign`，渲染每行 wrapper 时调用 `.text_left()/.text_center()/.text_right()`；`StyledText/TextLayout` 会跟随 `text_style.text_align`，命中测试/光标位置也会一起正确更新。
- **行距/段距**：行距可通过每块设置 `.line_height(...)`（`TextStyle` 原生支持），段距可通过块容器的 `mt/mb` 或动态 `gap` 实现。

## 当前模型的边界（需要重构的方向）

- **文档结构**：当前是“`\n` = 一个 block/row”的线性结构（`blocks: Vec<BlockFormat>` 对齐到 `Rope` 的行）。像“段落内部多行、嵌套列表、表格、复杂组合块、真正的节点树”这类 Slate 风格能力，未来要做会涉及重构为 block-tree/节点模型，目前没有预留可插拔 schema/normalize 机制。
- **序列化/持久化**：目前 `value()` 只返回纯文本，样式未序列化。若要持久化/复制带样式内容，需要定义文档格式（blocks + style runs）并实现导入导出。
