use std::{ops::Range, rc::Rc};

use gpui::{
    App, AppContext as _, Context, CursorStyle, ElementId, Entity, EntityId, FocusHandle,
    InteractiveElement as _, IntoElement, ListSizingBehavior, Modifiers, ParentElement as _,
    Pixels, Render, RenderOnce, SharedString, Size, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px, size,
};
use gpui_component::list::ListItem;
use gpui_component::scroll::{Scrollbar, ScrollbarState};
use gpui_component::{ActiveTheme as _, StyledExt as _, VirtualListScrollHandle, v_virtual_list};

const CONTEXT: &str = "DndVList";
const DEFAULT_ROW_HEIGHT: Pixels = px(28.);

/// Create a [`DndVList`].
pub fn dnd_vlist<T, R>(state: &Entity<DndVListState<T>>, render_item: R) -> DndVList<T>
where
    T: 'static,
    R: Fn(usize, &DndVListItem<T>, DndVListRowState, &mut Window, &mut App) -> ListItem + 'static,
{
    DndVList::new(state, render_item)
}

/// A single item in a [`DndVListState`].
#[derive(Clone)]
pub struct DndVListItem<T> {
    pub id: SharedString,
    pub label: SharedString,
    pub data: T,
    size: Size<Pixels>,
    disabled: bool,
}

impl<T> DndVListItem<T> {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, data: T) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            data,
            size: size(px(0.), DEFAULT_ROW_HEIGHT),
            disabled: false,
        }
    }

    /// Set this item's size used by [`gpui_component::v_virtual_list`].
    ///
    /// For vertical lists, only `height` is used.
    pub fn size(mut self, size: Size<Pixels>) -> Self {
        self.size = size;
        self
    }

    /// Set this item's height (vertical axis size).
    pub fn height(mut self, height: Pixels) -> Self {
        self.size.height = height;
        self
    }

    pub fn item_size(&self) -> Size<Pixels> {
        self.size
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }
}

#[derive(Clone)]
struct DndVListDrag {
    list_id: EntityId,
    item_id: SharedString,
    label: SharedString,
    ix: usize,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DndVListDropTarget {
    Before,
    After,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DndVListRowState {
    pub selected: bool,
    pub dragging: bool,
    pub drop_target: Option<DndVListDropTarget>,
}

#[derive(Clone, Debug)]
pub struct DndVListReorder {
    pub item_id: SharedString,
    pub from: usize,
    pub to: usize,
}

struct DndVListStateCallbacks<T> {
    can_drop: Option<Rc<dyn Fn(&DndVListReorder, &[DndVListItem<T>], Modifiers) -> bool>>,
    on_reorder: Option<Rc<dyn Fn(&DndVListReorder, &[DndVListItem<T>])>>,
}

impl<T> Default for DndVListStateCallbacks<T> {
    fn default() -> Self {
        Self {
            can_drop: None,
            on_reorder: None,
        }
    }
}

/// State for a draggable, reorderable list based on `gpui-component`'s virtual list.
pub struct DndVListState<T> {
    focus_handle: FocusHandle,
    items: Vec<DndVListItem<T>>,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    scrollbar_state: ScrollbarState,
    scroll_handle: VirtualListScrollHandle,
    drag_handle_width: Option<Pixels>,
    selected_ix: Option<usize>,
    dragged_id: Option<SharedString>,
    dragged_ix: Option<usize>,
    callbacks: DndVListStateCallbacks<T>,
    render_item:
        Rc<dyn Fn(usize, &DndVListItem<T>, DndVListRowState, &mut Window, &mut App) -> ListItem>,
}

impl<T: 'static> DndVListState<T> {
    pub fn new(cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            items: Vec::new(),
            item_sizes: Rc::new(Vec::new()),
            scrollbar_state: ScrollbarState::default(),
            scroll_handle: VirtualListScrollHandle::new(),
            drag_handle_width: Some(px(32.)),
            selected_ix: None,
            dragged_id: None,
            dragged_ix: None,
            callbacks: DndVListStateCallbacks::default(),
            render_item: Rc::new(|_, _, _, _, _| ListItem::new("dnd-vlist-empty")),
        }
    }

