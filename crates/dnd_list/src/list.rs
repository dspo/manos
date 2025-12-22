use std::{ops::Range, rc::Rc};

use gpui::{
    App, AppContext as _, Context, CursorStyle, ElementId, Entity, EntityId, FocusHandle,
    InteractiveElement as _, IntoElement, ListSizingBehavior, Modifiers, ParentElement as _,
    Pixels, Render, RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement,
    Styled, UniformListScrollHandle, Window, div, prelude::FluentBuilder as _, px, uniform_list,
};
use gpui_component::list::ListItem;
use gpui_component::scroll::{Scrollbar, ScrollbarState};
use gpui_component::{ActiveTheme as _, StyledExt as _};

const CONTEXT: &str = "DndList";

/// Create a [`DndList`].
pub fn dnd_list<T, R>(state: &Entity<DndListState<T>>, render_item: R) -> DndList<T>
where
    T: 'static,
    R: Fn(usize, &DndListItem<T>, DndListRowState, &mut Window, &mut App) -> ListItem + 'static,
{
    DndList::new(state, render_item)
}

/// A single item in a [`DndListState`].
#[derive(Clone)]
pub struct DndListItem<T> {
    pub id: SharedString,
    pub label: SharedString,
    pub data: T,
    disabled: bool,
}

impl<T> DndListItem<T> {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, data: T) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            data,
            disabled: false,
        }
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
struct DndListDrag {
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
pub enum DndListDropTarget {
    Before,
    After,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DndListRowState {
    pub selected: bool,
    pub dragging: bool,
    pub drop_target: Option<DndListDropTarget>,
}

#[derive(Clone, Debug)]
pub struct DndListReorder {
    pub item_id: SharedString,
    pub from: usize,
    pub to: usize,
}

struct DndListStateCallbacks<T> {
    can_drop: Option<Rc<dyn Fn(&DndListReorder, &[DndListItem<T>], Modifiers) -> bool>>,
    on_reorder: Option<Rc<dyn Fn(&DndListReorder, &[DndListItem<T>])>>,
}

impl<T> Default for DndListStateCallbacks<T> {
    fn default() -> Self {
        Self {
            can_drop: None,
            on_reorder: None,
        }
    }
}

/// State for a draggable, reorderable list.
pub struct DndListState<T> {
    focus_handle: FocusHandle,
    items: Vec<DndListItem<T>>,
    scrollbar_state: ScrollbarState,
    scroll_handle: UniformListScrollHandle,
    drag_handle_width: Option<Pixels>,
    selected_ix: Option<usize>,
    dragged_id: Option<SharedString>,
    dragged_ix: Option<usize>,
    callbacks: DndListStateCallbacks<T>,
    render_item:
        Rc<dyn Fn(usize, &DndListItem<T>, DndListRowState, &mut Window, &mut App) -> ListItem>,
}

impl<T: 'static> DndListState<T> {
    pub fn new(cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            items: Vec::new(),
            scrollbar_state: ScrollbarState::default(),
            scroll_handle: UniformListScrollHandle::default(),
            drag_handle_width: Some(px(32.)),
            selected_ix: None,
            dragged_id: None,
            dragged_ix: None,
            callbacks: DndListStateCallbacks::default(),
            render_item: Rc::new(|_, _, _, _, _| ListItem::new("dnd-list-empty")),
        }
    }

