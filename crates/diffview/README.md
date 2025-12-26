# diffview 设计结论

构建基于 gpui/gpui-component 的文本比对组件（支持 unified/split）是可行的，可复用 Zed 的部分能力，关键点如下：

- 核心依赖：使用 rope（如 Zed 的 buffer）存文本，diff 算法可用 `similar` 或直接复用 Zed 的 diff/hunk 逻辑；UI 由 gpui 布局 + gpui-component（滚动容器、按钮、列表/虚拟化）。
- unified 视图：单列滚动，按 hunk 渲染，带行号/背景色；可折叠上下文。
- split 视图：左右双滚动容器，共享滚动模型做同步；双侧行号和连接标识。
- 高亮与内联差异：可复用 Zed 的 Tree-sitter 高亮与行内差异切片，给 span 增加 diff 元数据做着色。
- 性能：行级虚拟化；异步预计算 hunk；缓存布局；防抖 resize/reflow。
- 复用自 Zed：现有 diff 计算、rope 文本模型、语法高亮管线可直接拿来；需要新实现的主要是 split 布局、双 gutter 与滚动同步。
- UI 控件建议：切换 unified/split、忽略空白、跳转 hunk、折叠大段未变更、键盘导航、可选评论入口。
