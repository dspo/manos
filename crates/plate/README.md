# gpui-manos-plate

GPUI 富文本编辑器视图层，基于 `gpui-plate-core` 构建。

## 架构概述

本项目采用双层架构：

- **Core 层**（`crates/plate-core`）：文档模型、编辑语义、插件系统
- **View 层**（`crates/plate`）：GPUI 视图、输入处理、IME、渲染

## 与 plate.js 的对比分析

### 插件扩展点对比

| 扩展点 | 本项目 | plate.js | 说明 |
|--------|--------|----------|------|
| 节点类型定义 | ✅ `node_specs()` | ✅ `isInline`/`isVoid` | 定义 Block/Inline、void、children 约束 |
| 事务转换 | ✅ `transaction_transforms()` | ✅ `withOverrides` | 事务应用前的转换 |
| 结构校验 | ✅ `normalize_passes()` | ✅ `normalizeNode` | 文档结构校验和修复 |
| 命令系统 | ✅ `commands()` | ✅ `commands` | 编辑器命令 |
| 状态查询 | ✅ `queries()` | ✅ `queries` | 状态查询 |
| 插件组合 | ❌ | ✅ `plugins` | 插件嵌套组合 |
| 自定义渲染 | ❌ | ✅ `renderElement`/`renderLeaf` | 节点渲染组件 |
| 事件处理器 | ❌ | ✅ `handlers` | onKeyDown/onPaste/onDrop 等 |
| 装饰器 | ❌ | ✅ `decorate` | 语法高亮、搜索高亮 |
| 行为覆写 | ❌ | ✅ `insertBreak`/`deleteBackward` | 特定行为覆写 |
| React Hooks | ❌ | ✅ `useHooks` | React hooks 集成 |
| 依赖注入 | ❌ | ✅ `inject` | 插件间依赖注入 |
| 快捷键绑定 | ❌ | ✅ `shortcuts` | 插件声明快捷键 |

### 能力对比

| 能力 | plate.js | 本项目 |
|------|----------|--------|
| 事件处理器扩展 | ✅ onKeyDown/onPaste/onDrop/onCopy | ❌ 硬编码在 view 层 |
| 装饰器系统 | ✅ decorate API | ❌ 未实现 |
| 自定义渲染 | ✅ renderElement/renderLeaf | ⚠️ 部分支持 |
| 协作编辑 | ✅ Yjs 集成 | ❌ 未实现 |
| 快捷键绑定 | ✅ 插件可声明 shortcuts | ❌ 需在 gpui Action 层硬编码 |
| Placeholder | ✅ 插件级别支持 | ❌ 未实现 |
| Soft Break | ✅ Shift+Enter 软换行 | ❌ 未实现 |
| Exit Break | ✅ 从特殊块退出 | ⚠️ 部分支持 |

### 插件数量对比

| 分类 | 本项目 | plate.js |
|------|--------|----------|
| 核心 | Paragraph、Divider、Heading、CodeBlock、Blockquote | ✅ 同等支持 |
| 列表 | List、Toggle、Todo | ✅ 同等支持 + indent-list |
| 表格 | Table | ✅ 同等支持 + caption |
| 富内容 | Mention、Emoji、Image、Math | ✅ 同等支持 + media-embed、excalidraw |
| 格式化 | Bold、Italic、Underline、Strikethrough、Code、Highlight、TextColor | ✅ 同等支持 + kbd |
| 布局 | Column、Indent、Align、LineHeight、FontFamily、FontSize、FontWeight | ✅ 同等支持 |
| 功能 | FindReplace、SlashCommand、Dnd、Markdown、BlockSelection | ✅ 同等支持 |
| 协作 | ❌ | ✅ cursor-overlay、comments |
| 序列化 | Markdown | ✅ csv、html、md、docx |
| UI 组件 | ❌ | ✅ floating-toolbar、combobox、select |
| 其他 | ❌ | ✅ autoformat、juice、node-id、reset-node、tabbable、trailing-block |

**总计：本项目约 26 个，plate.js 约 50+ 个**

### 架构层面的差距

1. **gpui 的静态类型限制**：gpui 的 Action 是编译期静态类型，无法像 plate.js 那样运行时动态注册新 Action。本项目采用"编译期可插拔 + 运行期可配置"的折中方案。

2. **渲染层耦合**：plate.js 的渲染完全由插件控制（React 组件），本项目的渲染逻辑仍部分硬编码在 `RichTextState` 中。

3. **协作编辑**：plate.js 有成熟的 Yjs 集成方案，本项目尚未涉及 CRDT/OT。

### 建议的改进方向

1. **添加事件处理器扩展点**：让插件可以注册 `on_key_down`、`on_paste` 等处理器
2. **实现装饰器系统**：支持搜索高亮、拼写检查等场景
3. **渲染层插件化**：让插件可以提供自定义的节点渲染逻辑
4. **协作编辑支持**：集成 CRDT 库（如 yrs）
5. **更多序列化格式**：HTML、DOCX 导入导出

## 使用方式

```bash
# 启动 Story Gallery 验证
cargo run
# 在左侧选择 "Rich Text"
```

## 相关文档

- [插件系统架构](../../docs/richtext-plugin-system-plan.md)
- [扩展性指南](../../docs/richtext-extensibility.md)
- [工具栏组件](../../docs/plate-toolbar-buttons.md)
