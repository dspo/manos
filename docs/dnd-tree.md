# DnD Tree：在 GPUI 中实现可拖拽重排的树形组件（从原理到代码）

本文基于本仓库新增的 `gpui-dnd-tree` crate，讲解如何在 **GPUI / gpui-component** 的生态里实现一个支持：

- 同级重排（兄弟节点上下换位）
- 跨层级移动（从深层拖到浅层/从浅层拖到深层）
- 投放为子节点（默认任意节点可接收，也可用字段限制）

你可以一边看一边跑示例验收：

- 运行 story：`cargo run`
- 运行独立示例：`cargo run --example dnd_tree`
- 跑单元测试：`cargo test -p gpui-dnd-tree`

---

## 1. 基础思路：Tree = 扁平 List + depth 缩进

在 `gpui` / `gpui-component` 里，Tree 通常不会做“递归布局”，而是：

1. 把树按 **先序遍历（pre-order）** 拍平成一维 `entries`
2. 每行根据 `depth` 增加 `padding-left`（视觉缩进）
3. 用 `uniform_list` 做虚拟列表（只渲染可见区，性能稳定）

`gpui-dnd-tree` 延续这一思路：渲染层仍然是 List，只是在交互层补齐 DnD + 树结构重排。

---

## 2. GPUI 的 DnD API：必须掌握的 3 个点

### 2.1 拖拽源：`on_drag(value, ghost)`

在行元素上调用 `.on_drag(value, |value, cursor_offset, window, cx| ...)`：

- `value: T` 是任意 `'static` 类型（本组件用 `DndTreeDrag`）
- drag 开始时，GPUI 会把 `value` 存到 `cx.active_drag`
- 同时调用你的 closure，创建拖拽“影子”（ghost），用于显示拖拽中的浮层

本组件用一个轻量的 `DragGhost` 来显示当前拖拽节点的 label（见 `crates/dnd_tree/src/tree.rs`）。

### 2.2 拖拽过程：`on_drag_move::<T>(...)`

拖拽激活期间，注册了 `on_drag_move::<T>` 的元素会持续收到 `DragMoveEvent<T>`：

- `event.event.position`：鼠标在窗口中的坐标（x/y）
- `event.event.modifiers`：修饰键（Option/Alt、Shift…）
- `event.bounds`：该元素 hitbox 的 bounds
- `event.drag(cx)`：拿到当前 drag 的类型化值 `&T`

本组件用它来实时计算 `drop_preview`（插入线位置、inside 高亮等）。

### 2.3 Drop：`on_drop::<T>(...)` 的命中规则（非常重要）

`on_drop::<T>` **只会在鼠标抬起时命中的 hitbox 上触发**。这意味着：

- 你把 `on_drop` 绑在外层容器上，但外层 hitbox 很可能被子元素覆盖/遮挡（例如 `uniform_list` 或行），导致收不到 drop

因此 `gpui-dnd-tree` 采用更稳的策略：

- `uniform_list` 上绑定 `on_drop::<DndTreeDrag>`：覆盖列表空白区域
- 每一行也绑定 `on_drop::<DndTreeDrag>`：覆盖“落到某行”的 drop

这也是我们从 Zed 的 tab 重排交互里学习到的关键点之一：**DnD 事件不要指望“冒泡”，要绑在真正覆盖区域的元素上**。

---

## 3. 数据结构：树结构与可见行列表并存

文件：`crates/dnd_tree/src/tree.rs`

### 3.1 `DndTreeItem`：树节点（共享展开态）

`DndTreeItem` 持有 `id/label/children`，并把交互状态放在 `Rc<RefCell<_>>` 中：

- `expanded`：是否展开（影响 entries 拍平）
- `disabled`：是否禁用（禁用后不响应 click/drag）
- `can_accept_children`：是否可作为父节点（决定 inside drop 是否允许）

其中：

- `DndTreeItem::accept_children(bool)` 用来控制可否接收子节点
- 默认 `true`（满足“任意节点可接收”）

### 3.2 `DndTreeEntry`：可见行（item + depth + parent_id）

`entries` 是渲染输入，也是 DnD 推导目标的重要依据：

- `depth`：决定行缩进（由你的行 renderer 使用）
- `parent_id`：用于把“行上的插入”映射回真实树结构

### 3.3 `DndTreeState`：核心 state

关键字段：

- `root_items: Vec<DndTreeItem>`：真实树结构（drop 最终修改它）
- `entries: Vec<DndTreeEntry>`：可见的扁平行列表（每次结构变化后重建）
- `scroll_handle`：读 scroll offset 与 item height（`uniform_list` 记录）
- `drop_preview`：拖拽过程的预览（插入线位置/目标行）
- `indent_width/indent_offset`：用于把鼠标 x 推导为“目标 depth”，并绘制缩进插入线

你可以通过 builder 配置它们：

- `DndTreeState::indent_width(px(...))`
- `DndTreeState::indent_offset(px(...))`（纯视觉，对齐插入线）

---

## 4. 渲染结构：`uniform_list` + 行 wrapper

文件：`crates/dnd_tree/src/tree.rs`

整体结构是：

- 列表：`uniform_list("entries", entries.len(), ...)`
  - 绑定：`.on_drag_move::<DndTreeDrag>(...)`、`.on_drop::<DndTreeDrag>(...)`
  - `track_scroll(self.scroll_handle.clone())` 用于读 scroll offset 与 item height
