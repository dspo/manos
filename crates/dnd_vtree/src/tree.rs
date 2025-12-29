use std::{cell::RefCell, ops::Range, rc::Rc};

use gpui::{
    App, AppContext as _, Context, CursorStyle, ElementId, Entity, EntityId, FocusHandle, Hsla,
    InteractiveElement as _, IntoElement, ListSizingBehavior, ParentElement as _, Pixels, Point,
    Render, RenderOnce, ScrollStrategy, SharedString, Size, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px, size,
};
use gpui_component::list::ListItem;
use gpui_component::scroll::{Scrollbar, ScrollbarState};
use gpui_component::{ActiveTheme as _, StyledExt as _, VirtualListScrollHandle, v_virtual_list};

const CONTEXT: &str = "DndVTree";
const HORIZONTAL_GESTURE_THRESHOLD_PX: f32 = 24.0;
const DEFAULT_ROW_HEIGHT: Pixels = px(28.);

/// Create a [`DndVTree`].
pub fn dnd_vtree<R>(state: &Entity<DndVTreeState>, render_item: R) -> DndVTree
where
    R: Fn(usize, &DndVTreeEntry, DndVTreeRowState, &mut Window, &mut App) -> ListItem + 'static,
{
    DndVTree::new(state, render_item)
}

#[derive(Clone)]
struct DndVTreeDrag {
    tree_id: EntityId,
    item_id: SharedString,
    label: SharedString,
}

struct DragGhost {
    label: SharedString,
}

impl DragGhost {
    fn new(label: SharedString) -> Self {
        Self { label }
    }
}

impl Render for DragGhost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .px(px(10.))
            .py(px(6.))
            .rounded(px(8.))
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .shadow_md()
            .text_color(theme.popover_foreground)
            .text_sm()
            .child(self.label.clone())
    }
}

#[derive(Clone)]
struct DndVTreeItemState {
    expanded: bool,
    disabled: bool,
    can_accept_children: bool,
    row_size: Size<Pixels>,
}

/// A tree item with children, and an expanded state.
#[derive(Clone)]
pub struct DndVTreeItem {
    pub id: SharedString,
    pub label: SharedString,
    pub children: Vec<DndVTreeItem>,
    state: Rc<RefCell<DndVTreeItemState>>,
}

impl DndVTreeItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children: Vec::new(),
            state: Rc::new(RefCell::new(DndVTreeItemState {
                expanded: false,
                disabled: false,
                can_accept_children: false,
                row_size: size(px(0.), DEFAULT_ROW_HEIGHT),
            })),
        }
    }

    pub fn child(mut self, child: DndVTreeItem) -> Self {
        self.state.borrow_mut().can_accept_children = true;
        self.children.push(child);
        self
    }

    pub fn children(mut self, children: impl Into<Vec<DndVTreeItem>>) -> Self {
        self.state.borrow_mut().can_accept_children = true;
        self.children.extend(children.into());
        self
    }

    pub fn expanded(self, expanded: bool) -> Self {
        self.state.borrow_mut().expanded = expanded;
        self
    }

    pub fn disabled(self, disabled: bool) -> Self {
        self.state.borrow_mut().disabled = disabled;
        self
    }

    /// Control whether this node can accept other nodes as its children during drop.
    ///
    /// Defaults to `false`. When you add a child via [`DndVTreeItem::child`] or
    /// [`DndVTreeItem::children`], it is automatically set to `true`.
    pub fn accept_children(self, accept_children: bool) -> Self {
        self.state.borrow_mut().can_accept_children = accept_children;
        self
    }

    /// Set this node's row size used by [`gpui_component::v_virtual_list`].
    ///
    /// For vertical lists, only `height` is used.
    pub fn row_size(self, row_size: Size<Pixels>) -> Self {
        self.state.borrow_mut().row_size = row_size;
        self
    }

    /// Set this node's row height (vertical axis size).
    pub fn row_height(self, height: Pixels) -> Self {
        self.state.borrow_mut().row_size.height = height;
        self
    }

    pub fn is_folder(&self) -> bool {
        !self.children.is_empty()
    }

    pub fn is_disabled(&self) -> bool {
        self.state.borrow().disabled
    }

    pub fn is_expanded(&self) -> bool {
        self.state.borrow().expanded
    }

    pub fn can_accept_children(&self) -> bool {
        self.state.borrow().can_accept_children
    }

    pub fn row_size_ref(&self) -> Size<Pixels> {
        self.state.borrow().row_size
    }
}

/// A flat representation of a tree item with its depth.
#[derive(Clone)]
pub struct DndVTreeEntry {
    item: DndVTreeItem,
    depth: usize,
    parent_id: Option<SharedString>,
}

impl DndVTreeEntry {
    #[inline]
    pub fn item(&self) -> &DndVTreeItem {
        &self.item
    }

    #[inline]
    pub fn depth(&self) -> usize {
        self.depth
    }

    #[inline]
    pub fn parent_id(&self) -> Option<&SharedString> {
        self.parent_id.as_ref()
    }

    #[inline]
    pub fn is_folder(&self) -> bool {
        self.item.is_folder()
    }

    #[inline]
    pub fn is_expanded(&self) -> bool {
        self.item.is_expanded()
    }

    #[inline]
    pub fn is_disabled(&self) -> bool {
        self.item.is_disabled()
    }

    #[inline]
    pub fn can_accept_children(&self) -> bool {
        self.item.can_accept_children()
    }

