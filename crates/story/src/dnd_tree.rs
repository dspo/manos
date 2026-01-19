use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::ActiveTheme as _;
use gpui_component::list::ListItem;
use gpui_component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_manos_dnd::{
    DndTreeEntry, DndTreeIndicatorCap, DndTreeItem, DndTreeRowState, DndTreeState, dnd_tree,
};

pub struct DndTreeExample {
    tree: Entity<DndTreeState>,
}

impl DndTreeExample {
    pub fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
        let items = demo_items();
        let tree = cx.new(|cx| {
            DndTreeState::new(cx)
                .indent_width(px(16.))
                .indent_offset(px(10.))
                .indicator_color(cx.theme().foreground)
                .indicator_thickness(px(1.))
                .indicator_cap(DndTreeIndicatorCap::StartAndEndBars {
                    width: px(2.),
                    height: px(8.),
                })
                .items(items)
        });
        cx.new(|_| Self { tree })
    }
}

impl Render for DndTreeExample {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let tree_dump = format_tree(self.tree.read(cx).root_items());
        let selected_id = self
            .tree
            .read(cx)
            .selected_entry()
            .map(|entry| entry.item().id.to_string())
            .unwrap_or_else(|| "<none>".to_string());

        v_flex()
            .size_full()
            .p(px(16.))
            .gap_y_3()
            .child(
                v_flex()
                    .gap_y_1()
                    .child(div().text_xl().font_weight(FontWeight::BOLD).child("DnD Tree"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("Tip: drag nodes to reorder. The target row highlights with an insertion line (upper half = Before, lower half = After). Horizontal drag changes depth (right = deeper, left = shallower). When the horizontal delta > 24px and dominates: left promotes one level; right demotes one level (as a child of the left sibling, auto-expands). Indicator styling: `indicator_style / indicator_color / indicator_cap`."),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(format!("Selected: {selected_id}")),
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
                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Tree"))
                            .child(
                                div()
                                    .flex_1()
                                    .min_h(px(0.))
                                    .rounded(px(12.))
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.background)
                                    .child(dnd_tree(&self.tree, move |ix, entry, row_state, _window, cx| {
                                        render_tree_row(ix, entry, row_state, cx)
                                    })),
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
                                    .child("Debug (tree structure)"),
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
    entry: &DndTreeEntry,
    row_state: DndTreeRowState,
    cx: &mut App,
) -> ListItem {
    let theme = cx.theme();
    let indent = px(16.) * entry.depth();
    let is_directory = entry.can_accept_children();
    let expanded = entry.is_expanded();

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

    ListItem::new(ix)
        .pl(px(10.) + indent)
        .when(row_state.dragging, |this| this.opacity(0.4))
        .child(
            h_flex()
                .gap_x_2()
                .items_center()
                .child(Icon::empty().path(icon_path).small().text_color(icon_color))
                .child(entry.item().label.clone()),
        )
}

fn render_tree_dump(text: String) -> impl IntoElement {
    let lines = text
        .lines()
        .map(|line| div().text_sm().child(line.to_string()));
    v_flex().gap_y_0p5().children(lines)
}

fn format_tree(items: &[DndTreeItem]) -> String {
    fn walk(items: &[DndTreeItem], depth: usize, out: &mut String) {
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

fn demo_items() -> Vec<DndTreeItem> {
    vec![
        DndTreeItem::new("src", "src")
            .expanded(true)
            .child(
                DndTreeItem::new("src/ui", "ui")
                    .expanded(true)
                    .child(DndTreeItem::new("src/ui/button.rs", "button.rs"))
                    .child(DndTreeItem::new("src/ui/icon.rs", "icon.rs"))
                    .child(DndTreeItem::new("src/ui/dnd_tree.rs", "dnd_tree.rs")),
            )
            .child(DndTreeItem::new("src/main.rs", "main.rs"))
            .child(DndTreeItem::new("src/lib.rs", "lib.rs")),
        DndTreeItem::new("Cargo.toml", "Cargo.toml"),
        DndTreeItem::new("Cargo.lock", "Cargo.lock").disabled(true),
        DndTreeItem::new("README.md", "README.md"),
    ]
}