- 行 wrapper：`div().id(ix)`
  - `.on_click(...)`：示例里点击会选中并切换展开
  - `.on_drag(...)`：启动拖拽
  - `.on_drop::<DndTreeDrag>(...)`：保证落到行上能触发
- 插入线：在树容器上方用 `div().absolute()` 叠加绘制（不会改变行高，避免 `uniform_list` 抖动）

---

## 5. Drop 预览算法：Y 决定插入位置，X 决定层级

核心在 `DndTreeState::compute_drop_preview`。

### 5.1 用 X 推导目标层级（depth）

基本公式：

```
desired_depth = floor((mouse_x - list_left) / indent_width)
```

插入线的起始 x：

```
line_x = indent_offset + desired_depth * indent_width
```

示例（story）里行缩进用的是 `pl(10 + depth * 16)`，所以 story 将：

- `indent_width = 16px`
- `indent_offset = 10px`

对齐了插入线的视觉缩进。

### 5.2 用 Y 推导“插入缝隙”（gap）

`uniform_list` 的行高是统一的（第一行测量并缓存），所以：

- `hovered_ix = floor(y_in_content / item_height)`
- 行的 **上半/下半** 决定 gap 在行前还是行后

这里刻意不再用 “上 1/4 / 中间 / 下 1/4” 的三分区，因为三分区会让同级重排命中非常困难；改成上半/下半更稳定。

### 5.3 Inside（投放为子节点）的判定

两种方式都支持（更符合实际使用习惯）：

1. **在行的中间区域**（25%~75%）并且：
   - `desired_depth > target.depth`（向右拖动表示“想变深层”）
   - 或按住 `Option(Alt)` 强制 inside
2. **在两行之间投放**且 `desired_depth == prev.depth + 1`：
   - 这相当于“把节点投放到上一行作为父节点”

### 5.4 Before/After 的目标推导（跨层级移动的关键）

当不是 inside 时，需要把“gap + desired_depth”映射回树上的 Before/After：

- 从 gap 往下找第一个 `depth <= desired_depth` 的“边界行”
  - 如果边界行 `depth == desired_depth`：插到它 **Before**
  - 否则：插到 gap 之前最近的同 depth 行 **After**（相当于追加到该层最后）

### 5.5 After 的插入线必须落到“子树末尾”

Tree 的 `After(target)` 语义是：插入到 target 的同级 **并位于 target 整个子树之后**。

因此插入线位置不能用 `target_ix + 1`，而要用：

- `subtree_end_ix = first index after target where depth <= target.depth`
- `line_y = subtree_end_ix * item_height + scroll_y`

否则当 target 是展开的目录节点时，插入线会错误地落在“目录行与第一个子节点之间”。

---

## 6. Drop 落地：Remove + ComputeDestination + Insert + Rebuild

文件：`crates/dnd_tree/src/tree.rs`（`DndTreeState::on_drop`）

核心步骤：

1. 递归移除被拖拽节点：`remove_item_recursive`
2. 根据 `drop_preview.target` 计算目标父节点与插入 index：`compute_destination`
3. 防止形成环（把节点塞进自己的子树）：`subtree_contains`
4. 插入并重建可见 entries：`insert_item_at` + `rebuild_entries`
5. inside 投放时自动展开目标父节点：`set_expanded`

---

## 7. Story 示例：验收与自定义

示例文件：

- UI：`crates/story/src/dnd_tree.rs`
- Example 入口：`crates/story/examples/dnd_tree.rs`

你可以重点关注：

- 行渲染如何用 `entry.depth()` 缩进
- `DndTreeState::indent_width/indent_offset` 如何与行缩进对齐
- 右侧 debug 树结构实时打印（验证重排是否正确）

---

## 8. 从 Zed 的 tab 重排学到什么（落点计算 + 视觉提示）

在 Zed 项目里，编辑器 tab 重排是一套非常成熟的 DnD 交互。建议你重点看：

- `crates/workspace/src/pane.rs`：tab 渲染与重排（`DraggedTab`）
- `crates/title_bar/src/system_window_tabs.rs`：系统窗口 tab 的 DnD

它们的共同点：

- 每个 tab 本身就是 drop target：`.drag_over::<DraggedTab>(...)` 提示插入位置、`.on_drop(...)` 直接根据目标 index 重排
- 视觉提示用“背景 + 边框”而不是改变布局（避免 jitter）

`gpui-dnd-tree` 的对应实践是：

- drop 监听绑定到 `uniform_list` 与行（避免 hitbox 被遮挡时丢 drop）
- 插入线用 absolute overlay（不改变行高）
- Before/After 目标行使用 `theme.drop_target` 做背景提示

---

## 9. 可继续增强的方向

如果你想继续把手感做到更像 IDE/文件树，可以按优先级加：

1. **靠近边缘自动滚动**（拖拽到顶部/底部时自动 scroll）
2. **悬停自动展开**（hover 某节点一段时间自动 expanded）
3. **拖拽时忽略自身子树命中**（拖拽目录节点时更易跨越其子节点区域）
4. **可插拔 drop policy**（限制某些节点不可投放、跨树拖拽等）

