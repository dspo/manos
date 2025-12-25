# DnD List 组件设计文档（Drag & Drop Reorderable List）

> 面向：产品经理 / UI 设计师 / UI 开发人员  
> 状态：草案（v0.1）  
> 更新时间：2025-12-22  
> 目标：定义一个“可拖拽重排”的通用 List 组件规范（交互、视觉、API、实现要点、验收标准）

---

## 1. 组件定义与边界

### 1.1 定义

**DnD List**（也常被称为 Sortable List / Reorderable List）是一个允许用户通过拖拽（Drag & Drop）改变列表项顺序的基础组件。它通常用于“顺序即语义”的数据，例如：待办优先级、播放列表、图层面板、标签页、字段排序等。

### 1.2 与相近组件的边界

- **与普通 List**：DnD List 在“指针拖拽 + 放置”链路上提供状态管理与视觉反馈；其余渲染仍然是列表。
- **与 DnD Tree**：DnD Tree = DnD List + 层级（depth）推导 + 结构重排；DnD List 只处理一维顺序重排。
- **与 OS 级拖拽**：本组件只负责“应用内重排/移动”，不覆盖文件拖拽、跨应用拖拽等系统能力（除非另有专项设计）。

---

## 2. 典型使用场景（PM 视角）

### 2.1 目标用户与收益

- 终端用户：通过拖拽快速调整顺序，降低“多次点击/多步操作”的成本。
- 业务方：用统一组件承载不同业务的排序能力，减少重复实现与交互不一致。
- 开发团队：获得可组合、可扩展、可测试的排序基础设施（含无障碍与性能约束）。

### 2.2 使用场景清单

- 任务/卡片：调整优先级（高频）。
- 播放列表：调整播放顺序（高频）。
- 设置面板：字段排序、快捷入口排序（中频）。
- 编辑器：tab 顺序、侧边栏条目顺序、图层列表顺序（高频，且对“命中稳定性”要求高）。
- 配置化页面：模块排序（中频，需支持锁定项/分组）。

---

## 3. 术语

- `Item`：列表项（可重排的最小单位）。
- `Handle`：拖拽手柄（建议默认启用，避免与行内交互冲突）。
- `Ghost`：拖拽中跟随指针的浮层（拖拽影子）。
- `Placeholder`：被拖拽项原位置的占位（可选；虚拟列表场景常用“插入线 overlay”替代）。
- `Drop Indicator`：落点指示（插入线/高亮区域/占位块）。
- `Drop Preview`：拖拽过程中实时计算的“预期落点”。
- `Source / Target`：拖拽源列表 / 目标列表（跨列表场景）。

---

## 4. 产品需求（PM 视角）

### 4.1 MVP 功能范围

1. **同列表重排**：拖拽任意可拖拽项到新位置，释放后完成重排。
2. **稳定的落点反馈**：拖拽过程中始终可见“将插入到哪里”（插入线或占位）。
3. **可配置的拖拽启动区域**：
   - 默认：仅 `Handle` 可启动拖拽（推荐）。
   - 可选：整行可拖拽（适合行内无交互或只读列表）。
4. **禁用与锁定**：
   - 禁用项不可拖拽。
   - 锁定项不可被插入到其“禁止区域”（例如：固定在顶部/底部、固定分组头）。
5. **取消与容错**：
   - `Esc` 取消拖拽并恢复原顺序。
   - 拖到列表外释放：不改变顺序（除非是跨列表设计）。
6. **滚动容器支持**：拖拽到容器边缘自动滚动（长列表必需）。
7. **回调/事件**：抛出 `onReorder`（或同等语义）事件，携带 `from/to` 信息，交由上层落地数据变更与持久化。

### 4.2 增强能力（可分期）

- 跨列表拖拽移动（move）/复制（copy，配合修饰键）。
- 多选重排（批量移动）。
- 分组与分隔符：支持“只能在同组内重排”或“允许跨组但有规则”。
- 受限重排：仅允许在某些 index 范围内移动；或根据业务规则动态判定。
- 撤销/重做集成：组件提供“操作描述 + 变更集”，方便上层接入 undo/redo。

### 4.3 验收标准（建议作为 QA Checklist）

- 任意可拖拽项：拖拽到列表任意位置后释放，顺序与落点一致。
- 拖拽到顶部/底部：可插入到第一个/最后一个位置。
- 快速拖动：落点指示不抖动、不丢失，命中稳定。
- 自动滚动：指针靠近边缘时滚动速度平滑、可控；离开边缘后停止。
- 禁用项：无法开始拖拽；视觉上明确（灰态/禁用光标）。
- 锁定规则：落点若不允许，显示“不可放置”反馈且释放不改变顺序。
- 取消：`Esc` 能撤销本次拖拽（无副作用）。
- 行内交互：点击按钮/输入框不会误触拖拽（默认使用 Handle 时必须满足）。
- 性能：在“长列表 + 虚拟化”下拖拽保持流畅；拖拽过程只做增量更新。
- 可访问性：键盘可完成重排（见第 5.6）。