    #[inline]
    pub fn row_size(&self) -> Size<Pixels> {
        self.item.row_size_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DndVTreeDropTarget {
    Before,
    After,
}

#[derive(Clone, Copy, Debug)]
pub enum DndVTreeIndicatorCap {
    None,
    StartBar { width: Pixels, height: Pixels },
    StartAndEndBars { width: Pixels, height: Pixels },
}

#[derive(Clone, Copy, Debug)]
pub struct DndVTreeIndicatorStyle {
    pub color: Option<Hsla>,
    pub thickness: Pixels,
    pub cap: DndVTreeIndicatorCap,
}

impl Default for DndVTreeIndicatorStyle {
    fn default() -> Self {
        Self {
            color: None,
            thickness: px(2.),
            cap: DndVTreeIndicatorCap::StartBar {
                width: px(2.),
                height: px(10.),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DndVTreeRowState {
    pub selected: bool,
    pub dragging: bool,
    pub drop_target: Option<DndVTreeDropTarget>,
}

#[derive(Clone, Debug, PartialEq)]
struct DropPreview {
    gap_index: usize,
    depth: usize,
    highlight_ix: Option<usize>,
    highlight_target: Option<DndVTreeDropTarget>,
    line_y: Option<Pixels>,
    line_x: Option<Pixels>,
}

#[derive(Clone)]
struct RemovedItem {
    item: DndVTreeItem,
    parent_id: Option<SharedString>,
    index: usize,
}

/// State for managing DnD tree items (virtual, variable-height).
pub struct DndVTreeState {
    focus_handle: FocusHandle,
    root_items: Vec<DndVTreeItem>,
    entries: Vec<DndVTreeEntry>,
    entry_sizes: Rc<Vec<Size<Pixels>>>,
    entry_heights: Vec<Pixels>,
    entry_origins_y: Vec<Pixels>,
    indent_width: Pixels,
    indent_offset: Pixels,
    indicator_style: DndVTreeIndicatorStyle,
    scrollbar_state: ScrollbarState,
    scroll_handle: VirtualListScrollHandle,
    drag_handle_width: Option<Pixels>,
    selected_ix: Option<usize>,
    dragged_id: Option<SharedString>,
    dragged_ix: Option<usize>,
    drop_preview: Option<DropPreview>,
    drag_start_mouse_position: Option<Point<Pixels>>,
    render_item:
        Rc<dyn Fn(usize, &DndVTreeEntry, DndVTreeRowState, &mut Window, &mut App) -> ListItem>,
}

impl DndVTreeState {
    pub fn new(cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            root_items: Vec::new(),
            entries: Vec::new(),
            entry_sizes: Rc::new(Vec::new()),
            entry_heights: Vec::new(),
            entry_origins_y: vec![px(0.)],
            indent_width: px(16.),
            indent_offset: px(12.),
            indicator_style: DndVTreeIndicatorStyle::default(),
            scrollbar_state: ScrollbarState::default(),
            scroll_handle: VirtualListScrollHandle::new(),
            drag_handle_width: None,
            selected_ix: None,
            dragged_id: None,
            dragged_ix: None,
            drop_preview: None,
            drag_start_mouse_position: None,
            render_item: Rc::new(|_, _, _, _, _| ListItem::new("dnd-vtree-empty")),
        }
    }

    /// Set the indentation width (in pixels) used to infer the intended depth during DnD.
    ///
    /// This should match the indentation used by your row renderer.
    pub fn indent_width(mut self, indent_width: Pixels) -> Self {
        self.indent_width = indent_width;
        self
    }

    /// Set the left offset for the drop indicator line.
    ///
    /// This is purely visual and does not affect the actual tree indentation.
    pub fn indent_offset(mut self, indent_offset: Pixels) -> Self {
        self.indent_offset = indent_offset;
        self
    }

    /// Restrict drag start to a left-side handle area with the given width.
    ///
    /// This generally results in a better drag ghost alignment than whole-row dragging.
    pub fn drag_handle_width(mut self, width: Pixels) -> Self {
        self.drag_handle_width = Some(width);
        self
    }

    /// Allow dragging from anywhere on the row.
    pub fn drag_on_row(mut self) -> Self {
        self.drag_handle_width = None;
        self
    }

    /// Configure the drop indicator line style (color/thickness/caps).
    pub fn indicator_style(mut self, style: DndVTreeIndicatorStyle) -> Self {
        self.indicator_style = style;
        self
    }

    /// Override the drop indicator line color.
    pub fn indicator_color(mut self, color: Hsla) -> Self {
        self.indicator_style.color = Some(color);
        self
    }

    /// Override the drop indicator thickness.
    pub fn indicator_thickness(mut self, thickness: Pixels) -> Self {
        self.indicator_style.thickness = thickness;
        self
    }

    /// Override the drop indicator caps.
    pub fn indicator_cap(mut self, cap: DndVTreeIndicatorCap) -> Self {
        self.indicator_style.cap = cap;
        self
    }

    pub fn items(mut self, items: impl Into<Vec<DndVTreeItem>>) -> Self {
        self.root_items = items.into();
        self.rebuild_entries();
        self
    }

    pub fn set_items(&mut self, items: impl Into<Vec<DndVTreeItem>>, cx: &mut Context<Self>) {
        self.root_items = items.into();
        self.selected_ix = None;
        self.drop_preview = None;
        self.dragged_id = None;
        self.dragged_ix = None;
        self.drag_start_mouse_position = None;
        self.rebuild_entries();
        cx.notify();
    }

    pub fn root_items(&self) -> &[DndVTreeItem] {
        &self.root_items
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_ix
    }

    pub fn set_selected_index(&mut self, ix: Option<usize>, cx: &mut Context<Self>) {
        self.selected_ix = ix;
        cx.notify();
    }

    pub fn selected_entry(&self) -> Option<&DndVTreeEntry> {
        self.selected_ix.and_then(|ix| self.entries.get(ix))
    }

    /// Update the size of a single visible entry row.
    ///
    /// For vertical lists, only `height` is used.
    pub fn set_entry_size(&mut self, ix: usize, row_size: Size<Pixels>, cx: &mut Context<Self>) {
        let Some(entry) = self.entries.get(ix) else {
            return;
        };

        entry.item.state.borrow_mut().row_size = row_size;

        if ix >= self.entry_sizes.len() {
            self.rebuild_entries();
            self.drop_preview = None;
            cx.notify();
            return;
        }

        let mut sizes = self.entry_sizes.as_ref().clone();
        sizes[ix] = sanitize_row_size(row_size);
        self.entry_sizes = Rc::new(sizes);
        self.rebuild_entry_layout_cache();
        self.drop_preview = None;
        cx.notify();
    }

    /// Update the height of a single visible entry row.
    pub fn set_entry_height(&mut self, ix: usize, height: Pixels, cx: &mut Context<Self>) {
        let Some(entry) = self.entries.get(ix) else {
            return;
        };

        let mut row_size = entry.row_size();
        row_size.height = height;
        self.set_entry_size(ix, row_size, cx);
    }

    fn rebuild_entries(&mut self) {
        self.entries.clear();
        let mut sizes = Vec::new();

        for index in 0..self.root_items.len() {
            let item = self.root_items[index].clone();
            self.add_entry(item, 0, None, &mut sizes);
        }

        self.entry_sizes = Rc::new(sizes);
        self.rebuild_entry_layout_cache();
    }

    fn rebuild_entry_layout_cache(&mut self) {
        self.entry_heights.clear();
        self.entry_origins_y.clear();
        self.entry_origins_y.push(px(0.));

        for s in self.entry_sizes.iter() {
            let mut h = s.height;
            let hf: f32 = h.into();
            if !hf.is_finite() || hf <= 0.0 {
                h = px(1.);
            }
            self.entry_heights.push(h);

            let last = *self.entry_origins_y.last().unwrap_or(&px(0.));
            self.entry_origins_y.push(last + h);
        }

        if self.entry_origins_y.is_empty() {
            self.entry_origins_y.push(px(0.));
        }
    }

    fn add_entry(
        &mut self,
        item: DndVTreeItem,
        depth: usize,
        parent_id: Option<SharedString>,
        sizes: &mut Vec<Size<Pixels>>,
    ) {
        let item_id = item.id.clone();
        self.entries.push(DndVTreeEntry {
            item: item.clone(),
            depth,
            parent_id: parent_id.clone(),
        });
        sizes.push(sanitize_row_size(item.row_size_ref()));

        if item.is_expanded() {
            for child in item.children.iter().cloned() {
                self.add_entry(child, depth + 1, Some(item_id.clone()), sizes);
            }
        }
    }

    fn toggle_expand(&mut self, ix: usize) {
        let Some(entry) = self.entries.get_mut(ix) else {
            return;
        };
        if !entry.is_folder() {
            return;
        }
        entry.item.state.borrow_mut().expanded = !entry.is_expanded();
        self.rebuild_entries();
    }

    fn on_key_down(&mut self, event: &gpui::KeyDownEvent, cx: &mut Context<Self>) -> bool {
        if cx.has_active_drag() {
            return false;
        }

        if self.entries.is_empty() {
            return false;
        }

        let mut selected_ix = self.selected_ix.unwrap_or(0).min(self.entries.len() - 1);

        let select = |this: &mut Self, ix: usize, cx: &mut Context<Self>| {
            let ix = ix.min(this.entries.len().saturating_sub(1));
            this.selected_ix = Some(ix);
            this.scroll_handle
                .scroll_to_item(ix, ScrollStrategy::Center);
            cx.notify();
        };

        match event.keystroke.key.as_str() {
            "up" => {
                if selected_ix > 0 {
                    selected_ix -= 1;
                }
                select(self, selected_ix, cx);
                true
            }
            "down" => {
                if selected_ix + 1 < self.entries.len() {
                    selected_ix += 1;
                }
                select(self, selected_ix, cx);
                true
            }
            "home" => {
                select(self, 0, cx);
                true
            }
            "end" => {
                select(self, self.entries.len().saturating_sub(1), cx);
                true
            }
            "right" => {
                let Some(entry) = self.entries.get(selected_ix) else {
                    return false;
                };
                if entry.is_folder() && !entry.is_expanded() {
                    let selected_id = entry.item().id.clone();
                    self.toggle_expand(selected_ix);
                    if let Some(ix) = self
                        .entries
                        .iter()
                        .position(|entry| entry.item().id == selected_id)
                    {
                        select(self, ix, cx);
                    } else {
                        cx.notify();
                    }
                    return true;
                }

                if entry.is_folder() && entry.is_expanded() {
                    let child_ix = selected_ix.saturating_add(1);
                    if self
                        .entries
                        .get(child_ix)
                        .is_some_and(|child| child.depth() == entry.depth() + 1)
                    {
                        select(self, child_ix, cx);
                    }
                    return true;
                }

                false
            }
            "left" => {
                let Some(entry) = self.entries.get(selected_ix) else {
                    return false;
                };

                if entry.is_folder() && entry.is_expanded() {
                    let selected_id = entry.item().id.clone();
                    self.toggle_expand(selected_ix);
                    if let Some(ix) = self
                        .entries
                        .iter()
                        .position(|entry| entry.item().id == selected_id)
                    {
                        select(self, ix, cx);
                    } else {
                        cx.notify();
                    }
                    return true;
                }

                if let Some(parent_id) = entry.parent_id()
                    && let Some(parent_ix) = self
                        .entries
                        .iter()
                        .position(|entry| entry.item().id == *parent_id)
                {
                    select(self, parent_ix, cx);
                    return true;
                }

                false
            }
            "enter" | "space" => {
                let Some(entry) = self.entries.get(selected_ix) else {
                    return false;
                };
                if !entry.is_folder() {
                    return false;
                }

                let selected_id = entry.item().id.clone();
                self.toggle_expand(selected_ix);
                self.selected_ix = self
                    .entries
                    .iter()
                    .position(|entry| entry.item().id == selected_id);
                if let Some(ix) = self.selected_ix {
                    self.scroll_handle
                        .scroll_to_item(ix, ScrollStrategy::Center);
                }
                cx.notify();
                true
            }
            _ => false,
        }
    }

    fn on_entry_click(
        &mut self,
        ix: usize,
        _event: &gpui::ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_ix = Some(ix);
        self.toggle_expand(ix);
        cx.notify();
    }

    fn on_drag_start(&mut self, drag: &DndVTreeDrag, window: &mut Window, cx: &mut Context<Self>) {
        self.dragged_id = Some(drag.item_id.clone());
        self.dragged_ix = self
            .entries
            .iter()
            .position(|entry| entry.item().id == drag.item_id);
        self.drag_start_mouse_position = Some(window.mouse_position());
        self.drop_preview = None;
        self.selected_ix = self
            .entries
            .iter()
            .position(|entry| entry.item().id == drag.item_id);
        cx.notify();
    }

    fn on_drag_move(
        &mut self,
        event: &gpui::DragMoveEvent<DndVTreeDrag>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !cx.has_active_drag() {
            return;
        }

        let mouse_position = event.event.position;
        let list_bounds = event.bounds;
        if !list_bounds.contains(&mouse_position) {
            if self.drop_preview.take().is_some() {
                cx.notify();
            }
            return;
        }

        let drag = event.drag(cx);
        if drag.tree_id != cx.entity_id() {
            if self.drop_preview.take().is_some() {
                cx.notify();
            }
            return;
        }

        let (delta_x, delta_y) = self
            .drag_start_mouse_position
            .map(|start| (mouse_position.x - start.x, mouse_position.y - start.y))
            .unwrap_or((Pixels::ZERO, Pixels::ZERO));
        let start_depth = self
            .dragged_ix
            .and_then(|ix| self.entries.get(ix))
            .map(|entry| entry.depth())
            .unwrap_or(0);
        let desired_depth = self.desired_depth(start_depth, delta_x);
        let delta_x_f32: f32 = delta_x.into();
        let delta_y_f32: f32 = delta_y.into();
        let is_horizontal_gesture = delta_x_f32.abs() > HORIZONTAL_GESTURE_THRESHOLD_PX
            && delta_x_f32.abs() > delta_y_f32.abs();

        let new_preview = (|| {
            if self.entries.is_empty() {
                return Some(DropPreview {
                    gap_index: 0,
                    depth: 0,
                    highlight_ix: None,
                    highlight_target: None,
                    line_y: Some(px(0.)),
                    line_x: Some(self.indent_offset),
                });
            }

            let scroll_y = self.scroll_handle.offset().y;
            let y_in_content = mouse_position.y - list_bounds.origin.y - scroll_y;
            let hovered_ix = self.hovered_index_for_y(y_in_content);

            if hovered_ix >= self.entries.len() {
                return self.compute_drop_preview_for_gap(self.entries.len(), desired_depth);
            }

            let dragged_ix = self.dragged_ix?;
            let subtree_end_ix = self.subtree_end_ix(dragged_ix);
            if hovered_ix > dragged_ix && hovered_ix < subtree_end_ix {
                return None;
            }

            if hovered_ix == dragged_ix {
                if is_horizontal_gesture {
                    return self.compute_horizontal_gesture_preview(dragged_ix, delta_x);
                }
                return None;
            }

            let origin_y = self
                .entry_origins_y
                .get(hovered_ix)
                .copied()
                .unwrap_or(px(0.));
            let item_height = self.entry_height(hovered_ix);
            let y_in_row = (y_in_content - origin_y).max(px(0.));

            let drop_target = if y_in_row < item_height / 2.0 {
                DndVTreeDropTarget::Before
            } else {
                DndVTreeDropTarget::After
            };

            let gap_index = match drop_target {
                DndVTreeDropTarget::Before => hovered_ix,
                DndVTreeDropTarget::After => hovered_ix.saturating_add(1),
            };

            let mut preview = self.compute_drop_preview_for_gap(gap_index, desired_depth)?;
            if preview.highlight_ix.is_none() {
                let hovered_depth = self.entries.get(hovered_ix).map(|entry| entry.depth());
                if hovered_depth.is_some_and(|hovered_depth| preview.depth > hovered_depth)
                    && preview.depth > 0
                    && preview.gap_index > 0
                {
                    let mut parent_ix = None;
                    for ix in (0..preview.gap_index).rev() {
                        if self.entries[ix].depth() == preview.depth - 1 {
                            parent_ix = Some(ix);
                            break;
                        }
                    }
                    if let Some(parent_ix) = parent_ix {
                        preview.highlight_ix = Some(parent_ix);
                        preview.highlight_target = Some(DndVTreeDropTarget::After);
                    } else {
                        preview.highlight_ix = Some(hovered_ix);
                        preview.highlight_target = Some(drop_target);
                    }
                } else {
                    preview.highlight_ix = Some(hovered_ix);
                    preview.highlight_target = Some(drop_target);
                }
            }
            Some(preview)
        })();

        if self.drop_preview != new_preview {
            self.drop_preview = new_preview;
            cx.notify();
        }
    }

    fn on_drag_move_over_row(
        &mut self,
        hovered_ix: usize,
        event: &gpui::DragMoveEvent<DndVTreeDrag>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !cx.has_active_drag() {
            return;
        }

        if hovered_ix >= self.entries.len() {
            return;
        }

        let mouse_position = event.event.position;
        let row_bounds = event.bounds;
        if !row_bounds.contains(&mouse_position) {
            return;
        }

        let drag = event.drag(cx);
        if drag.tree_id != cx.entity_id() {
            if self.drop_preview.take().is_some() {
                cx.notify();
            }
            return;
        }

        let (delta_x, delta_y) = self
            .drag_start_mouse_position
            .map(|start| (mouse_position.x - start.x, mouse_position.y - start.y))
            .unwrap_or((Pixels::ZERO, Pixels::ZERO));
        let start_depth = self
            .dragged_ix
            .and_then(|ix| self.entries.get(ix))
            .map(|entry| entry.depth())
            .unwrap_or(0);
        let desired_depth = self.desired_depth(start_depth, delta_x);
        let delta_x_f32: f32 = delta_x.into();
        let delta_y_f32: f32 = delta_y.into();
        let is_horizontal_gesture = delta_x_f32.abs() > HORIZONTAL_GESTURE_THRESHOLD_PX
            && delta_x_f32.abs() > delta_y_f32.abs();

        let new_preview = (|| {
            if self.entries.is_empty() {
                return Some(DropPreview {
                    gap_index: 0,
                    depth: 0,
                    highlight_ix: None,
                    highlight_target: None,
                    line_y: Some(px(0.)),
                    line_x: Some(self.indent_offset),
                });
            }

            let dragged_ix = self.dragged_ix?;
            let subtree_end_ix = self.subtree_end_ix(dragged_ix);
            if hovered_ix > dragged_ix && hovered_ix < subtree_end_ix {
                return None;
            }

            if hovered_ix == dragged_ix {
                if is_horizontal_gesture {
                    return self.compute_horizontal_gesture_preview(dragged_ix, delta_x);
                }
                return None;
            }

            let item_height = self.entry_height(hovered_ix);
            let y_in_row = (mouse_position.y - row_bounds.origin.y).max(px(0.));
            let drop_target = if y_in_row < item_height / 2.0 {
                DndVTreeDropTarget::Before
            } else {
                DndVTreeDropTarget::After
            };

            let gap_index = match drop_target {
                DndVTreeDropTarget::Before => hovered_ix,
                DndVTreeDropTarget::After => hovered_ix.saturating_add(1),
            };

            let mut preview = self.compute_drop_preview_for_gap(gap_index, desired_depth)?;
            if preview.highlight_ix.is_none() {
                let hovered_depth = self.entries.get(hovered_ix).map(|entry| entry.depth());
                if hovered_depth.is_some_and(|hovered_depth| preview.depth > hovered_depth)
                    && preview.depth > 0
                    && preview.gap_index > 0
                {
                    let mut parent_ix = None;
                    for ix in (0..preview.gap_index).rev() {
                        if self.entries[ix].depth() == preview.depth - 1 {
                            parent_ix = Some(ix);
                            break;
                        }
                    }
                    if let Some(parent_ix) = parent_ix {
                        preview.highlight_ix = Some(parent_ix);
                        preview.highlight_target = Some(DndVTreeDropTarget::After);
                    } else {
                        preview.highlight_ix = Some(hovered_ix);
                        preview.highlight_target = Some(drop_target);
                    }
                } else {
                    preview.highlight_ix = Some(hovered_ix);
                    preview.highlight_target = Some(drop_target);
                }
            }
            Some(preview)
        })();

        if self.drop_preview != new_preview {
            self.drop_preview = new_preview;
            cx.notify();
        }
    }

    fn desired_depth(&self, start_depth: usize, delta_x: Pixels) -> usize {
        let indent_width_f32: f32 = self.indent_width.into();
        if !indent_width_f32.is_finite() || indent_width_f32 <= 0.0 {
            return start_depth;
        }

        let delta_x_f32: f32 = delta_x.into();
        if !delta_x_f32.is_finite() {
            return start_depth;
        }

        let depth_delta = (delta_x_f32 / indent_width_f32).trunc() as isize;
        let desired_depth = start_depth as isize + depth_delta;
        desired_depth.max(0) as usize
    }

    fn compute_horizontal_gesture_preview(
        &self,
        dragged_ix: usize,
        delta_x: Pixels,
    ) -> Option<DropPreview> {
        let dragged_entry = self.entries.get(dragged_ix)?;
        let dragged_depth = dragged_entry.depth();

        let scroll_y = self.scroll_handle.offset().y;
        let delta_x_f32: f32 = delta_x.into();

        if delta_x_f32 < 0.0 {
            let parent_id = dragged_entry.parent_id()?.clone();
            let parent_ix = self
                .entries
                .iter()
                .position(|entry| entry.item().id == parent_id)?;
            let parent_depth = self.entries.get(parent_ix)?.depth();
            let gap_index = self.subtree_end_ix(parent_ix);

            return Some(DropPreview {
                gap_index,
                depth: parent_depth,
                highlight_ix: Some(parent_ix),
                highlight_target: Some(DndVTreeDropTarget::After),
                line_y: Some(self.gap_y(gap_index) + scroll_y),
                line_x: Some(self.indent_offset + self.indent_width * parent_depth),
            });
        }

        if delta_x_f32 > 0.0 && dragged_ix > 0 {
            let parent_id = dragged_entry.parent_id().cloned();
            let mut ix = dragged_ix - 1;
            loop {
                let entry = self.entries.get(ix)?;
                if entry.depth() < dragged_depth {
                    break;
                }
                if entry.depth() == dragged_depth && entry.parent_id() == parent_id.as_ref() {
                    if !entry.can_accept_children() {
                        return None;
                    }

                    let gap_index = ix + 1;
                    let depth = entry.depth() + 1;
                    return Some(DropPreview {
                        gap_index,
                        depth,
                        highlight_ix: Some(ix),
                        highlight_target: Some(DndVTreeDropTarget::After),
                        line_y: Some(self.gap_y(gap_index) + scroll_y),
                        line_x: Some(self.indent_offset + self.indent_width * depth),
                    });
                }

                if ix == 0 {
                    break;
                }
                ix -= 1;
            }
        }

        None
    }

    fn subtree_end_ix(&self, start_ix: usize) -> usize {
        let item_count = self.entries.len();
        let start_depth = self.entries[start_ix].depth();
        let mut ix = start_ix + 1;
        while ix < item_count && self.entries[ix].depth() > start_depth {
            ix += 1;
        }
        ix
    }

    fn entry_height(&self, ix: usize) -> Pixels {
        self.entry_heights
            .get(ix)
            .copied()
            .unwrap_or(DEFAULT_ROW_HEIGHT)
    }

    fn gap_y(&self, gap_index: usize) -> Pixels {
        let clamped = gap_index.min(self.entries.len());
        self.entry_origins_y.get(clamped).copied().unwrap_or(px(0.))
    }

    fn hovered_index_for_y(&self, y_in_content: Pixels) -> usize {
        let item_count = self.entries.len();
        if item_count == 0 {
            return 0;
        }

        let y_f32: f32 = y_in_content.into();
        if !y_f32.is_finite() {
            return 0;
        }

        let y = y_in_content.max(px(0.));
        let content_end = self.entry_origins_y.last().copied().unwrap_or(px(0.));
        if y >= content_end {
            return item_count;
        }

        // `entry_origins_y` is strictly increasing (heights are clamped to >= 1px).
        let mut lo = 0usize;
        let mut hi = item_count;
        while lo < hi {
            let mid = (lo + hi) / 2;
            let end_y = self
                .entry_origins_y
                .get(mid + 1)
                .copied()
                .unwrap_or(content_end);
            if end_y <= y {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }

    fn compute_drop_preview_for_gap(
        &self,
        gap_index: usize,
        desired_depth: usize,
    ) -> Option<DropPreview> {
        let item_count = self.entries.len();
        if item_count == 0 {
            return Some(DropPreview {
                gap_index: 0,
                depth: 0,
                highlight_ix: None,
                highlight_target: None,
                line_y: Some(px(0.)),
                line_x: Some(self.indent_offset),
            });
        }

        let scroll_y = self.scroll_handle.offset().y;

        let max_depth = if gap_index == 0 {
            0
        } else {
            let prev = self.entries.get(gap_index - 1)?;
            if prev.can_accept_children() {
                prev.depth() + 1
            } else {
                prev.depth()
            }
        };

        let mut desired_depth = desired_depth.min(max_depth);

        if desired_depth > 0
            && gap_index > 0
            && let Some(prev) = self.entries.get(gap_index - 1)
        {
            if desired_depth == prev.depth() + 1 {
                if prev.can_accept_children()
                    && !self
                        .dragged_id
                        .as_ref()
                        .is_some_and(|id| *id == prev.item().id)
                {
                    return Some(DropPreview {
                        gap_index,
                        depth: desired_depth,
                        highlight_ix: Some(gap_index - 1),
                        highlight_target: Some(DndVTreeDropTarget::After),
                        line_y: Some(self.gap_y(gap_index) + scroll_y),
                        line_x: Some(self.indent_offset + self.indent_width * desired_depth),
                    });
                }

                desired_depth = prev.depth();
            }
        }

        let find_next_boundary_ix = |start: usize, depth: usize| -> Option<usize> {
            for ix in start..item_count {
                if self.entries[ix].depth() <= depth {
                    return Some(ix);
                }
            }
            None
        };

        let find_prev_same_depth_ix = |start_inclusive: usize, depth: usize| -> Option<usize> {
            let mut ix = start_inclusive;
            loop {
                if self.entries[ix].depth() == depth {
                    return Some(ix);
                }
                if ix == 0 {
                    return None;
                }
                ix -= 1;
            }
        };

        let subtree_end_ix = |start_ix: usize| self.subtree_end_ix(start_ix);

        let gap_index = if gap_index == 0 {
            0
        } else if gap_index >= item_count {
            let anchor_ix = find_prev_same_depth_ix(item_count - 1, desired_depth)?;
            subtree_end_ix(anchor_ix)
        } else {
            match find_next_boundary_ix(gap_index, desired_depth) {
                Some(boundary_ix) if self.entries[boundary_ix].depth() == desired_depth => {
                    boundary_ix
                }
                _ => {
                    let anchor_ix = find_prev_same_depth_ix(gap_index - 1, desired_depth)?;
                    subtree_end_ix(anchor_ix)
                }
            }
        };

        let line_y = self.gap_y(gap_index) + scroll_y;

        Some(DropPreview {
            gap_index,
            depth: desired_depth,
            highlight_ix: None,
            highlight_target: None,
            line_y: Some(line_y),
            line_x: Some(self.indent_offset + self.indent_width * desired_depth),
        })
    }

    fn apply_drop_preview(
        &mut self,
        drag: &DndVTreeDrag,
        preview: DropPreview,
        cx: &mut Context<Self>,
    ) {
        let Some((dest_parent_id, mut dest_index, expand_parent)) =
            compute_destination_from_gap_depth(
                &self.root_items,
                &self.entries,
                &drag.item_id,
                preview.gap_index,
                preview.depth,
            )
        else {
            self.drop_preview = None;
            self.dragged_id = None;
            self.dragged_ix = None;
            self.drag_start_mouse_position = None;
            cx.notify();
            return;
        };

        let Some(removed) = remove_item_recursive(&mut self.root_items, &drag.item_id, None) else {
            self.drop_preview = None;
            self.dragged_id = None;
            self.dragged_ix = None;
            self.drag_start_mouse_position = None;
            cx.notify();
            return;
        };

        if let Some(parent_id) = dest_parent_id.as_ref()
            && subtree_contains(&removed.item, parent_id)
        {
            insert_item_at(
                &mut self.root_items,
                removed.parent_id.as_ref(),
                removed.index,
                removed.item,
            );
            self.drop_preview = None;
            self.dragged_id = None;
            self.dragged_ix = None;
            self.drag_start_mouse_position = None;
            cx.notify();
            return;
        }

        if dest_parent_id.as_ref() == removed.parent_id.as_ref() && dest_index > removed.index {
            dest_index = dest_index.saturating_sub(1);
        }

        insert_item_at(
            &mut self.root_items,
            dest_parent_id.as_ref(),
            dest_index,
            removed.item,
        );
        if expand_parent {
            if let Some(parent_id) = dest_parent_id.as_ref() {
                set_expanded(&mut self.root_items, parent_id, true);
            }
        }

        self.rebuild_entries();
        self.selected_ix = self
            .entries
            .iter()
            .position(|entry| entry.item().id == drag.item_id);
        self.drop_preview = None;
        self.dragged_id = None;
        self.dragged_ix = None;
        self.drag_start_mouse_position = None;
        cx.notify();
    }

    fn on_drop_on_row(
        &mut self,
        drag: &DndVTreeDrag,
        target_ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = (target_ix, window);
        if drag.tree_id != cx.entity_id() {
            self.drop_preview = None;
            self.dragged_id = None;
            self.dragged_ix = None;
            self.drag_start_mouse_position = None;
            cx.notify();
            return;
        }

        let Some(preview) = self.drop_preview.clone() else {
            return;
        };
        self.apply_drop_preview(drag, preview, cx);
    }

    fn on_drop_after_last(
        &mut self,
        drag: &DndVTreeDrag,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = window;
        if drag.tree_id != cx.entity_id() {
            self.drop_preview = None;
            self.dragged_id = None;
            self.dragged_ix = None;
            self.drag_start_mouse_position = None;
            cx.notify();
            return;
        }

        let Some(preview) = self.drop_preview.clone() else {
            return;
        };

        self.apply_drop_preview(drag, preview, cx);
    }

    // Note: Drop is handled per-row (`on_drop_on_row`) and on the list container
    // (`on_drop_after_last`), and uses the current `drop_preview` to guarantee that the drop
    // result matches the user's visual indicator.
}

impl Render for DndVTreeState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !cx.has_active_drag() {
            self.drop_preview = None;
            self.dragged_id = None;
            self.dragged_ix = None;
            self.drag_start_mouse_position = None;
        }

        let render_item = Rc::clone(&self.render_item);
        let state_entity = cx.entity();
        let dragged_id = self.dragged_id.clone();
        let drop_preview = self.drop_preview.clone();
        let indicator_style = self.indicator_style;
        let drag_handle_width = self.drag_handle_width;
        let entry_sizes = self.entry_sizes.clone();
        let scroll_handle = self.scroll_handle.clone();

        let line = drop_preview
            .as_ref()
            .and_then(|p| p.line_y.zip(p.line_x))
            .map(|(y, x)| {
                let theme = cx.theme();
                let color = indicator_style.color.unwrap_or(theme.foreground);
                let thickness = indicator_style.thickness.max(px(1.));

                let mut line = div()
                    .absolute()
                    .left(x)
                    .right_0()
                    .top(y)
                    .h(thickness)
                    .bg(color);

                match indicator_style.cap {
                    DndVTreeIndicatorCap::None => {}
                    DndVTreeIndicatorCap::StartBar { width, height } => {
                        let cap_width = width.max(px(1.));
                        let cap_height = height.max(px(1.));
                        let offset_y = (thickness - cap_height) / 2.0;
                        let cap_left = px(0.) - cap_width / 2.0;
                        line = line.child(
                            div()
                                .absolute()
                                .left(cap_left)
                                .top(offset_y)
                                .w(cap_width)
                                .h(cap_height)
                                .bg(color),
                        );
                    }
                    DndVTreeIndicatorCap::StartAndEndBars { width, height } => {
                        let cap_width = width.max(px(1.));
                        let cap_height = height.max(px(1.));
                        let offset_y = (thickness - cap_height) / 2.0;
                        let cap_left = px(0.) - cap_width / 2.0;
                        line = line
                            .child(
                                div()
                                    .absolute()
                                    .left(cap_left)
                                    .top(offset_y)
                                    .w(cap_width)
                                    .h(cap_height)
                                    .bg(color),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .right_0()
                                    .top(offset_y)
                                    .w(cap_width)
                                    .h(cap_height)
                                    .bg(color),
                            );
                    }
                }

                line
            });

        div()
            .id("dnd-vtree-state")
            .size_full()
            .relative()
            .child(
                div()
                    .id("dnd-vtree-list")
                    .size_full()
                    .on_drag_move::<DndVTreeDrag>(cx.listener(Self::on_drag_move))
                    .on_drop::<DndVTreeDrag>(cx.listener(Self::on_drop_after_last))
                    .child(
                        v_virtual_list(cx.entity(), "entries", entry_sizes, move |state, visible_range: Range<usize>, window, cx| {
                            let drop_target_bg = cx.theme().drop_target;
                            let mut items = Vec::with_capacity(visible_range.len());
                            for ix in visible_range {
                                let entry = &state.entries[ix];
                                let selected = Some(ix) == state.selected_ix;
                                let dragging =
                                    dragged_id.as_ref().is_some_and(|id| *id == entry.item().id)
                                        && cx.has_active_drag();

                                let drop_target = drop_preview.as_ref().and_then(|preview| {
                                    if preview.highlight_ix != Some(ix) {
                                        return None;
                                    }
                                    preview.highlight_target
                                });

                                let row_state = DndVTreeRowState {
                                    selected,
                                    dragging,
                                    drop_target,
                                };

                                let item = (render_item)(ix, entry, row_state, window, cx);
                                let tree_id = cx.entity_id();
                                let drag_value = DndVTreeDrag {
                                    tree_id,
                                    item_id: entry.item().id.clone(),
                                    label: entry.item().label.clone(),
                                };

                                let is_disabled = entry.item().is_disabled();
                                let row = div()
                                    .id(ix)
                                    .relative()
                                    .size_full()
                                    .flex()
                                    .flex_row()
                                    .when_some(drop_target, |this, _| this.bg(drop_target_bg))
                                    .child(
                                        item.disabled(is_disabled)
                                            .selected(selected)
                                            .h_full()
                                            .flex_1(),
                                    )
                                    .on_drag_move::<DndVTreeDrag>(cx.listener(
                                        move |this, event, window, cx| {
                                            this.on_drag_move_over_row(ix, event, window, cx);
                                        },
                                    ))
                                    .on_drop::<DndVTreeDrag>(cx.listener(
                                        move |this, drag, window, cx| {
                                            this.on_drop_on_row(drag, ix, window, cx);
                                        },
                                    ))
                                    .when(!is_disabled, |this| {
                                        this.on_click(cx.listener(
                                            move |this, click_event, window, cx| {
                                                this.on_entry_click(ix, click_event, window, cx);
                                            },
                                        ))
                                    })
                                    .when(!is_disabled, |this| {
                                        let state_entity = state_entity.clone();
                                        match drag_handle_width {
                                            Some(handle_width) => this.child(
                                                div()
                                                    .id(("dnd-vtree-handle", ix))
                                                    .absolute()
                                                    .top_0()
                                                    .left_0()
                                                    .bottom_0()
                                                    .w(handle_width)
                                                    .cursor(CursorStyle::OpenHand)
                                                    .on_drag(
                                                        drag_value,
                                                        move |drag, _cursor_offset, window, cx: &mut App| {
                                                            state_entity.update(cx, |state, cx| {
                                                                state.on_drag_start(drag, window, cx);
                                                            });
                                                            let label = drag.label.clone();
                                                            cx.new(|_| DragGhost::new(label))
                                                        },
                                                    ),
                                            ),
                                            None => this.on_drag(
                                                drag_value,
                                                move |drag, _cursor_offset, window, cx: &mut App| {
                                                    state_entity.update(cx, |state, cx| {
                                                        state.on_drag_start(drag, window, cx);
                                                    });
                                                    let label = drag.label.clone();
                                                    cx.new(|_| DragGhost::new(label))
                                                },
                                            ),
                                        }
                                    });

                                items.push(row);
                            }
                            items
                        })
                        .track_scroll(&scroll_handle)
                        .flex_grow()
                        .size_full()
                        .with_sizing_behavior(ListSizingBehavior::Auto)
                        .into_any_element(),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .w(px(12.))
                    .child(Scrollbar::uniform_scroll(&self.scrollbar_state, &self.scroll_handle)),
            )
            .when_some(line, |this, line| this.child(line))
    }
}

/// A draggable tree view element that displays hierarchical data (virtual, variable-height).
#[derive(IntoElement)]
pub struct DndVTree {
    id: ElementId,
    state: Entity<DndVTreeState>,
    style: StyleRefinement,
    render_item:
        Rc<dyn Fn(usize, &DndVTreeEntry, DndVTreeRowState, &mut Window, &mut App) -> ListItem>,
}

impl DndVTree {
    pub fn new<R>(state: &Entity<DndVTreeState>, render_item: R) -> Self
    where
        R: Fn(usize, &DndVTreeEntry, DndVTreeRowState, &mut Window, &mut App) -> ListItem + 'static,
    {
        Self {
            id: ElementId::Name(format!("dnd-vtree-{}", state.entity_id()).into()),
            state: state.clone(),
            style: StyleRefinement::default(),
            render_item: Rc::new(move |ix, entry, row_state, window, cx| {
                render_item(ix, entry, row_state, window, cx)
            }),
        }
    }
}

impl Styled for DndVTree {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DndVTree {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus_handle = self.state.read(cx).focus_handle.clone();
        let state_entity = self.state.clone();
        self.state
            .update(cx, |state, _| state.render_item = self.render_item);

        div()
            .id(self.id)
            .key_context(CONTEXT)
            .track_focus(&focus_handle)
            .on_key_down(move |event, window, cx| {
                let handled = state_entity.update(cx, |state, cx| state.on_key_down(event, cx));
                if handled {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            })
            .size_full()
            .child(self.state)
            .refine_style(&self.style)
    }
}

fn sanitize_row_size(mut row_size: Size<Pixels>) -> Size<Pixels> {
    let height: f32 = row_size.height.into();
    if !height.is_finite() || height <= 0.0 {
        row_size.height = px(1.);
    }
    row_size
}

fn subtree_contains(item: &DndVTreeItem, id: &SharedString) -> bool {
    if item.id == *id {
        return true;
    }
    item.children
        .iter()
        .any(|child| subtree_contains(child, id))
}

fn remove_item_recursive(
    items: &mut Vec<DndVTreeItem>,
    target_id: &SharedString,
    parent_id: Option<SharedString>,
) -> Option<RemovedItem> {
    for index in 0..items.len() {
        if items[index].id == *target_id {
            let item = items.remove(index);
            return Some(RemovedItem {
                item,
                parent_id,
                index,
            });
        }
    }

    for index in 0..items.len() {
        let parent_id = items[index].id.clone();
        if let Some(removed) =
            remove_item_recursive(&mut items[index].children, target_id, Some(parent_id))
        {
            return Some(removed);
        }
    }

    None
}

fn insert_item_at(
    root_items: &mut Vec<DndVTreeItem>,
    parent_id: Option<&SharedString>,
    index: usize,
    item: DndVTreeItem,
) {
    match parent_id {
        None => {
            let ix = index.min(root_items.len());
            root_items.insert(ix, item);
        }
        Some(parent_id) => {
            let mut item = Some(item);
            if !insert_into_parent(root_items, parent_id, index, &mut item) {
                if let Some(item) = item.take() {
                    let ix = index.min(root_items.len());
                    root_items.insert(ix, item);
                }
            }
        }
    }
}

fn insert_into_parent(
    items: &mut Vec<DndVTreeItem>,
    parent_id: &SharedString,
    index: usize,
    item: &mut Option<DndVTreeItem>,
) -> bool {
    for node in items.iter_mut() {
        if node.id == *parent_id {
            let Some(item) = item.take() else {
                return true;
            };
            let ix = index.min(node.children.len());
            node.children.insert(ix, item);
            return true;
        }

        if insert_into_parent(&mut node.children, parent_id, index, item) {
            return true;
        }
    }

    false
}

fn set_expanded(items: &mut Vec<DndVTreeItem>, target_id: &SharedString, expanded: bool) -> bool {
    for node in items.iter_mut() {
        if node.id == *target_id {
            node.state.borrow_mut().expanded = expanded;
            return true;
        }
        if set_expanded(&mut node.children, target_id, expanded) {
            return true;
        }
    }
    false
}

fn find_parent_and_index(
    items: &[DndVTreeItem],
    target_id: &SharedString,
    parent_id: Option<&SharedString>,
) -> Option<(Option<SharedString>, usize)> {
    for (index, node) in items.iter().enumerate() {
        if node.id == *target_id {
            return Some((parent_id.cloned(), index));
        }
        if let Some(found) = find_parent_and_index(&node.children, target_id, Some(&node.id)) {
            return Some(found);
        }
    }
    None
}

fn find_node<'a>(items: &'a [DndVTreeItem], target_id: &SharedString) -> Option<&'a DndVTreeItem> {
    for node in items {
        if node.id == *target_id {
            return Some(node);
        }
        if let Some(found) = find_node(&node.children, target_id) {
            return Some(found);
        }
    }
    None
}

fn compute_destination_from_gap_depth(
    root_items: &[DndVTreeItem],
    entries: &[DndVTreeEntry],
    dragged_id: &SharedString,
    gap_index: usize,
    depth: usize,
) -> Option<(Option<SharedString>, usize, bool)> {
    let gap_index = gap_index.min(entries.len());

    if entries.is_empty() {
        return Some((None, 0, false));
    }

    if depth == 0 {
        let next_root_id = entries
            .iter()
            .skip(gap_index)
            .find(|entry| entry.depth() == 0)
            .map(|entry| entry.item().id.clone());

        if let Some(next_root_id) = next_root_id {
            let (parent_id, index) = find_parent_and_index(root_items, &next_root_id, None)?;
            if parent_id.is_some() {
                return None;
            }
            return Some((None, index, false));
        }

        return Some((None, root_items.len(), false));
    }

    if gap_index == 0 {
        return None;
    }

    let mut parent_entry_ix = None;
    for ix in (0..gap_index).rev() {
        if entries[ix].depth() == depth - 1 {
            parent_entry_ix = Some(ix);
            break;
        }
    }
    let parent_entry_ix = parent_entry_ix?;
    let parent_id = entries[parent_entry_ix].item().id.clone();

    if parent_id == *dragged_id {
        return None;
    }

    let parent_node = find_node(root_items, &parent_id)?;
    if !parent_node.can_accept_children() {
        return None;
    }

    // If we're inserting right after the parent row, treat it as "first child" even if the
    // subtree is currently collapsed (so there are no visible children to anchor against).
    if gap_index == parent_entry_ix + 1 {
        return Some((Some(parent_id), 0, true));
    }

    let next_sibling_id = entries
        .iter()
        .skip(gap_index)
        .find(|entry| {
            if entry.depth() != depth {
                return false;
            }
            match entry.parent_id() {
                Some(entry_parent) => entry_parent == &parent_id,
                None => false,
            }
        })
        .map(|entry| entry.item().id.clone());

    if let Some(next_sibling_id) = next_sibling_id {
        let (found_parent_id, index) = find_parent_and_index(root_items, &next_sibling_id, None)?;
        if found_parent_id.as_ref() != Some(&parent_id) {
            return None;
        }
        return Some((Some(parent_id), index, true));
    }

    Some((Some(parent_id), parent_node.children.len(), true))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &'static str, expanded: bool, children: Vec<DndVTreeItem>) -> DndVTreeItem {
        DndVTreeItem::new(id, id)
            .expanded(expanded)
            .children(children)
    }

    fn dump(items: &[DndVTreeItem], depth: usize, out: &mut String) {
        for node in items {
            out.push_str(&"  ".repeat(depth));
            out.push_str(node.id.as_str());
            out.push('\n');
            dump(&node.children, depth + 1, out);
        }
    }

    fn flatten(items: &[DndVTreeItem]) -> Vec<DndVTreeEntry> {
        fn walk(
            items: &[DndVTreeItem],
            depth: usize,
            parent_id: Option<SharedString>,
            out: &mut Vec<DndVTreeEntry>,
        ) {
            for item in items {
                let item_id = item.id.clone();
                out.push(DndVTreeEntry {
                    item: item.clone(),
                    depth,
                    parent_id: parent_id.clone(),
                });
                if item.is_expanded() {
                    walk(&item.children, depth + 1, Some(item_id.clone()), out);
                }
            }
        }

        let mut out = Vec::new();
        walk(items, 0, None, &mut out);
        out
    }

    fn apply_drop(
        root: &mut Vec<DndVTreeItem>,
        dragged_id: SharedString,
        gap_index: usize,
        depth: usize,
    ) {
        let entries = flatten(root);
        let (dest_parent_id, mut dest_index, _expand_parent) =
            compute_destination_from_gap_depth(root, &entries, &dragged_id, gap_index, depth)
                .expect("destination");

        let removed = remove_item_recursive(root, &dragged_id, None).expect("removed");

        if let Some(parent_id) = dest_parent_id.as_ref()
            && subtree_contains(&removed.item, parent_id)
        {
            insert_item_at(
                root,
                removed.parent_id.as_ref(),
                removed.index,
                removed.item,
            );
            return;
        }

        if dest_parent_id.as_ref() == removed.parent_id.as_ref() && dest_index > removed.index {
            dest_index = dest_index.saturating_sub(1);
        }

        insert_item_at(root, dest_parent_id.as_ref(), dest_index, removed.item);
    }

    #[test]
    fn move_before_in_same_parent() {
        let mut root = vec![item(
            "A",
            true,
            vec![item("B", true, vec![]), item("C", true, vec![])],
        )];
        apply_drop(&mut root, "C".into(), 1, 1);

        let mut s = String::new();
        dump(&root, 0, &mut s);
        assert_eq!(
            s.trim(),
            r#"A
  C
  B"#
        );
    }

    #[test]
    fn prevent_cycle_on_drop_into_descendant() {
        let mut root = vec![item(
            "A",
            true,
            vec![item("B", true, vec![item("C", true, vec![])])],
        )];

        let original = {
            let mut s = String::new();
            dump(&root, 0, &mut s);
            s
        };

        // Attempt to drop A as a child of C (gap after C, depth=3). This would create a cycle, so
        // the drop should be rejected and the tree should remain unchanged.
        apply_drop(&mut root, "A".into(), 3, 3);

        let mut s = String::new();
        dump(&root, 0, &mut s);
        assert_eq!(s, original);
    }

    #[test]
    fn move_out_to_root_level() {
        let mut root = vec![
            item(
                "A",
                true,
                vec![item("B", true, vec![item("C", true, vec![])])],
            ),
            item("D", true, vec![]),
        ];
        apply_drop(&mut root, "C".into(), 3, 0);

        let mut s = String::new();
        dump(&root, 0, &mut s);
        assert_eq!(
            s.trim(),
            r#"A
  B
C
D"#
        );
    }

    #[test]
    fn move_item_into_other_node() {
        let mut root = vec![
            item("A", true, vec![item("B", true, vec![])]),
            item("D", true, vec![]),
        ];
        apply_drop(&mut root, "B".into(), 3, 1);

        let mut s = String::new();
        dump(&root, 0, &mut s);
        assert_eq!(
            s.trim(),
            r#"A
D
  B"#
        );
    }

    #[test]
    fn depth_plus_one_inserts_as_first_child() {
        let mut root = vec![
            item(
                "A",
                false,
                vec![item("X", true, vec![]), item("Y", true, vec![])],
            ),
            item("B", true, vec![]),
        ];
        apply_drop(&mut root, "B".into(), 1, 1);

        let mut s = String::new();
        dump(&root, 0, &mut s);
        assert_eq!(
            s.trim(),
            r#"A
  B
  X
  Y"#
        );
    }
}
