# RichText Editor 扩展性总结

## 现状：哪些扩展“已经可做”

- **更多文字/背景颜色**：`InlineStyle` 直接存 `fg/bg: Option<Hsla>`；`RichTextState::set_text_color` / `set_highlight_color` 只是示例按钮少。接 `gpui-component` 的 `color_picker` 或自定义调色板即可，无需改文档模型。
- **新增块类型（TodoList 等）**：块级用 `BlockKind + BlockFormat` 存在节点树里；可加 `BlockKind::Todo { checked: bool }`，渲染时像列表前缀一样绘制 checkbox，交互上在点击命中区域切换状态即可。
- **对齐（左/中/右）**：可在 `BlockFormat` 增加 `align: TextAlign`，渲染 block wrapper 时设置 `.text_left()/.text_center()/.text_right()`（或对应 `TextStyle`）；命中测试/光标位置跟随 `TextLayout` 一起正确更新。
- **行距/段距**：行距可通过每块 `.line_height(...)`（`TextStyle` 原生支持），段距可通过 block 容器 `mt/mb` 或 `gap` 实现。

## 当前模型的边界（需要重构的方向）

- **文档结构（已升级为节点树）**：现在内部是 `RichTextDocument { blocks: Vec<BlockNode> }`，每个 `BlockNode` 有 `format`（块属性）与 `inlines`（`InlineNode::Text(TextNode { text, style })`）。编辑时仍然用“`\n` 连接 blocks”的扁平文本做 hit-test/IME，节点树是富文本的真实来源。
- **更深层的 Slate/Plate 能力**：目前节点树是“block -> text leaf”的两层结构，足以支撑大多数 inline marks + 简单 block。要做到 Plate 那种嵌套列表、表格、图片/多媒体（void nodes）、块拖拽、schema/normalize 管线等，需要把 `InlineNode` 扩展为真正的 `Node { Element | Text | Void }` 并引入 Path/Operation/Normalize 体系。
- **序列化/持久化**：节点树已经能表达样式，但还需要定义稳定的存储格式（建议做 versioned JSON），并提供与 Slate JSON 的互转层（因为当前结构天然接近 Slate 的 “element children + text leaf marks” 形态）。