    pub fn items(mut self, items: impl Into<Vec<DndListItem<T>>>) -> Self {
        self.items = items.into();
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

    pub fn set_items(&mut self, items: impl Into<Vec<DndListItem<T>>>, cx: &mut Context<Self>) {
        self.items = items.into();
        self.selected_ix = None;
        self.dragged_id = None;
        self.dragged_ix = None;
        cx.notify();
    }

    pub fn items_ref(&self) -> &[DndListItem<T>] {
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
        can_drop: impl Fn(&DndListReorder, &[DndListItem<T>], Modifiers) -> bool + 'static,
    ) -> Self {
        self.callbacks.can_drop = Some(Rc::new(can_drop));
        self
    }

    /// Provide a callback invoked after a successful reorder.
    pub fn on_reorder(
        mut self,
        on_reorder: impl Fn(&DndListReorder, &[DndListItem<T>]) + 'static,
    ) -> Self {
        self.callbacks.on_reorder = Some(Rc::new(on_reorder));
        self
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

    fn on_drag_start(&mut self, drag: &DndListDrag, _window: &mut Window, cx: &mut Context<Self>) {
        self.dragged_id = Some(drag.item_id.clone());
        self.dragged_ix = self
            .items
            .iter()
            .position(|item| item.id == drag.item_id)
            .or(Some(drag.ix));
        self.selected_ix = self.dragged_ix;
        cx.notify();
    }

    fn drop_is_allowed(&mut self, reorder: &DndListReorder, modifiers: Modifiers) -> bool {
        self.callbacks
            .can_drop
            .as_ref()
            .map(|f| f(reorder, &self.items, modifiers))
            .unwrap_or(true)
    }

    fn on_drop_on_row(
        &mut self,
        drag: &DndListDrag,
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

        let reorder = DndListReorder {
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
        drag: &DndListDrag,
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

        let reorder = DndListReorder {
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

        self.selected_ix = Some(to_ix);
        self.dragged_ix = Some(to_ix);
        cx.notify();

        if let Some(on_reorder) = self.callbacks.on_reorder.as_ref() {
            on_reorder(&reorder, &self.items);
        }
    }
}

impl<T: 'static> Render for DndListState<T> {
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

        div()
            .id("dnd-list-state")
            .size_full()
            .relative()
            .child(
                uniform_list("items", self.items.len(), {
                    cx.processor(move |state, visible_range: Range<usize>, window, cx| {
                        let drop_target_bg = cx.theme().drop_target;
                        let drag_border = cx.theme().drag_border;
                        let mut rows = Vec::with_capacity(visible_range.len());
                        for ix in visible_range {
                            let item = &state.items[ix];

                            let selected = Some(ix) == state.selected_ix;
                            let dragging = dragged_id.as_ref().is_some_and(|id| *id == item.id)
                                && cx.has_active_drag();

                            let row_state = DndListRowState {
                                selected,
                                dragging,
                                drop_target: None,
                            };

                            let list_item = (render_item)(ix, item, row_state, window, cx);
                            let drag_value = DndListDrag {
                                list_id,
                                item_id: item.id.clone(),
                                label: item.label.clone(),
                                ix,
                            };

                            let is_disabled = item.is_disabled();
                            let row = div()
                                .id(ix)
                                .relative()
                                .child(list_item.disabled(is_disabled).selected(selected))
                                .drag_over::<DndListDrag>(move |style, drag, _window, _cx| {
                                    if drag.list_id != list_id {
                                        return style;
                                    }

                                    let mut style =
                                        style.bg(drop_target_bg.alpha(drop_target_bg.a.max(0.2)));
                                    style = style.border_color(drag_border);
                                    if ix < drag.ix {
                                        style = style.border_t_2();
                                    } else if ix > drag.ix {
                                        style = style.border_b_2();
                                    }
                                    style
                                })
                                .on_drop::<DndListDrag>(cx.listener(
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
                                                .id(("dnd-list-handle", ix))
                                                .absolute()
                                                .top_0()
                                                .left_0()
                                                .bottom_0()
                                                .w(handle_width)
                                                .cursor(CursorStyle::OpenHand)
                                                .on_drag(
                                                    drag_value,
                                                    move |drag, _offset, window, cx| {
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
                                            move |drag, _offset, window, cx| {
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
                    })
                })
                .on_drop::<DndListDrag>(cx.listener(Self::on_drop_after_last))
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
    }
}

/// A draggable list element that supports drag-and-drop reordering.
#[derive(IntoElement)]
pub struct DndList<T: 'static> {
    id: ElementId,
    state: Entity<DndListState<T>>,
    style: StyleRefinement,
    render_item:
        Rc<dyn Fn(usize, &DndListItem<T>, DndListRowState, &mut Window, &mut App) -> ListItem>,
}

impl<T: 'static> DndList<T> {
    pub fn new<R>(state: &Entity<DndListState<T>>, render_item: R) -> Self
    where
        R: Fn(usize, &DndListItem<T>, DndListRowState, &mut Window, &mut App) -> ListItem + 'static,
    {
        Self {
            id: ElementId::Name(format!("dnd-list-{}", state.entity_id()).into()),
            state: state.clone(),
            style: StyleRefinement::default(),
            render_item: Rc::new(move |ix, item, row_state, window, cx| {
                render_item(ix, item, row_state, window, cx)
            }),
        }
    }
}

impl<T: 'static> Styled for DndList<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<T: 'static> RenderOnce for DndList<T> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(items: &[DndListItem<&'static str>]) -> Vec<String> {
        items.iter().map(|i| i.id.to_string()).collect()
    }

    #[test]
    fn reorder_moves_item_down() {
        let mut items = vec![
            DndListItem::new("A", "A", "A"),
            DndListItem::new("B", "B", "B"),
            DndListItem::new("C", "C", "C"),
            DndListItem::new("D", "D", "D"),
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
    }

    #[test]
    fn reorder_moves_item_up() {
        let mut items = vec![
            DndListItem::new("A", "A", "A"),
            DndListItem::new("B", "B", "B"),
            DndListItem::new("C", "C", "C"),
            DndListItem::new("D", "D", "D"),
        ];

        let from_ix = 3;
        let gap_index = 1; // drop before B
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
                "D".to_string(),
                "B".to_string(),
                "C".to_string()
            ]
        );
    }

    #[test]
    fn reorder_noop_when_dropped_around_self() {
        let items = vec![
            DndListItem::new("A", "A", "A"),
            DndListItem::new("B", "B", "B"),
            DndListItem::new("C", "C", "C"),
        ];

        let from_ix = 1;
        for gap_index in [from_ix, from_ix + 1] {
            let mut items = items.clone();
            let mut to_ix = gap_index;
            if to_ix > from_ix {
                to_ix -= 1;
            }
            to_ix = to_ix.min(items.len() - 1);

            if from_ix != to_ix {
                let item = items.remove(from_ix);
                items.insert(to_ix, item);
            }
            assert_eq!(
                ids(&items),
                vec!["A".to_string(), "B".to_string(), "C".to_string()]
            );
        }
    }
}
