use super::*;

impl GitViewerApp {
    pub(super) fn render_file_view(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let app = cx.entity();

        let path = self.opened_file.as_ref().map(|entry| entry.path.clone());
        let (can_prev_file, can_next_file) = self.file_nav_state(path.as_deref());

        let title: SharedString = path
            .as_ref()
            .map(|path| path.clone().into())
            .unwrap_or_else(|| "Coming soon".into());

        let toolbar = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .p(px(12.))
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        Button::new("file-view-back")
                            .icon(IconName::ArrowLeft)
                            .ghost()
                            .small()
                            .compact()
                            .tooltip("返回")
                            .on_click({
                                let app = app.clone();
                                move |_, window, cx| {
                                    app.update(cx, |this, cx| {
                                        this.close_file_view();
                                        window.focus(&this.focus_handle);
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(
                        Button::new("file-view-prev-file")
                            .icon(PlateIconName::ArrowUpToLine)
                            .ghost()
                            .small()
                            .compact()
                            .disabled(!can_prev_file)
                            .tooltip("上一个文件")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_adjacent_file(-1, window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("file-view-next-file")
                            .icon(PlateIconName::ArrowDownToLine)
                            .ghost()
                            .small()
                            .compact()
                            .disabled(!can_next_file)
                            .tooltip("下一个文件")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_adjacent_file(1, window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .min_w(px(0.))
                            .truncate()
                            .text_sm()
                            .text_color(theme.foreground)
                            .child(title),
                    ),
            )
            .child(
                Button::new("file-view-refresh")
                    .icon(PlateIconName::FolderSync)
                    .ghost()
                    .small()
                    .compact()
                    .disabled(!self.git_available || self.loading)
                    .tooltip("刷新 Git 状态")
                    .on_click({
                        let app = app.clone();
                        move |_, window, cx| {
                            app.update(cx, |this, cx| {
                                this.refresh_git_status(window, cx);
                                cx.notify();
                            });
                        }
                    }),
            );

        let body = div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .items_center()
            .justify_center()
            .text_color(theme.muted_foreground)
            .child(div().text_lg().child("Coming soon"));

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(toolbar)
            .child(body)
    }
}
