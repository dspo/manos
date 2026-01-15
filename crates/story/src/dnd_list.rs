use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::ActiveTheme as _;
use gpui_component::list::ListItem;
use gpui_component::{Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_manos_dnd::{DndListItem, DndListRowState, DndListState, dnd_list};

pub struct DndListExample {
    list: Entity<DndListState<()>>,
}

impl DndListExample {
    pub fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
        let items = demo_items();
        let list = cx.new(|cx| DndListState::new(cx).items(items).drag_on_row());
        cx.new(|_| Self { list })
    }
}

impl Render for DndListExample {
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
            .map(|(ix, item)| format!("{ix:02}  {}", item.id))
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
                            .child("DnD List"),
                    )
                    .child(div().text_sm().text_color(theme.muted_foreground).child(
                        "提示：拖拽整行以重排（也可配置为仅手柄可拖）；目标行会高亮并显示黑色插入线（上/下分别表示插入到该行之前/之后）；禁用项不可拖拽。",
                    ))
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
                            .w(px(420.))
                            .min_w(px(0.))
                            .h_full()
                            .gap_y_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("List"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_h(px(0.))
                                    .rounded(px(12.))
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.background)
                                    .child(dnd_list(
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
                                    .child("Debug (当前顺序)"),
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
    item: &DndListItem<()>,
    row_state: DndListRowState,
    cx: &mut App,
) -> ListItem {
    let theme = cx.theme();
    let meta_icon = IconName::File;
    let handle_icon = IconName::Menu;

    ListItem::new(ix)
        .when(row_state.dragging, |this| this.opacity(0.4))
        .child(
            h_flex()
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
                .child(item.label.clone()),
        )
}

fn render_dump(text: String) -> impl IntoElement {
    let lines = text
        .lines()
        .map(|line| div().text_sm().child(line.to_string()));
    v_flex().gap_y_0p5().children(lines)
}

fn demo_items() -> Vec<DndListItem<()>> {
    vec![
        DndListItem::new("file/a.txt", "a.txt", ()),
        DndListItem::new("file/b.txt", "b.txt", ()),
        DndListItem::new("file/c.txt", "c.txt", ()),
        DndListItem::new("file/d.txt", "d.txt", ()),
        DndListItem::new("file/disabled.txt", "disabled.txt", ()).disabled(true),
        DndListItem::new("file/e.txt", "e.txt", ()),
        DndListItem::new("file/f.txt", "f.txt", ()),
        DndListItem::new("file/g.txt", "g.txt", ()),
    ]
}
