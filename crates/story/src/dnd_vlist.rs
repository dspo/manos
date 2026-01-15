use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::ActiveTheme as _;
use gpui_component::list::ListItem;
use gpui_component::{Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_manos_dnd::{DndVListItem, DndVListRowState, DndVListState, dnd_vlist};

pub struct DndVListExample {
    list: Entity<DndVListState<()>>,
}

impl DndVListExample {
    pub fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
        let items = demo_items();
        let list = cx.new(|cx| DndVListState::new(cx).items(items).drag_on_row());
        cx.new(|_| Self { list })
    }
}

impl Render for DndVListExample {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let items = self.list.read(cx).items_ref();
        let selected = self.list.read(cx).selected_index();
        let selected_id = selected
            .and_then(|ix| items.get(ix))
            .map(|item| item.id.to_string())
            .unwrap_or_else(|| "<none>".to_string());
        let dump = items
            .iter()
            .enumerate()
            .map(|(ix, item)| {
                let h: f32 = item.item_size().height.into();
                format!("{ix:02}  {}  h={h:.0}px", item.id)
            })
            .collect::<Vec<_>>()
            .join("\n");

        v_flex()
            .size_full()
            .p(px(16.))
            .gap_y_3()
            .child(
                v_flex()
                    .gap_y_1()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .child("DnD VList (Variable Height)"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("提示：该示例基于 `gpui-component::v_virtual_list`，每行高度不同；拖拽整行以重排。"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(format!("当前选中：{selected_id}")),
                    ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .gap_x_3()
                    .child(
                        v_flex()
                            .w(px(520.))
                            .min_w(px(0.))
                            .h_full()
                            .gap_y_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("VList"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_h(px(0.))
                                    .rounded(px(12.))
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.background)
                                    .child(dnd_vlist(
                                        &self.list,
                                        move |ix, item, row_state, _window, cx| {
                                            render_list_row(ix, item, row_state, cx)
                                        },
                                    )),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(0.))
                            .h_full()
                            .gap_y_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Debug (顺序/高度)"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_h(px(0.))
                                    .rounded(px(12.))
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.background)
                                    .p(px(12.))
                                    .child(render_dump(dump)),
                            ),
                    ),
            )
    }
}

fn render_list_row(
    ix: usize,
    item: &DndVListItem<()>,
    row_state: DndVListRowState,
    cx: &mut App,
) -> ListItem {
    let theme = cx.theme();
    let meta_icon = IconName::File;
    let handle_icon = IconName::Menu;
    let height: f32 = item.item_size().height.into();

    let left = h_flex()
        .gap_x_2()
        .items_center()
        .child(
            Icon::from(handle_icon)
                .small()
                .text_color(theme.muted_foreground),
        )
        .child(
            Icon::from(meta_icon)
                .small()
                .text_color(theme.muted_foreground),
        )
        .child(item.label.clone());
    let right = div()
        .text_xs()
        .text_color(theme.muted_foreground)
        .child(format!("{height:.0}px"));

    ListItem::new(ix)
        .when(row_state.dragging, |this| this.opacity(0.4))
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(left)
                .child(right),
        )
}

fn render_dump(text: String) -> impl IntoElement {
    let lines = text
        .lines()
        .map(|line| div().text_sm().child(line.to_string()));
    v_flex().gap_y_0p5().children(lines)
}

fn demo_items() -> Vec<DndVListItem<()>> {
    let heights = [px(28.), px(40.), px(56.), px(32.)];

    (0..40)
        .map(|ix| {
            let id = format!("row/{ix:02}");
            let label = format!("Row {ix:02}");
            let mut item = DndVListItem::new(id, label, ()).height(heights[ix % heights.len()]);
            if ix == 7 {
                item = item.disabled(true);
            }
            item
        })
        .collect()
}
