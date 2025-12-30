use gpui::{
    Context, IntoElement, ParentElement as _, Render, SharedString, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;

pub(crate) struct DragGhost {
    label: SharedString,
}

impl DragGhost {
    pub(crate) fn new(label: SharedString) -> Self {
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

pub(crate) fn reorder_to_index_for_drop_on_row(
    from_ix: usize,
    target_ix: usize,
    item_count: usize,
) -> usize {
    let gap_index = if target_ix < from_ix {
        target_ix
    } else if target_ix > from_ix {
        target_ix.saturating_add(1)
    } else {
        from_ix
    };

    reorder_to_index_from_gap(from_ix, gap_index, item_count)
}

pub(crate) fn reorder_to_index_for_drop_after_last(from_ix: usize, item_count: usize) -> usize {
    let gap_index = item_count;
    reorder_to_index_from_gap(from_ix, gap_index, item_count)
}

fn reorder_to_index_from_gap(from_ix: usize, gap_index: usize, item_count: usize) -> usize {
    let mut to_ix = gap_index;
    if to_ix > from_ix {
        to_ix = to_ix.saturating_sub(1);
    }
    to_ix.min(item_count.saturating_sub(1))
}