    pub fn items(mut self, items: impl Into<Vec<DndVListItem<T>>>) -> Self {
        self.items = items.into();
        self.rebuild_item_sizes();
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

    pub fn set_items(&mut self, items: impl Into<Vec<DndVListItem<T>>>, cx: &mut Context<Self>) {
        self.items = items.into();
        self.rebuild_item_sizes();
        self.selected_ix = None;
        self.dragged_id = None;
        self.dragged_ix = None;
        cx.notify();
    }

    pub fn items_ref(&self) -> &[DndVListItem<T>] {
        &self.items
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_ix
    }

    pub fn set_selected_index(&mut self, ix: Option<usize>, cx: &mut Context<Self>) {
        self.selected_ix = ix;
        cx.notify();
    }

    /// Provide a predicate to control whether a reorder is allowed.
    ///
    /// The predicate receives:
    /// - `reorder`: the proposed reorder (`from` is the dragged index in the pre-drop list,
    ///   `to` is the destination index in the post-drop list)
    /// - `items`: current items slice
    /// - `modifiers`: current keyboard modifiers
    pub fn can_drop(
        mut self,
        can_drop: impl Fn(&DndVListReorder, &[DndVListItem<T>], Modifiers) -> bool + 'static,
    ) -> Self {
        self.callbacks.can_drop = Some(Rc::new(can_drop));
        self
    }

    /// Provide a callback invoked after a successful reorder.
    pub fn on_reorder(
        mut self,
        on_reorder: impl Fn(&DndVListReorder, &[DndVListItem<T>]) + 'static,
    ) -> Self {
        self.callbacks.on_reorder = Some(Rc::new(on_reorder));
        self
    }

    /// Update the size of a single row.
    ///
    /// For vertical lists, only `height` is used.
    pub fn set_item_size(&mut self, ix: usize, size: Size<Pixels>, cx: &mut Context<Self>) {
        if let Some(item) = self.items.get_mut(ix) {
            item.size = size;
            self.rebuild_item_sizes();
            cx.notify();
        }
    }

    /// Update the height of a single row.
    pub fn set_item_height(&mut self, ix: usize, height: Pixels, cx: &mut Context<Self>) {
        if let Some(item) = self.items.get_mut(ix) {
            item.size.height = height;
            self.rebuild_item_sizes();
            cx.notify();
        }
    }

    fn rebuild_item_sizes(&mut self) {
        self.item_sizes = Rc::new(
            self.items
                .iter()
                .map(|item| sanitize_item_size(item.item_size()))
                .collect(),
        );
    }

    fn on_entry_click(
        &mut self,
        ix: usize,
        _event: &gpui::ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_ix = Some(ix);
        cx.notify();
    }

    fn on_drag_start(&mut self, drag: &DndVListDrag, _window: &mut Window, cx: &mut Context<Self>) {
        self.dragged_id = Some(drag.item_id.clone());
        self.dragged_ix = self
            .items
            .iter()
            .position(|item| item.id == drag.item_id)
            .or(Some(drag.ix));
        self.selected_ix = self.dragged_ix;
        cx.notify();
    }

    fn drop_is_allowed(&mut self, reorder: &DndVListReorder, modifiers: Modifiers) -> bool {
        self.callbacks
            .can_drop
            .as_ref()
            .map(|f| f(reorder, &self.items, modifiers))
            .unwrap_or(true)
    }

    fn on_drop_on_row(
        &mut self,
        drag: &DndVListDrag,
        target_ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if drag.list_id != cx.entity_id() {
            return;
        }

        let Some(from_ix) = self.items.iter().position(|item| item.id == drag.item_id) else {
            return;
        };

        let item_count = self.items.len();
        if item_count == 0 {
            return;
        }

        let gap_index = if target_ix < from_ix {
            target_ix
        } else if target_ix > from_ix {
            target_ix.saturating_add(1)
        } else {
            from_ix
        };

        let mut to_ix = gap_index;
        if to_ix > from_ix {
            to_ix = to_ix.saturating_sub(1);
        }
        to_ix = to_ix.min(item_count.saturating_sub(1));

        let reorder = DndVListReorder {
            item_id: drag.item_id.clone(),
            from: from_ix,
            to: to_ix,
        };

        if from_ix == to_ix {
            return;
        }

        let allowed = self.drop_is_allowed(&reorder, window.modifiers());
        if !allowed {
            return;
        }

        if from_ix != to_ix {
            let item = self.items.remove(from_ix);
            self.items.insert(to_ix, item);
            self.rebuild_item_sizes();
        }

        self.selected_ix = Some(to_ix);
        self.dragged_ix = Some(to_ix);
        cx.notify();

        if let Some(on_reorder) = self.callbacks.on_reorder.as_ref() {
            on_reorder(&reorder, &self.items);
        }
    }

    fn on_drop_after_last(
        &mut self,
        drag: &DndVListDrag,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if drag.list_id != cx.entity_id() {
            return;
        }

        let Some(from_ix) = self.items.iter().position(|item| item.id == drag.item_id) else {
            return;
        };

        let item_count = self.items.len();
        if item_count == 0 {
            return;
        }

        let gap_index = item_count;
        let mut to_ix = gap_index;
        if to_ix > from_ix {
            to_ix = to_ix.saturating_sub(1);
        }
        to_ix = to_ix.min(item_count.saturating_sub(1));

        let reorder = DndVListReorder {
            item_id: drag.item_id.clone(),
            from: from_ix,
            to: to_ix,
        };

        if from_ix == to_ix {
            return;
        }

        let allowed = self.drop_is_allowed(&reorder, window.modifiers());
        if !allowed {
            return;
        }

        let item = self.items.remove(from_ix);
        self.items.insert(to_ix, item);
        self.rebuild_item_sizes();

        self.selected_ix = Some(to_ix);
        self.dragged_ix = Some(to_ix);
        cx.notify();

        if let Some(on_reorder) = self.callbacks.on_reorder.as_ref() {
            on_reorder(&reorder, &self.items);
        }
    }
}

impl<T: 'static> Render for DndVListState<T> {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !cx.has_active_drag() {
            self.dragged_id = None;
            self.dragged_ix = None;
        }

        let render_item = Rc::clone(&self.render_item);
        let state_entity = cx.entity();
        let dragged_id = self.dragged_id.clone();
        let drag_handle_width = self.drag_handle_width;
        let list_id = cx.entity_id();
        let item_sizes = self.item_sizes.clone();
        let scroll_handle = self.scroll_handle.clone();

        div()
            .id("dnd-vlist-state")
            .size_full()
            .relative()
            .child(
                div()
                    .id("dnd-vlist-list")
                    .size_full()
                    .on_drop::<DndVListDrag>(cx.listener(Self::on_drop_after_last))
                    .child(
                        v_virtual_list(
                            cx.entity(),
                            "items",
                            item_sizes,
                            move |state, visible_range: Range<usize>, window, cx| {
                                let drop_target_bg = cx.theme().drop_target;
                                let drag_border = cx.theme().drag_border;
                                let mut rows = Vec::with_capacity(visible_range.len());
                                for ix in visible_range {
                                    let item = &state.items[ix];

                                    let selected = Some(ix) == state.selected_ix;
                                    let dragging =
                                        dragged_id.as_ref().is_some_and(|id| *id == item.id)
                                            && cx.has_active_drag();

                                    let row_state = DndVListRowState {
                                        selected,
                                        dragging,
                                        drop_target: None,
                                    };

                                    let list_item = (render_item)(ix, item, row_state, window, cx);
                                    let drag_value = DndVListDrag {
                                        list_id,
                                        item_id: item.id.clone(),
                                        label: item.label.clone(),
                                        ix,
                                    };

                                    let is_disabled = item.is_disabled();
                                    let row = div()
                                        .id(ix)
                                        .relative()
                                        .size_full()
                                        .flex()
                                        .flex_row()
                                        .child(
                                            list_item
                                                .disabled(is_disabled)
                                                .selected(selected)
                                                .h_full()
                                                .flex_1(),
                                        )
                                        .drag_over::<DndVListDrag>(
                                            move |style, drag, _window, _cx| {
                                                if drag.list_id != list_id {
                                                    return style;
                                                }

                                                let mut style = style
                                                    .bg(drop_target_bg
                                                        .alpha(drop_target_bg.a.max(0.2)));
                                                style = style.border_color(drag_border);
                                                if ix < drag.ix {
                                                    style = style.border_t_2();
                                                } else if ix > drag.ix {
                                                    style = style.border_b_2();
                                                }
                                                style
                                            },
                                        )
                                        .on_drop::<DndVListDrag>(cx.listener(
                                            move |this, drag, window, cx| {
                                                this.on_drop_on_row(drag, ix, window, cx);
                                            },
                                        ))
                                        .when(!is_disabled, |this| {
                                            this.on_click(cx.listener(
                                                move |this, click_event, window, cx| {
                                                    this.on_entry_click(
                                                        ix,
                                                        click_event,
                                                        window,
                                                        cx,
                                                    );
                                                },
                                            ))
                                        })
                                        .when(!is_disabled, |this| {
                                            let state_entity = state_entity.clone();
                                            match drag_handle_width {
                                                Some(handle_width) => this.child(
                                                    div()
                                                        .id(("dnd-vlist-handle", ix))
                                                        .absolute()
                                                        .top_0()
                                                        .left_0()
                                                        .bottom_0()
                                                        .w(handle_width)
                                                        .cursor(CursorStyle::OpenHand)
                                                        .on_drag(
                                                            drag_value,
                                                            move |drag, _offset, window, cx: &mut App| {
                                                                state_entity.update(
                                                                    cx,
                                                                    |state, cx| {
                                                                        state.on_drag_start(
                                                                            drag, window, cx,
                                                                        );
                                                                    },
                                                                );
                                                                let label = drag.label.clone();
                                                                cx.new(|_| DragGhost::new(label))
                                                            },
                                                        ),
                                                ),
                                                None => this.on_drag(
                                                    drag_value,
                                                    move |drag, _offset, window, cx: &mut App| {
                                                        state_entity.update(cx, |state, cx| {
                                                            state.on_drag_start(drag, window, cx);
                                                        });
                                                        let label = drag.label.clone();
                                                        cx.new(|_| DragGhost::new(label))
                                                    },
                                                ),
                                            }
                                        });

                                    rows.push(row);
                                }
                                rows
                            },
                        )
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
                    .child(Scrollbar::uniform_scroll(
                        &self.scrollbar_state,
                        &self.scroll_handle,
                    )),
            )
    }
}

