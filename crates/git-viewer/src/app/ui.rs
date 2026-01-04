use super::*;

impl Render for GitViewerApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let main_content = match self.screen {
            AppScreen::StatusList => self.render_main_placeholder(cx).into_any_element(),
            AppScreen::FileView => self.render_file_view(window, cx).into_any_element(),
        };

        let sidebar = self.render_sidebar(window, cx);
        let theme = cx.theme();

        let title_bar = TitleBar::new().child(
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(theme.foreground)
                        .child(self.workspace_name.clone()),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("·"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(self.branch_name.clone()),
                ),
        );

        let layout = div()
            .id("git-viewer-layout")
            .flex_1()
            .min_h(px(0.))
            .child(
                h_resizable("git-viewer-main-split")
                    .child(
                        resizable_panel()
                            .size(px(360.))
                            .size_range(px(280.)..Pixels::MAX)
                            .child(sidebar),
                    )
                    .child(
                        resizable_panel().child(
                            div()
                                .flex()
                                .flex_col()
                                .size_full()
                                .min_w(px(0.))
                                .child(main_content),
                        ),
                    ),
            )
            .into_any_element();

        let content = div()
            .id("git-viewer-content")
            .flex()
            .flex_col()
            .size_full()
            .child(title_bar)
            .child(layout)
            .into_any_element();

        let command_palette_overlay = self.render_command_palette_overlay(window, cx);

        let mut root = div()
            .id("git-viewer-root")
            .size_full()
            .relative()
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .tab_index(0)
            .when(!window.is_inspector_picking(cx), |this| {
                this.on_action(cx.listener(|this, _: &OpenCommandPalette, window, cx| {
                    if this.command_palette_overlay.is_some() {
                        this.close_command_palette(window, cx);
                    } else {
                        this.open_command_palette(window, cx);
                    }
                }))
                .on_action(cx.listener(|this, _: &Back, window, cx| {
                    if this.command_palette_overlay.is_some() {
                        this.close_command_palette(window, cx);
                        return;
                    }
                    match this.screen {
                        AppScreen::FileView => this.close_file_view(),
                        AppScreen::StatusList => {}
                    }
                    window.focus(&this.focus_handle);
                    cx.notify();
                }))
                .on_action(cx.listener(|this, _: &Next, window, cx| {
                    if matches!(this.screen, AppScreen::FileView) {
                        this.open_adjacent_file(1, window, cx);
                    }
                    cx.notify();
                }))
                .on_action(cx.listener(|this, _: &Prev, window, cx| {
                    if matches!(this.screen, AppScreen::FileView) {
                        this.open_adjacent_file(-1, window, cx);
                    }
                    cx.notify();
                }))
            })
            .child(content);

        if let Some(overlay) = command_palette_overlay {
            root = root.child(overlay);
        }

        root
    }
}
