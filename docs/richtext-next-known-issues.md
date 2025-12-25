# RichText Next：已知问题（Tracked Issues）

本文件用于记录 `richtext_next` 示例与 `gpui-plate-core` 内核在当前阶段的已知问题，便于验收与后续迭代排期。

运行入口：`cargo run -p gpui-manos-components-story --example richtext_next`

## 已知问题

### 1) 双击选词：中文边界不理想

- 现象：双击选词对中文（以及部分 CJK 混排）选择边界不符合预期。
- 原因：当前使用简单的字符分类（word/whitespace/punct）做“近似选词”，未接入更完善的分词/字素簇规则。
- 影响范围：仅影响双击选词体验；不影响键盘输入/IME/undo/redo/序列化。
- 建议方向（后续迭代）：引入更合适的文本边界规则（例如字素簇/Unicode word boundary，必要时引入分词库作为可选特性）。

