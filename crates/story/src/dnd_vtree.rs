use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::ActiveTheme as _;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::list::ListItem;
use gpui_component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_dnd_vtree::{
    DndVTreeIndicatorCap, DndVTreeItem, DndVTreeRowState, DndVTreeState, dnd_vtree,
};

pub struct DndVTreeExample {
    tree: Entity<DndVTreeState>,
}

impl DndVTreeExample {
    pub fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
        let items = demo_items();
        let tree = cx.new(|cx| {
            DndVTreeState::new(cx)
                .indent_width(px(16.))
                .indent_offset(px(10.))
                .indicator_color(cx.theme().foreground)
                .indicator_thickness(px(1.))
                .indicator_cap(DndVTreeIndicatorCap::StartAndEndBars {
                    width: px(2.),
                    height: px(8.),
                })
                .items(items)
        });
        cx.new(|_| Self { tree })
    }
}

impl Render for DndVTreeExample {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let tree_dump = format_tree(self.tree.read(cx).root_items());
        let selected_id = self
            .tree
            .read(cx)
            .selected_entry()
            .map(|entry| entry.item().id.to_string())
            .unwrap_or_else(|| "<none>".to_string());
        let selected_height = self
            .tree
            .read(cx)
            .selected_entry()
            .map(|entry| {
                let h: f32 = entry.row_size().height.into();
                format!("{h:.0}px")
            })
            .unwrap_or_else(|| "<none>".to_string());

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
                            .child("DnD VTree (Variable Height)"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("提示：该示例基于 `gpui-component::v_virtual_list`，不同节点行高不同；拖拽时 y 命中/插入线计算使用前缀和；水平拖动可改变层级。"),
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
                    .gap_x_2()
                    .items_center()
                    .child(
                        Button::new("dnd-vtree-height-inc")
                            .label("选中 +12px")
                            .ghost()
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.tree.update(cx, |tree, cx| {
                                    let Some(ix) = tree.selected_index() else {
                                        return;
                                    };
                                    let current = tree
                                        .selected_entry()
                                        .map(|entry| entry.row_size().height)
                                        .unwrap_or(px(28.));
                                    tree.set_entry_height(ix, current + px(12.), cx);
                                });
                            })),
                    )
                    .child(
                        Button::new("dnd-vtree-height-dec")
                            .label("选中 -12px")
                            .ghost()
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.tree.update(cx, |tree, cx| {
                                    let Some(ix) = tree.selected_index() else {
                                        return;
                                    };
                                    let current = tree
                                        .selected_entry()
                                        .map(|entry| entry.row_size().height)
                                        .unwrap_or(px(28.));
                                    let current_f32: f32 = current.into();
                                    let next = (current_f32 - 12.0).max(8.0);
                                    tree.set_entry_height(ix, px(next), cx);
                                });
                            })),
                    )
                    .child(
                        Button::new("dnd-vtree-height-toggle")
                            .label("选中 高/低 切换")
                            .ghost()
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.tree.update(cx, |tree, cx| {
                                    let Some(ix) = tree.selected_index() else {
                                        return;
                                    };
                                    let current = tree
                                        .selected_entry()
                                        .map(|entry| entry.row_size().height)
                                        .unwrap_or(px(28.));
                                    let current_f32: f32 = current.into();
                                    let next = if current_f32 >= 48.0 { px(28.) } else { px(72.) };
                                    tree.set_entry_height(ix, next, cx);
                                });
                            })),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(format!("选中行高：{selected_height}")),
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
                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("VTree"))
                            .child(
                                div()
                                    .flex_1()
                                    .min_h(px(0.))
                                    .rounded(px(12.))
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.background)
                                    .child(dnd_vtree(
                                        &self.tree,
                                        move |ix, entry, row_state, _window, cx| {
                                            render_tree_row(ix, entry, row_state, cx)
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
                                    .child("Debug (树结构)"),
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
                                    .child(render_tree_dump(tree_dump)),
                            ),
                    ),
            )
    }
}

fn render_tree_row(
    ix: usize,
    entry: &gpui_dnd_vtree::DndVTreeEntry,
    row_state: DndVTreeRowState,
    cx: &mut App,
) -> ListItem {
    let theme = cx.theme();
    let indent = px(16.) * entry.depth();
    let is_directory = entry.can_accept_children();
    let expanded = entry.is_expanded();
    let height: f32 = entry.row_size().height.into();

    let icon_path = if is_directory {
        if expanded {
            "icons/library.svg"
        } else {
            "icons/square-library.svg"
        }
    } else if row_state.selected {
        "icons/pen-line.svg"
    } else {
        "icons/text-align-start.svg"
    };
    let icon_color = if row_state.selected {
        theme.foreground
    } else {
        theme.muted_foreground
    };

    let left = h_flex()
        .gap_x_2()
        .items_center()
        .child(Icon::empty().path(icon_path).small().text_color(icon_color))
        .child(entry.item().label.clone());
    let right = div()
        .text_xs()
        .text_color(theme.muted_foreground)
        .child(format!("{height:.0}px"));

    ListItem::new(ix)
        .pl(px(10.) + indent)
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

fn render_tree_dump(text: String) -> impl IntoElement {
    let lines = text
        .lines()
        .map(|line| div().text_sm().child(line.to_string()));
    v_flex().gap_y_0p5().children(lines)
}

fn format_tree(items: &[DndVTreeItem]) -> String {
    fn walk(items: &[DndVTreeItem], depth: usize, out: &mut String) {
        for item in items {
            out.push_str(&"  ".repeat(depth));
            out.push_str(item.id.as_str());
            out.push('\n');
            walk(&item.children, depth + 1, out);
        }
    }

    let mut out = String::new();
    walk(items, 0, &mut out);
    out
}

fn demo_items() -> Vec<DndVTreeItem> {
    vec![
        DndVTreeItem::new("src", "src")
            .expanded(true)
            .row_height(px(36.))
            .child(
                DndVTreeItem::new("src/ui", "ui")
                    .expanded(true)
                    .row_height(px(44.))
                    .child(DndVTreeItem::new("src/ui/button.rs", "button.rs").row_height(px(28.)))
                    .child(DndVTreeItem::new("src/ui/icon.rs", "icon.rs").row_height(px(28.)))
                    .child(
                        DndVTreeItem::new("src/ui/dnd_vtree.rs", "dnd_vtree.rs")
                            .row_height(px(56.)),
                    ),
            )
            .child(DndVTreeItem::new("src/main.rs", "main.rs").row_height(px(28.)))
            .child(DndVTreeItem::new("src/lib.rs", "lib.rs").row_height(px(28.))),
        DndVTreeItem::new("Cargo.toml", "Cargo.toml").row_height(px(32.)),
        DndVTreeItem::new("Cargo.lock", "Cargo.lock")
            .row_height(px(32.))
            .disabled(true),
        DndVTreeItem::new("README.md", "README.md").row_height(px(32.)),
    ]
}
