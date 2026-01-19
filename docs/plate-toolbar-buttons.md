# Plate Toolbar Buttons（GPUI 组件实现教程）

本教程介绍如何在本仓库中使用/扩展 `plate.js` 风格的 Toolbar Buttons，并说明这些组件是如何用 `gpui + gpui-component` 的方式组织起来的。

这些组件也将作为 RichText 编辑器“工具层插件（Tooling Plugins）”的重要组成部分；编辑器未来架构与插件系统计划见：
- [RichText：插件系统与未来架构计划（激进重构版）](richtext-plugin-system-plan.md)

## 运行预览

本仓库提供了一个集中预览页面：

- 运行：`cargo run`，在 Story Gallery 左侧选择：`Plate Toolbar`
- 入口：`crates/story/src/plate_toolbar_buttons.rs`

## 组件与模块位置

- 组件库：`crates/extras`
  - SVG 资源：`crates/extras/src/assets.rs`
  - Toolbar 组件：`crates/extras/src/plate_toolbar.rs`
- Story（预览页）：`crates/story/src/plate_toolbar_buttons.rs`

对外 API：

- 资源：`gpui_manos_components::assets::ExtrasAssetSource`
- Toolbar：`gpui_manos_components::plate_toolbar::*`

## 关键点 1：让 SVG 能显示（资产源）

GPUI 的 `svg().path("...")` 会从 `App` 的 `AssetSource` 里加载文件内容，所以要在 `main` 里配置资源。

本仓库用 `ExtrasAssetSource` 把 `crates/extras/assets/icons/*.svg` 编译进二进制并提供给 GPUI：

```rust
use gpui::*;
use gpui_manos_components::assets::ExtrasAssetSource;

fn main() {
    let app = Application::new().with_assets(ExtrasAssetSource::new());
    app.run(|cx| {
        gpui_component::init(cx);
        // ...open_window
    });
}
```

参考实现：

- `crates/story/src/main.rs`
- `crates/story/src/plate_toolbar_buttons.rs`

## 关键点 2：Icon 的复用方式

`PlateIconName` 对齐了 `plate.js` 示例中的 SVG（本质是 lucide icons），并实现了 `gpui_component::IconNamed`：

- SVG 文件：`crates/extras/assets/icons/*.svg`
- 路径映射：`crates/extras/src/plate_toolbar.rs`（`impl IconNamed for PlateIconName`）

使用方式：

```rust
use gpui_manos_components::plate_toolbar::PlateIconName;
use gpui_component::Icon;

let icon = Icon::new(PlateIconName::Undo2);
```

如果要新增一个图标：

1. 添加 SVG 文件到 `crates/extras/assets/icons/xxx.svg`
2. 在 `crates/extras/src/assets.rs` 的 `ASSETS` 数组里注册路径
3. 在 `crates/extras/src/plate_toolbar.rs` 的 `PlateIconName` 增加枚举值，并在 `IconNamed::path` 里映射到 `icons/xxx.svg`

## 关键点 3：Toolbar Button 的“gpui-component 风格”实现思路

以 `PlateToolbarButton` 为例（`crates/extras/src/plate_toolbar.rs`）：

- **Builder 风格**：struct + `#[derive(IntoElement)]` + `impl RenderOnce`
- **主题一致性**：在 `render()` 里读取 `cx.theme()`，用 `muted/accent/border/...` 等 token
- **状态样式**：
  - `Selectable`：`selected(true)` 走 `accent` 背景
  - `Disableable`：`disabled(true)` 降低透明度并阻断交互
- **编辑器友好**：在 `on_mouse_down` 里 `window.prevent_default()`，避免按钮抢焦点（更接近 Web 编辑器 Toolbar 的体验）

在它之上，组件库封装了几个常用形态，和 plate.js 的 HTML 对应关系如下：

- `PlateToolbarIconButton`：单图标按钮（Undo/Redo/Bold/Italic…）
- `PlateToolbarDropdownButton`：左侧内容 + 右侧 Chevron 的下拉触发器
- `PlateToolbarSplitButton`：主按钮 + 右侧小 Chevron（split button）
- `PlateToolbarStepper`：Minus + 输入框 + Plus（字号步进器）
- `PlateToolbarSeparator`：组间分隔线
- `PlateToolbarColorPicker`：字体色/高亮色的选色器（Popover + 颜色网格 + Hex 输入）

## 进阶：对齐下拉菜单与颜色选色器

### 对齐下拉菜单（Align）

`plate.js` 的 Align 是一个下拉菜单：触发按钮显示“当前对齐方式的图标 + Chevron”，点击后弹出对齐选项。

本仓库的实现思路是：

- 触发器用 `PlateToolbarDropdownButton`
- 弹层用 `gpui_component::popover::Popover`
- 菜单项用 `div()` 自己拼（hover/active/selected 走主题 token）

参考实现：

- `crates/story/src/richtext.rs`（`richtext-align-menu`）
- `crates/story/src/plate_toolbar_buttons.rs`（`plate-toolbar-align-menu`）

### 字体颜色（Text Color）

`PlateToolbarColorPicker` 是一个可复用组件，可用于字体颜色/高亮色：

```rust
use gpui_manos_components::plate_toolbar::{PlateIconName, PlateToolbarColorPicker};

PlateToolbarColorPicker::new("text-color", PlateIconName::Baseline)
    .tooltip("Text color")
    .value(current_color)
    .on_change(move |color, window, cx| {
        // 把 color 应用到编辑器，并把焦点还给编辑器
    })
```

参考实现：

- `crates/story/src/richtext.rs`

## 代码示例：组合一个 Toolbar Group

```rust
use gpui_manos_components::plate_toolbar::*;

div()
    .flex()
    .items_center()
    .child(PlateToolbarIconButton::new("undo", PlateIconName::Undo2).tooltip("Undo"))
    .child(PlateToolbarIconButton::new("redo", PlateIconName::Redo2).tooltip("Redo").disabled(true))
    .child(PlateToolbarSeparator)
    .child(
        PlateToolbarDropdownButton::new("insert")
            .tooltip("Insert")
            .child(PlateIconName::ArrowDownToLine),
    );
```

更完整示例可直接参考：

- `crates/story/src/plate_toolbar_buttons.rs`