---

## 5. 交互与视觉规范（UI 设计师视角）

### 5.1 结构建议（可组合）

每行建议包含：

- 左侧 `Handle`（≡/⋮⋮）：最小点击区域建议 ≥ 24×24（桌面）/ ≥ 32×32（触控）。
- 主内容区：标题、图标、描述等。
- 右侧操作区：更多、删除、开关等（避免在操作区启动拖拽）。

### 5.2 状态设计（建议用主题 token 表达）

| 状态 | 行背景 | 文本 | 光标 | 说明 |
|---|---|---|---|---|
| Default | `list.item.bg` | `text.primary` | default | 常态 |
| Hover | `list.item.bg_hover` | 同上 | default / grab(Handle) | 行 hover；Handle 可提示可拖拽 |
| Active(Pressed) | `list.item.bg_active` | 同上 | grabbing(Handle) | 按下/开始拖拽 |
| Dragging Source | 透明/降低不透明度 | `text.muted` | grabbing | 原位置行弱化或隐藏，避免“双影” |
| Drop Allowed | `list.drop.allowed_bg`（可选） | 同上 | grabbing | 可放置区域反馈 |
| Drop Not Allowed | `list.drop.denied_bg`（可选） | 同上 | not-allowed | 禁止放置反馈 |
| Disabled | `list.item.bg_disabled` | `text.disabled` | not-allowed | 不可交互 |

### 5.3 落点指示（Drop Indicator）

推荐优先级（从稳定到复杂）：

1. **插入线 overlay（推荐）**：2px 高亮线 + 可选圆角端点，不改变布局，最不容易引起虚拟列表抖动。
2. **占位块（Placeholder）**：在目标位置撑开空间，直观但对虚拟列表/统一行高不友好。

建议规范：

- 插入线厚度：2px（高 DPI 可做 2px/3px 自适应）。
- 颜色：主题强调色或 `drop_target` 专用色，需满足对比度要求。
- 与行内容对齐：若行有缩进/图标列，插入线起点应与内容列对齐（避免视觉错位）。

### 5.4 Ghost（拖拽浮层）

建议：

- 外观：跟随主题的 Popover 卡片（圆角 + 边框 + 阴影），可复用现有 token（如 `popover/bg/border/shadow`）。
- 内容：至少展示主标题；可附带小图标/副标题，但应避免过高复杂度。
- 尺寸：不强制与行等宽，但建议不超过容器宽度；过宽会遮挡落点指示。
- 透明度：100% 或 95%（避免“穿透”造成阅读干扰）。

### 5.5 动效（Motion）

- 行位移动画：建议 120–180ms，`ease-out`，让插入位置变化更可读。
- Ghost 跟随：建议无缓动或极短缓动（< 50ms），避免“拖不住”的感觉。
- 取消/释放：回弹/落位动画 150–220ms（可选）。

### 5.6 键盘与无障碍（A11y）

推荐键盘模型（不依赖 Web 特定 ARIA 属性，也适用于桌面 UI）：

- `Tab/Shift+Tab`：聚焦到某一行或其 Handle。
- `Space/Enter`：进入“搬运模式”（Pick up）；再次按下执行 Drop。
- `↑/↓`（或 `←/→` 水平列表）：在搬运模式下移动目标位置并实时更新 Drop Indicator。
- `Esc`：取消搬运模式并还原。
- 读屏提示：进入/移动/放置时，通过可读文本提示当前序号变化（例如“已拾起：Item A，当前位置 3/10；移动到 5/10”）。

---

## 6. API 与实现建议（UI 开发视角）

> 目标：API 要能覆盖 MVP；实现上可在“统一行高虚拟列表（uniform list）”下稳定工作，并为高级特性留扩展点。

### 6.1 组件 API（概念设计）

核心输入：

- `items: Vec<T>`：渲染数据（建议由上层持有，组件以受控方式工作）。
- `key: Fn(&T) -> Key`：稳定 key（避免重排导致状态错乱）。
- `render_item: Fn(RenderCtx, &T, ItemState) -> Element`：行渲染。

核心输出：

- `on_reorder: Fn(ReorderEvent<T>)`：
  - `from_index / to_index`
  - `from_key / to_key`（可选）
  - `position: Before/After`（可选；若用 gap index 则可省略）

核心配置：

