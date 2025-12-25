# RichText Editor：扩展性与重构方向（面向未来）

本文包含两部分：

1. 当前实现（`crates/rich_text`）在不改模型的前提下“已经能做/容易做什么”（用于理解现状与能力边界）
2. 本项目的目标方向：直接重构为面向未来最佳实践的 **树模型 + 操作（operations）+ normalize + 插件系统** 架构（不再规划 v1 过渡态）

详细的目标架构与落地计划见：
- [RichText 插件系统与未来架构计划](richtext-plugin-system-plan.md)

## 现状：哪些扩展“已经可做”

- **更多文字/背景颜色**：`InlineStyle` 直接存 `fg/bg: Option<Hsla>`；`RichTextState::set_text_color` / `set_highlight_color` 只是示例按钮少。接 `gpui-component` 的 `color_picker` 或自定义调色板即可，无需改文档模型。
- **新增块类型（TodoList 等）**：块级用 `BlockKind + BlockFormat` 存在节点树里；可加 `BlockKind::Todo { checked: bool }`，渲染时像列表前缀一样绘制 checkbox，交互上在点击命中区域切换状态即可。
- **对齐（左/中/右）**：可在 `BlockFormat` 增加 `align: TextAlign`，渲染 block wrapper 时设置 `.text_left()/.text_center()/.text_right()`（或对应 `TextStyle`）；命中测试/光标位置跟随 `TextLayout` 一起正确更新。
- **行距/段距**：行距可通过每块 `.line_height(...)`（`TextStyle` 原生支持），段距可通过 block 容器 `mt/mb` 或 `gap` 实现。

## 现状模型的边界（也是我们选择“直接重构”的原因）

- **结构表达能力有限**：当前是“按行 blocks + 文本 leaf”的两层结构，缺少通用的 `Element children` 递归结构，难以优雅支持 table、图片/mention（inline void）、复杂嵌套、块级/行内级 schema 约束。
- **编辑语义难以组合**：缺少 operation（InsertNode/RemoveNode/SplitNode/MergeNode/SetMark/SetAttr…）这一层，导致复杂能力只能写成特例逻辑，插件之间也难以组合、复用与覆写。
- **normalize 体系不完整**：目前主要依靠局部 normalize（如合并相邻同样式的 TextNode）与手写特例来维持结构合法，难以扩展到复杂节点类型与跨节点约束。
- **序列化可扩展性不足**：现有 Slate JSON 互转对未知类型缺少“无损保留”的兜底策略；当插件体系出现后，必须保证未启用插件时也不丢数据（round-trip）。

结论：由于项目早期且无历史负担，我们不再规划“先做一个轻量 v1 插件化再升级”的路线，而是直接以未来目标架构为准绳重构；允许的“中间态”仅用于技术验证，并且应当与最终架构同构（例如在独立模块/独立 crate 内完成闭环后再替换旧实现）。
