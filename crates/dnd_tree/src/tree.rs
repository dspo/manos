use std::{cell::RefCell, ops::Range, rc::Rc};

use gpui::{
    App, AppContext as _, Context, ElementId, Entity, EntityId, FocusHandle,
    InteractiveElement as _, IntoElement, ListSizingBehavior, Modifiers, ParentElement as _,
    Pixels, Point, Render, RenderOnce, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, UniformListScrollHandle, Window, div, prelude::FluentBuilder as _, px,
    uniform_list,
};
use gpui_component::list::ListItem;
use gpui_component::scroll::{Scrollbar, ScrollbarState};
use gpui_component::{ActiveTheme as _, StyledExt as _};

const CONTEXT: &str = "DndTree";

/// Create a [`DndTree`].
pub fn dnd_tree<R>(state: &Entity<DndTreeState>, render_item: R) -> DndTree
where
    R: Fn(usize, &DndTreeEntry, DndTreeRowState, &mut Window, &mut App) -> ListItem + 'static,
{
    DndTree::new(state, render_item)
}

#[derive(Clone)]
struct DndTreeDrag {
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
struct DndTreeItemState {
    expanded: bool,
    disabled: bool,
    can_accept_children: bool,
}

/// A tree item with children, and an expanded state.
#[derive(Clone)]
pub struct DndTreeItem {
    pub id: SharedString,
    pub label: SharedString,
    pub children: Vec<DndTreeItem>,
    state: Rc<RefCell<DndTreeItemState>>,
}

impl DndTreeItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children: Vec::new(),
            state: Rc::new(RefCell::new(DndTreeItemState {
                expanded: false,
                disabled: false,
                can_accept_children: true,
            })),
        }
    }

    pub fn child(mut self, child: DndTreeItem) -> Self {
        self.children.push(child);
        self
    }

    pub fn children(mut self, children: impl Into<Vec<DndTreeItem>>) -> Self {
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
    /// Defaults to `true`.
    pub fn accept_children(self, accept_children: bool) -> Self {
        self.state.borrow_mut().can_accept_children = accept_children;
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
}

/// A flat representation of a tree item with its depth.
#[derive(Clone)]
pub struct DndTreeEntry {
    item: DndTreeItem,
    depth: usize,
    parent_id: Option<SharedString>,
}

impl DndTreeEntry {
    #[inline]
    pub fn item(&self) -> &DndTreeItem {
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DndTreeDropTarget {
    Before,
    After,
    Inside,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DndTreeRowState {
    pub selected: bool,
    pub dragging: bool,
    pub drop_target: Option<DndTreeDropTarget>,
}

#[derive(Clone, Debug, PartialEq)]
enum DropPreviewTarget {
    Before { target_id: SharedString },
    After { target_id: SharedString },
    Inside { target_id: SharedString },
    Root { index: usize },
}

#[derive(Clone, Debug, PartialEq)]
struct DropPreview {
    target: DropPreviewTarget,
    target_ix: Option<usize>,
    line_y: Option<Pixels>,
    line_x: Option<Pixels>,
}

#[derive(Clone)]
struct RemovedItem {
    item: DndTreeItem,
    parent_id: Option<SharedString>,
    index: usize,
}

/// State for managing DnD tree items.
pub struct DndTreeState {
    focus_handle: FocusHandle,
    root_items: Vec<DndTreeItem>,
    entries: Vec<DndTreeEntry>,
    indent_width: Pixels,
    indent_offset: Pixels,
    scrollbar_state: ScrollbarState,
    scroll_handle: UniformListScrollHandle,
    selected_ix: Option<usize>,
    dragged_id: Option<SharedString>,
    dragged_ix: Option<usize>,
    drop_preview: Option<DropPreview>,
    drag_origin_x: Option<Pixels>,
    render_item:
        Rc<dyn Fn(usize, &DndTreeEntry, DndTreeRowState, &mut Window, &mut App) -> ListItem>,
}

impl DndTreeState {
    pub fn new(cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            root_items: Vec::new(),
            entries: Vec::new(),
            indent_width: px(16.),
            indent_offset: px(0.),
            scrollbar_state: ScrollbarState::default(),
            scroll_handle: UniformListScrollHandle::default(),
            selected_ix: None,
            dragged_id: None,
            dragged_ix: None,
            drop_preview: None,
            drag_origin_x: None,
            render_item: Rc::new(|_, _, _, _, _| ListItem::new("dnd-tree-empty")),
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

    pub fn items(mut self, items: impl Into<Vec<DndTreeItem>>) -> Self {
        self.root_items = items.into();
        self.rebuild_entries();
        self
    }

    pub fn set_items(&mut self, items: impl Into<Vec<DndTreeItem>>, cx: &mut Context<Self>) {
        self.root_items = items.into();
        self.selected_ix = None;
        self.drop_preview = None;
        self.dragged_id = None;
        self.dragged_ix = None;
        self.drag_origin_x = None;
        self.rebuild_entries();
        cx.notify();
    }

    pub fn root_items(&self) -> &[DndTreeItem] {
        &self.root_items
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_ix
    }

    pub fn set_selected_index(&mut self, ix: Option<usize>, cx: &mut Context<Self>) {
        self.selected_ix = ix;
        cx.notify();
    }

    pub fn selected_entry(&self) -> Option<&DndTreeEntry> {
        self.selected_ix.and_then(|ix| self.entries.get(ix))
    }

    fn rebuild_entries(&mut self) {
        self.entries.clear();
        for index in 0..self.root_items.len() {
            let item = self.root_items[index].clone();
            self.add_entry(item, 0, None);
        }
    }

    fn add_entry(&mut self, item: DndTreeItem, depth: usize, parent_id: Option<SharedString>) {
        let item_id = item.id.clone();
        self.entries.push(DndTreeEntry {
            item: item.clone(),
            depth,
            parent_id: parent_id.clone(),
        });

        if item.is_expanded() {
            for child in item.children.iter().cloned() {
                self.add_entry(child, depth + 1, Some(item_id.clone()));
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

    fn on_drag_start(
        &mut self,
        drag: &DndTreeDrag,
        cursor_offset: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dragged_id = Some(drag.item_id.clone());
        self.dragged_ix = self
            .entries
            .iter()
            .position(|entry| entry.item().id == drag.item_id);
        self.drag_origin_x = Some(window.mouse_position().x - cursor_offset.x);
        self.drop_preview = None;
        self.selected_ix = self
            .entries
            .iter()
            .position(|entry| entry.item().id == drag.item_id);
        cx.notify();
    }

    fn on_drag_move(
        &mut self,
        event: &gpui::DragMoveEvent<DndTreeDrag>,
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

        if self.entries.is_empty() {
            let origin_x = self.drag_origin_x.unwrap_or(list_bounds.origin.x);
            let desired_depth = self.desired_depth(mouse_position.x - origin_x);
            let new_preview = Some(DropPreview {
                target: DropPreviewTarget::Root { index: 0 },
                target_ix: None,
                line_y: Some(px(0.)),
                line_x: Some(self.indent_offset + self.indent_width * desired_depth),
            });
            if self.drop_preview != new_preview {
                self.drop_preview = new_preview;
                cx.notify();
            }
        }
    }

    fn desired_depth(&self, x_in_list: Pixels) -> usize {
        if x_in_list <= self.indent_offset {
            return 0;
        }
        ((x_in_list - self.indent_offset) / self.indent_width)
            .floor()
            .max(0.0) as usize
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

    fn compute_drop_preview_for_gap(
        &self,
        gap_index: usize,
        desired_depth: usize,
    ) -> Option<DropPreview> {
        let item_count = self.entries.len();
        if item_count == 0 {
            return Some(DropPreview {
                target: DropPreviewTarget::Root { index: 0 },
                target_ix: None,
                line_y: Some(px(0.)),
                line_x: Some(self.indent_offset),
            });
        }

        let scroll_y = self.scroll_handle.0.borrow().base_handle.offset().y;
        let item_height = self
            .scroll_handle
            .0
            .borrow()
            .last_item_size
            .map(|s| s.item.height)
            .unwrap_or(px(28.));

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
                        target: DropPreviewTarget::Inside {
                            target_id: prev.item().id.clone(),
                        },
                        target_ix: Some(gap_index - 1),
                        line_y: None,
                        line_x: None,
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

        let (target, target_ix, line_y) = if gap_index == 0 {
            let first = self.entries.first()?;
            (
                DropPreviewTarget::Before {
                    target_id: first.item().id.clone(),
                },
                0,
                scroll_y,
            )
        } else if gap_index >= item_count {
            let anchor_ix = find_prev_same_depth_ix(item_count - 1, desired_depth)?;
            (
                DropPreviewTarget::After {
                    target_id: self.entries[anchor_ix].item().id.clone(),
                },
                anchor_ix,
                item_height * subtree_end_ix(anchor_ix) + scroll_y,
            )
        } else {
            match find_next_boundary_ix(gap_index, desired_depth) {
                Some(boundary_ix) if self.entries[boundary_ix].depth() == desired_depth => (
                    DropPreviewTarget::Before {
                        target_id: self.entries[boundary_ix].item().id.clone(),
                    },
                    boundary_ix,
                    item_height * boundary_ix + scroll_y,
                ),
                _ => {
                    let anchor_ix = find_prev_same_depth_ix(gap_index - 1, desired_depth)?;
                    (
                        DropPreviewTarget::After {
                            target_id: self.entries[anchor_ix].item().id.clone(),
                        },
                        anchor_ix,
                        item_height * subtree_end_ix(anchor_ix) + scroll_y,
                    )
                }
            }
        };

        Some(DropPreview {
            target,
            target_ix: Some(target_ix),
            line_y: Some(line_y),
            line_x: Some(self.indent_offset + self.indent_width * desired_depth),
        })
    }

    fn compute_drop_preview_for_row_hover(
        &self,
        hovered_ix: usize,
        x_in_list: Pixels,
        modifiers: Modifiers,
    ) -> Option<DropPreview> {
        let item_count = self.entries.len();
        if item_count == 0 {
            return Some(DropPreview {
                target: DropPreviewTarget::Root { index: 0 },
                target_ix: None,
                line_y: Some(px(0.)),
                line_x: Some(self.indent_offset),
            });
        }

        let dragged_ix = self.dragged_ix?;
        let dragged_id = self.dragged_id.as_ref()?;

        if hovered_ix < item_count {
            let subtree_end_ix = self.subtree_end_ix(dragged_ix);
            if hovered_ix >= dragged_ix && hovered_ix < subtree_end_ix {
                return None;
            }

            let entry = self.entries.get(hovered_ix)?;
            let desired_depth = self.desired_depth(x_in_list);

            if (modifiers.alt || desired_depth > entry.depth())
                && entry.can_accept_children()
                && *dragged_id != entry.item().id
            {
                return Some(DropPreview {
                    target: DropPreviewTarget::Inside {
                        target_id: entry.item().id.clone(),
                    },
                    target_ix: Some(hovered_ix),
                    line_y: None,
                    line_x: None,
                });
            }

            if hovered_ix == dragged_ix {
                return None;
            }

            let gap_index = if hovered_ix < dragged_ix {
                hovered_ix
            } else {
                hovered_ix.saturating_add(1)
            };
            return self.compute_drop_preview_for_gap(gap_index, desired_depth);
        }

        let desired_depth = self.desired_depth(x_in_list);
        self.compute_drop_preview_for_gap(item_count, desired_depth)
    }

    fn on_row_drag_move(
        &mut self,
        row_ix: usize,
        event: &gpui::DragMoveEvent<DndTreeDrag>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !cx.has_active_drag() {
            return;
        }

        let drag = event.drag(cx);
        if drag.tree_id != cx.entity_id() {
            return;
        }

        let mouse_position = event.event.position;
        if !event.bounds.contains(&mouse_position) {
            return;
        }

        let x_in_list = mouse_position.x - event.bounds.origin.x;
        let new_preview =
            self.compute_drop_preview_for_row_hover(row_ix, x_in_list, event.event.modifiers);
        if self.drop_preview != new_preview {
            self.drop_preview = new_preview;
            cx.notify();
        }
    }

    fn apply_drop_target(
        &mut self,
        drag: &DndTreeDrag,
        target: DropPreviewTarget,
        cx: &mut Context<Self>,
    ) {
        let Some(removed) = remove_item_recursive(&mut self.root_items, &drag.item_id, None) else {
            self.drop_preview = None;
            self.dragged_id = None;
            self.dragged_ix = None;
            self.drag_origin_x = None;
            cx.notify();
            return;
        };

        let Some((dest_parent_id, dest_index, expand_parent)) =
            compute_destination(&self.root_items, &removed.item, &target)
        else {
            insert_item_at(
                &mut self.root_items,
                removed.parent_id.as_ref(),
                removed.index,
                removed.item,
            );
            self.drop_preview = None;
            self.dragged_id = None;
            self.dragged_ix = None;
            self.drag_origin_x = None;
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
            self.drag_origin_x = None;
            cx.notify();
            return;
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
        self.drag_origin_x = None;
        cx.notify();
    }

    fn on_drop_on_row(
        &mut self,
        drag: &DndTreeDrag,
        target_ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if drag.tree_id != cx.entity_id() {
            self.drop_preview = None;
            self.dragged_id = None;
            self.dragged_ix = None;
            self.drag_origin_x = None;
            cx.notify();
            return;
        }

        let origin_x = self.drag_origin_x.unwrap_or(px(0.));
        let x_in_list = window.mouse_position().x - origin_x;

        let Some(preview) =
            self.compute_drop_preview_for_row_hover(target_ix, x_in_list, window.modifiers())
        else {
            return;
        };

        self.apply_drop_target(drag, preview.target, cx);
    }

    fn on_drop_after_last(
        &mut self,
        drag: &DndTreeDrag,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if drag.tree_id != cx.entity_id() {
            self.drop_preview = None;
            self.dragged_id = None;
            self.dragged_ix = None;
            self.drag_origin_x = None;
            cx.notify();
            return;
        }

        let origin_x = self.drag_origin_x.unwrap_or(px(0.));
        let x_in_list = window.mouse_position().x - origin_x;
        let Some(preview) = self.compute_drop_preview_for_row_hover(
            self.entries.len(),
            x_in_list,
            window.modifiers(),
        ) else {
            return;
        };

        self.apply_drop_target(drag, preview.target, cx);
    }

    // Note: actual drop handling is wired per-row (`on_drop_on_row`) and on the list container
    // (`on_drop_after_last`). `drop_preview` is used only for rendering feedback.
}

impl Render for DndTreeState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !cx.has_active_drag() {
            self.drop_preview = None;
            self.dragged_id = None;
            self.dragged_ix = None;
            self.drag_origin_x = None;
        }

        let render_item = Rc::clone(&self.render_item);
        let state_entity = cx.entity();
        let dragged_id = self.dragged_id.clone();
        let drop_preview = self.drop_preview.clone();

        let line = drop_preview
            .as_ref()
            .and_then(|p| p.line_y.zip(p.line_x))
            .map(|(y, x)| {
                let theme = cx.theme();
                div()
                    .absolute()
                    .left(x)
                    .right_0()
                    .top(y)
                    .h(px(2.))
                    .bg(theme.drag_border)
            });

        div()
            .id("dnd-tree-state")
            .size_full()
            .relative()
            .child(
                uniform_list("entries", self.entries.len(), {
                    cx.processor(move |state, visible_range: Range<usize>, window, cx| {
                        let drop_target_bg = cx.theme().drop_target;
                        let mut items = Vec::with_capacity(visible_range.len());
                        for ix in visible_range {
                            let entry = &state.entries[ix];
                            let selected = Some(ix) == state.selected_ix;
                            let dragging =
                                dragged_id.as_ref().is_some_and(|id| *id == entry.item().id)
                                    && cx.has_active_drag();

                            let drop_target = drop_preview.as_ref().and_then(|preview| {
                                if preview.target_ix != Some(ix) {
                                    return None;
                                }
                                match preview.target {
                                    DropPreviewTarget::Before { .. } => {
                                        Some(DndTreeDropTarget::Before)
                                    }
                                    DropPreviewTarget::After { .. } => {
                                        Some(DndTreeDropTarget::After)
                                    }
                                    DropPreviewTarget::Inside { .. } => {
                                        Some(DndTreeDropTarget::Inside)
                                    }
                                    DropPreviewTarget::Root { .. } => None,
                                }
                            });

                            let row_state = DndTreeRowState {
                                selected,
                                dragging,
                                drop_target,
                            };

                            let item = (render_item)(ix, entry, row_state, window, cx);
                            let tree_id = cx.entity_id();
                            let drag_value = DndTreeDrag {
                                tree_id,
                                item_id: entry.item().id.clone(),
                                label: entry.item().label.clone(),
                            };

                            let is_disabled = entry.item().is_disabled();
                            let row = div()
                                .id(ix)
                                .when(
                                    matches!(
                                        drop_target,
                                        Some(DndTreeDropTarget::Before | DndTreeDropTarget::After)
                                    ),
                                    |this| this.bg(drop_target_bg),
                                )
                                .child(item.disabled(is_disabled).selected(selected))
                                .on_drag_move::<DndTreeDrag>(cx.listener(
                                    move |this, ev, window, cx| {
                                        this.on_row_drag_move(ix, ev, window, cx);
                                    },
                                ))
                                .on_drop::<DndTreeDrag>(cx.listener(
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
                                    this.on_drag(
                                        drag_value,
                                        move |drag, cursor_offset, window, cx| {
                                            state_entity.update(cx, |state, cx| {
                                                state.on_drag_start(
                                                    drag,
                                                    cursor_offset,
                                                    window,
                                                    cx,
                                                );
                                            });
                                            let label = drag.label.clone();
                                            cx.new(|_| DragGhost::new(label))
                                        },
                                    )
                                });

                            items.push(row);
                        }
                        items
                    })
                })
                .on_drag_move::<DndTreeDrag>(cx.listener(Self::on_drag_move))
                .on_drop::<DndTreeDrag>(cx.listener(Self::on_drop_after_last))
                .flex_grow()
                .size_full()
                .track_scroll(self.scroll_handle.clone())
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .into_any_element(),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .w(px(12.))
                    .child(Scrollbar::uniform_scroll(
                        &self.scrollbar_state,
                        &self.scroll_handle,
                    )),
            )
            .when_some(line, |this, line| this.child(line))
    }
}

/// A draggable tree view element that displays hierarchical data.
#[derive(IntoElement)]
pub struct DndTree {
    id: ElementId,
    state: Entity<DndTreeState>,
    style: StyleRefinement,
    render_item:
        Rc<dyn Fn(usize, &DndTreeEntry, DndTreeRowState, &mut Window, &mut App) -> ListItem>,
}

impl DndTree {
    pub fn new<R>(state: &Entity<DndTreeState>, render_item: R) -> Self
    where
        R: Fn(usize, &DndTreeEntry, DndTreeRowState, &mut Window, &mut App) -> ListItem + 'static,
    {
        Self {
            id: ElementId::Name(format!("dnd-tree-{}", state.entity_id()).into()),
            state: state.clone(),
            style: StyleRefinement::default(),
            render_item: Rc::new(move |ix, entry, row_state, window, cx| {
                render_item(ix, entry, row_state, window, cx)
            }),
        }
    }
}

impl Styled for DndTree {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DndTree {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus_handle = self.state.read(cx).focus_handle.clone();
        self.state
            .update(cx, |state, _| state.render_item = self.render_item);

        div()
            .id(self.id)
            .key_context(CONTEXT)
            .track_focus(&focus_handle)
            .size_full()
            .child(self.state)
            .refine_style(&self.style)
    }
}

fn subtree_contains(item: &DndTreeItem, id: &SharedString) -> bool {
    if item.id == *id {
        return true;
    }
    item.children
        .iter()
        .any(|child| subtree_contains(child, id))
}

fn remove_item_recursive(
    items: &mut Vec<DndTreeItem>,
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
    root_items: &mut Vec<DndTreeItem>,
    parent_id: Option<&SharedString>,
    index: usize,
    item: DndTreeItem,
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
    items: &mut Vec<DndTreeItem>,
    parent_id: &SharedString,
    index: usize,
    item: &mut Option<DndTreeItem>,
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

fn set_expanded(items: &mut Vec<DndTreeItem>, target_id: &SharedString, expanded: bool) -> bool {
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
    items: &[DndTreeItem],
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

fn find_node<'a>(items: &'a [DndTreeItem], target_id: &SharedString) -> Option<&'a DndTreeItem> {
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

fn compute_destination(
    root_items: &[DndTreeItem],
    dragged_item: &DndTreeItem,
    preview: &DropPreviewTarget,
) -> Option<(Option<SharedString>, usize, bool)> {
    match preview {
        DropPreviewTarget::Root { index } => Some((None, *index, false)),
        DropPreviewTarget::Before { target_id } => {
            if *target_id == dragged_item.id {
                return None;
            }
            let (parent_id, target_ix) = find_parent_and_index(root_items, target_id, None)?;
            Some((parent_id, target_ix, false))
        }
        DropPreviewTarget::After { target_id } => {
            if *target_id == dragged_item.id {
                return None;
            }
            let (parent_id, target_ix) = find_parent_and_index(root_items, target_id, None)?;
            Some((parent_id, target_ix + 1, false))
        }
        DropPreviewTarget::Inside { target_id } => {
            if *target_id == dragged_item.id {
                return None;
            }
            let target = find_node(root_items, target_id)?;
            if !target.can_accept_children() {
                return None;
            }
            Some((Some(target_id.clone()), target.children.len(), true))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &'static str, children: Vec<DndTreeItem>) -> DndTreeItem {
        let mut node = DndTreeItem::new(id, id).expanded(true);
        node.children = children;
        node
    }

    fn dump(items: &[DndTreeItem], depth: usize, out: &mut String) {
        for node in items {
            out.push_str(&"  ".repeat(depth));
            out.push_str(node.id.as_str());
            out.push('\n');
            dump(&node.children, depth + 1, out);
        }
    }

    #[test]
    fn move_before_in_same_parent() {
        let mut root = vec![item("A", vec![item("B", vec![]), item("C", vec![])])];
        let removed = remove_item_recursive(&mut root, &"C".into(), None).unwrap();
        let preview = DropPreviewTarget::Before {
            target_id: "B".into(),
        };
        let (parent_id, index, _) = compute_destination(&root, &removed.item, &preview).unwrap();
        insert_item_at(&mut root, parent_id.as_ref(), index, removed.item);

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
        let mut root = vec![item("A", vec![item("B", vec![item("C", vec![])])])];
        let removed = remove_item_recursive(&mut root, &"A".into(), None).unwrap();

        let preview = DropPreviewTarget::Inside {
            target_id: "C".into(),
        };
        let dest = compute_destination(&root, &removed.item, &preview);
        assert!(dest.is_none(), "target no longer exists after removal");
    }

    #[test]
    fn move_out_to_root_level() {
        let mut root = vec![
            item("A", vec![item("B", vec![item("C", vec![])])]),
            item("D", vec![]),
        ];

        let removed = remove_item_recursive(&mut root, &"C".into(), None).unwrap();
        let preview = DropPreviewTarget::After {
            target_id: "A".into(),
        };
        let (parent_id, index, _) = compute_destination(&root, &removed.item, &preview).unwrap();
        insert_item_at(&mut root, parent_id.as_ref(), index, removed.item);

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
        let mut root = vec![item("A", vec![item("B", vec![])]), item("D", vec![])];
        let removed = remove_item_recursive(&mut root, &"B".into(), None).unwrap();

        let preview = DropPreviewTarget::Inside {
            target_id: "D".into(),
        };
        let (parent_id, index, _) = compute_destination(&root, &removed.item, &preview).unwrap();
        insert_item_at(&mut root, parent_id.as_ref(), index, removed.item);

        let mut s = String::new();
        dump(&root, 0, &mut s);
        assert_eq!(
            s.trim(),
            r#"A
D
  B"#
        );
    }
}