- `direction: Vertical | Horizontal`
- `drag_activation: HandleOnly | WholeRow`
- `can_drag: Fn(&T) -> bool`
- `can_drop: Fn(DropContext<T>) -> bool`（用于锁定/分组规则）
- `auto_scroll: AutoScrollConfig`（阈值、速度曲线）
- `indicator: IndicatorStyle | CustomRenderer`
- `ghost: GhostStyle | CustomRenderer`

### 6.2 Drop Preview 算法（统一行高列表）

当列表使用统一行高虚拟化（例如 `uniform_list`）时，可用 O(1) 算法稳定计算落点：

1. 将鼠标 y 转为内容坐标：`y_in_content = (mouse_y - list_top) - scroll_y`
2. `hovered_ix = floor(y_in_content / item_height)`，并 clamp 到 `[0, item_count - 1]`
3. 用“上半/下半”决定 gap：
   - 上半：`gap_index = hovered_ix`
   - 下半：`gap_index = hovered_ix + 1`
   - 越界：`gap_index = 0`（顶部外）或 `item_count`（底部外）
4. 插入线位置：`line_y = gap_index * item_height + scroll_y`

注意：重排时要考虑“先移除再插入”的 index 偏移（若 `to_index > from_index`，插入点通常需要 `-1` 修正）。

> 实现变体（更贴近 Zed Tab Drag Reorder）：  
> 不做“上半/下半”判定，而是把“当前 hover 的目标 item”视作 drop target，并用 **目标 index 与被拖拽 index 的相对关系** 决定插入点：  
> - `target_ix < from_ix`：插入到目标项之前（`gap_index = target_ix`，插入线在目标项上方）  
> - `target_ix > from_ix`：插入到目标项之后（`gap_index = target_ix + 1`，插入线在目标项下方）  
> 优点：无需依赖 `scroll_y/item_height` 推导、命中稳定、体验更像 Zed；缺点：用户需要理解“hover 哪个 item 就是在该 item 边缘插入”的语义。

### 6.3 事件绑定与命中（以 GPUI DnD 模型为例）

在 GPUI 的 DnD 模型中，`on_drop::<T>` **只会在鼠标抬起时命中的 hitbox 上触发**。为了避免“落到空白处收不到 drop”的问题，建议：

- 在列表容器（虚拟列表根元素）上绑定 `on_drop::<DragValue>`：覆盖空白区域。
- 在每一行上也绑定 `on_drop::<DragValue>`：覆盖“落到某行”的 drop。

同理，`on_drag_move::<T>` 建议绑定在覆盖区域最大的元素上，用于持续计算 `Drop Preview`；若采用上文 “Zed Tab 风格” 的变体，则可主要依赖 `drag_over + on_drop`，减少对 `on_drag_move` 的依赖。

### 6.4 状态管理建议

最小状态集合：

- `dragged_key`：当前被拖拽的 item 标识（用于行弱化/禁止自身落点等）。
- `drop_preview`：`gap_index` + `line_y` + `allowed/denied` 等。
- `scroll_handle`：用于读写滚动偏移（自动滚动）。

渲染建议：

- 插入线使用 `absolute overlay` 叠加绘制，避免改变行高导致虚拟列表抖动（与 `docs/dnd-tree.md` 中的实践一致）。
- 拖拽源行建议降低不透明度或隐藏（避免“行 + ghost”双重视觉）。

### 6.5 自动滚动（Auto-scroll）

推荐策略：

- 设定 `edge_threshold`（例如 24px/32px）。
- 当指针进入阈值区：
  - 速度随“距离边缘越近越快”线性/指数增长，封顶 `max_speed`。
  - 每帧或定时器更新滚动偏移，并触发 `drop_preview` 重新计算。
- 指针离开阈值区：停止滚动。

### 6.6 可扩展点（为后续特性留口）

- 跨列表：`DragValue` 携带 `source_list_id`；`can_drop` 可根据来源决定 move/copy。
- 分组/锁定：`can_drop(DropContext)` 内做规则判定，并在 `drop_preview` 上反馈 denied。
- 多选：`DragValue` 携带多个 key；落地时批量重排。

### 6.7 测试与示例建议

- 单元测试：覆盖“from/to 修正”“禁用/锁定规则”“空列表/边界插入”等纯算法逻辑。
- 交互示例（Story/App）：至少提供
  - 长列表（验证自动滚动）
  - 含禁用项/锁定项
  - 行内含按钮/输入框（验证 HandleOnly）
  -（可选）跨列表 demo

---

## 7. 边界场景清单（实现与验收共用）

- 空列表：拖入时的落点表现（插入线在 index 0 或容器高亮）。
- 拖到自身：释放后无变化（不触发或触发但上层忽略）。
- 快速掠过：`drop_preview` 更新节流（避免过度 notify/重绘）。
- 数据源在拖拽中变化：被拖拽项被删除/过滤，需安全取消或重算。
- 嵌套滚动容器：列表在面板内滚动，拖拽时命中与滚动需一致。
- 混合输入：触控 + 鼠标；长按启动拖拽的策略差异。
- RTL / 水平列表：指标方向、键盘方向与布局方向一致。

