# DnD Virtual List / Tree（dnd_vlist & dnd_vtree）实现计划

> 目标：在现有 `gpui-dnd-list` / `gpui-dnd-tree`（基于 `gpui::uniform_list`）基础上，新增两套基于 `gpui-component::v_virtual_list` 的可拖拽重排组件：`gpui-dnd-vlist` 与 `gpui-dnd-vtree`。  
> 其中 `v` 表示 *virtual*（虚拟列表），核心差异是 **每一行/项支持自定义尺寸（高度）**，并保持长列表性能。

---

## 1. 价值（Why）

1. **性能与体验**：长列表/长树结构仍能保持流畅（窗口化渲染），且支持不同行高（例如：带多行描述、不同密度的行）。
2. **能力对齐**：与现有 `dnd_list` / `dnd_tree` 的 DnD/选择/指示器/禁用等能力对齐，降低业务迁移成本。
3. **可扩展性**：虚拟化 + 可变高度是构建 diff viewer / log viewer / 复杂树面板等组件的通用基础设施。

---

## 2. 可行性（Feasibility）

- `gpui-component` 已提供 `virtual_list`（`v_virtual_list`）与 `VirtualListScrollHandle`，并通过 `item_sizes: Rc<Vec<Size<Pixels>>>` 支持每项不同高度。
- `dnd_list` 的逻辑天然基于 “index 重排”，迁移成本低：将 `uniform_list` 替换为 `v_virtual_list`，并将滚动句柄替换为 `VirtualListScrollHandle` 即可。
- `dnd_tree` 的难点主要在拖拽时的 **命中计算**（y 坐标 → hovered index）与 **插入线 y 坐标**：原实现用固定 `row_height` 推导；虚拟可变高度需要使用 `item_sizes` 做前缀和计算。该改造是局部且可控的。

---

## 3. 范围（Scope）

### 3.1 必做（MVP，对标现有组件）

#### dnd_vlist
- 同列表内拖拽重排（Before/After）
- 选中态（点击选中）
- 禁用项不可拖拽/不可点击
- 支持拖拽手柄宽度（`drag_handle_width`）或整行拖拽（`drag_on_row`）
- 可选回调：
  - `can_drop(reorder, items, modifiers) -> bool`
  - `on_reorder(reorder, items)`
- **支持自定义行高**：每个 item 自带 `size/height`，并映射到 `v_virtual_list` 的 `item_sizes`

#### dnd_vtree
- 树结构拖拽重排（同 `dnd_tree` 的 gap+depth 方案）
- 展开/折叠（点击、键盘）
- 禁用项不可拖拽/不可点击
- 指示器样式（color/thickness/cap）
- 水平手势（向左提升一级、向右降低一级成为左侧兄弟子节点）
- **支持自定义行高**：每个节点可配置行高，扁平化 entries 时同步生成 `item_sizes`

### 3.2 暂不做（后续可迭代）
- 跨列表/跨树拖拽（move/copy）
- 自动边缘滚动策略增强
- 多选拖拽/批量移动
- 动态测量高度（自动布局测量并回写 item_sizes）

---

## 4. 设计要点（How）

### 4.1 关键 API 约定

- `VirtualList` 需要 `item_sizes: Rc<Vec<Size<Pixels>>>`
  - 纵向列表仅使用 `height`
  - 要求每项高度为正（或最小值钳制到 `>= 1px`）
- `DndVListItem` / `DndVTreeItem` 增加 `height/size` builder，用于业务显式指定。

### 4.2 关键算法（Tree 可变高度）

将原先的：
- `hovered_ix = floor(y / row_height)`
- `line_y = row_height * gap_index + scroll_y`

替换为基于 item_sizes 的前缀和：
- `hovered_ix = first index where prefix(i+1) > y`
- `line_y = prefix(gap_index) + scroll_y`

其中 `prefix(k) = sum(height[0..k])`。

---

## 5. 迭代计划（Milestones）

### Iteration 0：项目结构
1. 新建 crates：
   - `crates/dnd_vlist`（crate 名：`gpui-dnd-vlist`）
   - `crates/dnd_vtree`（crate 名：`gpui-dnd-vtree`）
2. 更新 workspace：
   - `Cargo.toml` 的 `workspace.members`
   - `workspace.dependencies` 增加两项 path 依赖

### Iteration 1：实现 `gpui-dnd-vlist`
1. 复用 `dnd_list` 的 DnD/选择/禁用/回调结构
2. 替换渲染为 `gpui_component::v_virtual_list`
3. 引入 `DndVListItem::height/size` 并在重排时保持尺寸随 item 一起移动
4. 增加基本单元测试（重排后顺序与尺寸一致）

### Iteration 2：实现 `gpui-dnd-vtree`
1. 复用 `dnd_tree` 的树结构与 drop preview 逻辑（gap+depth）
2. 引入 `DndVTreeItem::row_height/size`
3. 将拖拽命中与插入线 y 坐标改为使用前缀和（支持可变高度）
4. 保持键盘导航与 `scroll_to_item` 行为一致

### Iteration 3：验证与文档补齐
1. `cargo test` / `cargo check` 验证编译通过
2. 在文档中补充：
   - `item_sizes`/自定义高度的使用说明
   - 与 `dnd_list/dnd_tree` 的差异点

---

## 6. 验收标准（Acceptance）

1. 两个 crate 可在 workspace 内正常编译通过。
2. `dnd_vlist`：拖拽重排在可变高度下表现稳定，目标行指示正确，禁用项不可交互。
3. `dnd_vtree`：在可变高度下：
   - 拖拽 hover 命中正确（不会因为行高不同而错位）
   - 插入线 y 坐标与视觉落点一致
   - 水平手势与深度推导逻辑保持一致
4. 对外 API 风格与 `dnd_list/dnd_tree` 一致，便于迁移与复用。