/// A draggable list element that supports drag-and-drop reordering (virtual, variable-height).
#[derive(IntoElement)]
pub struct DndVList<T: 'static> {
    id: ElementId,
    state: Entity<DndVListState<T>>,
    style: StyleRefinement,
    render_item:
        Rc<dyn Fn(usize, &DndVListItem<T>, DndVListRowState, &mut Window, &mut App) -> ListItem>,
}

impl<T: 'static> DndVList<T> {
    pub fn new<R>(state: &Entity<DndVListState<T>>, render_item: R) -> Self
    where
        R: Fn(usize, &DndVListItem<T>, DndVListRowState, &mut Window, &mut App) -> ListItem
            + 'static,
    {
        Self {
            id: ElementId::Name(format!("dnd-vlist-{}", state.entity_id()).into()),
            state: state.clone(),
            style: StyleRefinement::default(),
            render_item: Rc::new(move |ix, item, row_state, window, cx| {
                render_item(ix, item, row_state, window, cx)
            }),
        }
    }
}

impl<T: 'static> Styled for DndVList<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<T: 'static> RenderOnce for DndVList<T> {
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

fn sanitize_item_size(mut size: Size<Pixels>) -> Size<Pixels> {
    let height: f32 = size.height.into();
    if !height.is_finite() || height <= 0.0 {
        size.height = px(1.);
    }
    size
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(items: &[DndVListItem<&'static str>]) -> Vec<String> {
        items.iter().map(|i| i.id.to_string()).collect()
    }

    fn heights(items: &[DndVListItem<&'static str>]) -> Vec<f32> {
        items
            .iter()
            .map(|i| {
                let h: f32 = i.item_size().height.into();
                h
            })
            .collect()
    }

    #[test]
    fn reorder_moves_item_down_and_keeps_heights_in_sync() {
        let mut items = vec![
            DndVListItem::new("A", "A", "A").height(px(10.)),
            DndVListItem::new("B", "B", "B").height(px(20.)),
            DndVListItem::new("C", "C", "C").height(px(30.)),
            DndVListItem::new("D", "D", "D").height(px(40.)),
        ];

        let from_ix = 1;
        let gap_index = 4; // drop after last
        let mut to_ix = gap_index;
        if to_ix > from_ix {
            to_ix -= 1;
        }
        to_ix = to_ix.min(items.len() - 1);

        let item = items.remove(from_ix);
        items.insert(to_ix, item);

        assert_eq!(
            ids(&items),
            vec![
                "A".to_string(),
                "C".to_string(),
                "D".to_string(),
                "B".to_string()
            ]
        );
        assert_eq!(heights(&items), vec![10.0, 30.0, 40.0, 20.0]);
    }

    #[test]
    fn sanitize_item_size_clamps_non_positive_heights() {
        let s = sanitize_item_size(size(px(0.), px(0.)));
        let h: f32 = s.height.into();
        assert!(h > 0.0);
    }
}