---

## 8. 参考与对标

GPUI 框架由编辑器 Zed 提供。
Zed 已实现了 Tab drag + reorder，即它事实上已经实现了 dnd list。
Zed 仓库位于 `/Users/chenzhongrun/projects/gpui-project/zed`.
请参考 Zed 中 dnd list 的实现，在本项目中实现 dnd list Component。
以下是详细说明。

Tab drag + reorder is implemented in the workspace pane logic, with the tab UI wiring and drop handling all living in `crates/workspace/src/pane.rs`.

- Tab UI builds: `render_tab_bar` splits pinned/unpinned tabs and adds an empty trailing drop target; it wires `drag_over` and `on_drop` on the tabs container so a drop after the last tab lands at `items.len()` (`crates/workspace/src/pane.rs:3068-3348`).
- Per-tab drag sources/targets: each tab is created in `render_tab` with `.on_drag` carrying a `DraggedTab { pane, item, ix, detail, is_active }`, hover styling that shows a left/right border depending on whether the dragged tab came from the left/right, and drop handlers that call `handle_tab_drop(dragged_tab, ix, …)` (`crates/workspace/src/pane.rs:2650-2727`). Dragged tab preview is rendered via `impl Render for DraggedTab` using a lightweight `Tab` (`crates/workspace/src/pane.rs:4420-4438`).
- Split detection during drag: the main pane content subscribes to `on_drag_move` for `DraggedTab` and updates `drag_split_direction` when the cursor nears an edge (`crates/workspace/src/pane.rs:4032-4110` uses `handle_drag_move` at `crates/workspace/src/pane.rs:3366-3419`). A full-overlay div shows the prospective split highlight based on that direction (`crates/workspace/src/pane.rs:4082-4128`).
- Dropping logic: `handle_tab_drop` optionally splits the pane first (using `drag_split_direction`), then reorders or moves the item with `workspace::move_item`, honoring modifier-keys to clone instead of move, and adjusts `pinned_tab_count` when moving between pinned/unpinned regions or panes (`crates/workspace/src/pane.rs:3422-3514`). The underlying move/insert is in `crates/workspace/src/workspace.rs:8730-8794`.
- Component shell: the tab strip chrome itself (layout, scroll, start/end slots) is the reusable `TabBar` component in `crates/ui/src/components/tab_bar.rs`, while the tab visuals come from `crates/ui/src/components/tab.rs`.

There is a large battery of drag-related tests for pinned/unpinned moves and splits in `crates/workspace/src/pane.rs` (see the drag_* tests around lines ~5090+), exercising `handle_tab_drop`’s behavior.

---

## 9. 本仓库落地情况（v1）

本设计文档已在本仓库落地为两个可复用组件 crate，并提供 story/example 验收入口。

### 9.1 Crates 与示例入口

- **DnD List**：`crates/dnd_list`（crate 名：`gpui-dnd-list`）
  - 示例：`cargo run --example dnd_list`
  - story 页面：`crates/story/src/dnd_list.rs`
- **DnD Tree**：`crates/dnd_tree`（crate 名：`gpui-dnd-tree`，基于“list + depth 缩进”实现）
  - 示例：`cargo run --example dnd_tree`
  - 说明文档：`docs/dnd-tree.md`

### 9.2 关键设计点与实际 API 的对应关系

- **拖拽启动区域（HandleOnly / WholeRow）**
  - `DndListState::drag_handle_width(px(...))` / `drag_on_row()`
  - `DndTreeState::drag_handle_width(px(...))` / `drag_on_row()`
- **重排回调（onReorder）**
  - `DndListState::on_reorder(|reorder, items| ...)`（并可用 `can_drop(...)` 做规则拦截）
  - `DndTreeState` 当前为内部落地（直接改树结构并重建 entries）；若业务侧需要受控模式，可在此基础上再封装回调式 API
- **Drop Indicator**
  - `gpui-dnd-list`：采用 `drag_over` 时对目标行加背景 + 上/下边框（更贴近 Zed 的 tab reorder 语义）
  - `gpui-dnd-tree`：采用 absolute overlay 插入线，并支持 `indicator_style/indent_offset/indent_width` 配置（见 `docs/dnd-tree.md`）

### 9.3 已知差异 / 待增强项

- `gpui-dnd-tree` 的插入线目前仅支持实线（颜色/粗细/端点可配），暂不支持虚线/点线（可扩展为自定义 renderer）。
- 自动滚动、hover 延时自动展开等属于增强体验能力，当前未内置（见 `docs/dnd-tree.md` 第 9 节）。
