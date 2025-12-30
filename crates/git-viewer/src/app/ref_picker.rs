use super::*;

impl GitViewerApp {
    pub(super) fn open_ref_picker_overlay(
        &mut self,
        side: RefPickerSide,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.git_available {
            window.push_notification(
                Notification::new().message("未检测到 git 命令，无法选择 refs"),
                cx,
            );
            return;
        }

        self.file_history_overlay = None;
        self.command_palette_overlay = None;

        let filter_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("搜索 refs（branch / remote / tag）")
                .default_value("")
        });

        self.ref_picker_overlay = Some(RefPickerOverlayState {
            side,
            refs: Vec::new(),
            loading: true,
            selected: 0,
            filter_input: filter_input.clone(),
        });

        filter_input.update(cx, |state, cx| state.focus(window, cx));

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        cx.spawn_in(window, async move |_, window| {
            let repo_root_bg = repo_root.clone();
            let refs = window
                .background_executor()
                .spawn(async move { fetch_git_refs(&repo_root_bg, 10_000) })
                .await;

            window
                .update(|window, cx| match refs {
                    Ok(refs) => {
                        this.update(cx, |this, cx| {
                            if let Some(overlay) = this.ref_picker_overlay.as_mut() {
                                overlay.loading = false;
                                overlay.refs = refs;
                                overlay.selected = 0;
                                cx.notify();
                            }
                        });
                    }
                    Err(err) => {
                        window.push_notification(
                            Notification::new().message(format!("获取 refs 失败：{err:#}")),
                            cx,
                        );
                        this.update(cx, |this, cx| {
                            if let Some(overlay) = this.ref_picker_overlay.as_mut() {
                                overlay.loading = false;
                                cx.notify();
                            }
                        });
                    }
                })
                .ok();

            Some(())
        })
        .detach();
    }

    pub(super) fn close_ref_picker_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ref_picker_overlay.take().is_some() {
            window.focus(&self.focus_handle);
            cx.notify();
        }
    }

    fn apply_selected_ref_picker_entry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(overlay) = self.ref_picker_overlay.as_ref() else {
            return;
        };
        if overlay.loading || overlay.refs.is_empty() {
            return;
        }

        let query = overlay.filter_input.read(cx).value().to_string();
        let filtered = filter_refs(&overlay.refs, &query);
        let Some(entry) = filtered.get(overlay.selected.min(filtered.len().saturating_sub(1)))
        else {
            return;
        };

        let side = overlay.side;
        let value = entry.name.clone();
        self.ref_picker_overlay = None;

        match side {
            RefPickerSide::Left => self.compare_left_input.update(cx, |state, cx| {
                state.set_value(value, window, cx);
            }),
            RefPickerSide::Right => self.compare_right_input.update(cx, |state, cx| {
                state.set_value(value, window, cx);
            }),
        }

        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn handle_ref_picker_overlay_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(overlay) = self.ref_picker_overlay.as_mut() else {
            return false;
        };

        match event.keystroke.key.as_str() {
            "escape" => {
                self.close_ref_picker_overlay(window, cx);
                true
            }
            "enter" => {
                self.apply_selected_ref_picker_entry(window, cx);
                true
            }
            "up" | "down" | "pageup" | "pagedown" => {
                let query = overlay.filter_input.read(cx).value().to_string();
                let filtered_len = filter_refs(&overlay.refs, &query).len();
                if filtered_len == 0 {
                    return true;
                }

                let step = match event.keystroke.key.as_str() {
                    "pageup" | "pagedown" => 10usize,
                    _ => 1usize,
                };

                let mut selected = overlay.selected.min(filtered_len.saturating_sub(1));
                match event.keystroke.key.as_str() {
                    "up" | "pageup" => selected = selected.saturating_sub(step),
                    "down" | "pagedown" => {
                        selected = (selected + step).min(filtered_len.saturating_sub(1))
                    }
                    _ => {}
                }

                overlay.selected = selected;
                cx.notify();
                true
            }
            _ => false,
        }
    }

    pub(super) fn render_ref_picker_overlay(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let overlay = self.ref_picker_overlay.as_mut()?;
        let theme = cx.theme();
        let app = cx.entity();

        let loading = overlay.loading;
        let side = overlay.side;
        let filter_input = overlay.filter_input.clone();

        let query = overlay.filter_input.read(cx).value().to_string();
        let filtered = filter_refs(&overlay.refs, &query);
        let selected = overlay.selected.min(filtered.len().saturating_sub(1));

        let can_apply = !loading && !filtered.is_empty();

        let list: Vec<AnyElement> = if loading {
            vec![
                div()
                    .px(px(12.))
                    .py(px(10.))
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("加载 refs 中…")
                    .into_any_element(),
            ]
        } else if filtered.is_empty() {
            vec![
                div()
                    .px(px(12.))
                    .py(px(10.))
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("没有匹配的 refs")
                    .into_any_element(),
            ]
        } else {
            filtered
                .iter()
                .take(220)
                .enumerate()
                .map(|(index, entry)| {
                    let is_selected = index == selected;
                    let kind_label: SharedString = match entry.kind {
                        GitRefKind::LocalBranch => "branch".into(),
                        GitRefKind::RemoteBranch => "remote".into(),
                        GitRefKind::Tag => "tag".into(),
                    };

                    let name = entry.name.clone();
                    let app = app.clone();
                    div()
                        .id(("ref-picker-item", index))
                        .flex()
                        .flex_row()
                        .items_center()
                        .h(px(28.))
                        .px(px(10.))
                        .rounded(px(6.))
                        .text_sm()
                        .when(is_selected, |this| {
                            this.bg(theme.accent)
                                .text_color(theme.accent_foreground)
                                .cursor_default()
                        })
                        .when(!is_selected, |this| {
                            this.bg(theme.transparent)
                                .text_color(theme.popover_foreground)
                                .cursor_pointer()
                                .hover(|this| {
                                    this.bg(theme.accent.alpha(0.4))
                                        .text_color(theme.accent_foreground)
                                })
                                .active(|this| {
                                    this.bg(theme.accent).text_color(theme.accent_foreground)
                                })
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    window.prevent_default();
                                    app.update(cx, |this, cx| {
                                        if let Some(overlay) = this.ref_picker_overlay.as_mut() {
                                            overlay.selected = index;
                                        }
                                        cx.notify();
                                    });
                                })
                        })
                        .child(
                            div()
                                .w(px(60.))
                                .flex_none()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(kind_label),
                        )
                        .child(div().truncate().child(name))
                        .into_any_element()
                })
                .collect()
        };

        let side_left_active = side == RefPickerSide::Left;
        let side_right_active = side == RefPickerSide::Right;

        let overlay_container = div()
            .id("ref-picker-overlay")
            .w(px(560.))
            .max_w(relative(0.92))
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .rounded(theme.radius)
            .shadow_lg()
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(12.))
            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_key_down({
                let app = app.clone();
                move |event, window, cx| {
                    let handled = app.update(cx, |this, cx| {
                        this.handle_ref_picker_overlay_key(event, window, cx)
                    });
                    if handled {
                        window.prevent_default();
                        cx.stop_propagation();
                    }
                }
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child("选择 refs")
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("支持 branch / remote branch / tag"),
                            ),
                    )
                    .child(
                        Button::new("ref-picker-overlay-close")
                            .label("关闭 (Esc)")
                            .ghost()
                            .on_click({
                                let app = app.clone();
                                move |_, window, cx| {
                                    app.update(cx, |this, cx| {
                                        this.close_ref_picker_overlay(window, cx);
                                    });
                                }
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .child(Input::new(&filter_input).w_full().small())
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                Button::new("ref-picker-side-left")
                                    .label("左侧")
                                    .small()
                                    .when(side_left_active, |this| this.primary())
                                    .when(!side_left_active, |this| this.ghost())
                                    .on_click({
                                        let app = app.clone();
                                        move |_, _window, cx| {
                                            app.update(cx, |this, cx| {
                                                if let Some(overlay) =
                                                    this.ref_picker_overlay.as_mut()
                                                {
                                                    overlay.side = RefPickerSide::Left;
                                                }
                                                cx.notify();
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Button::new("ref-picker-side-right")
                                    .label("右侧")
                                    .small()
                                    .when(side_right_active, |this| this.primary())
                                    .when(!side_right_active, |this| this.ghost())
                                    .on_click({
                                        let app = app.clone();
                                        move |_, _window, cx| {
                                            app.update(cx, |this, cx| {
                                                if let Some(overlay) =
                                                    this.ref_picker_overlay.as_mut()
                                                {
                                                    overlay.side = RefPickerSide::Right;
                                                }
                                                cx.notify();
                                            });
                                        }
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .id("ref-picker-overlay-list")
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .min_h(px(0.))
                    .max_h(px(360.))
                    .overflow_y_scroll()
                    .border_1()
                    .border_color(theme.border.alpha(0.5))
                    .rounded(theme.radius)
                    .p(px(6.))
                    .children(list),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("↑↓ 选择 · Enter 填入 · Esc 关闭"),
                    )
                    .child(
                        Button::new("ref-picker-overlay-apply")
                            .label("填入 ref")
                            .primary()
                            .small()
                            .disabled(!can_apply)
                            .on_click({
                                let app = app.clone();
                                move |_, window, cx| {
                                    app.update(cx, |this, cx| {
                                        this.apply_selected_ref_picker_entry(window, cx);
                                    });
                                }
                            }),
                    ),
            );

        Some(
            div()
                .id("ref-picker-overlay-backdrop")
                .absolute()
                .top(px(0.))
                .bottom(px(0.))
                .left(px(0.))
                .right(px(0.))
                .bg(theme.background.alpha(0.75))
                .flex()
                .flex_row()
                .justify_center()
                .pt(px(72.))
                .on_mouse_down(MouseButton::Left, {
                    let app = app.clone();
                    move |_, window, cx| {
                        window.prevent_default();
                        app.update(cx, |this, cx| {
                            this.close_ref_picker_overlay(window, cx);
                        });
                    }
                })
                .child(overlay_container)
                .into_any_element(),
        )
    }
}

fn filter_refs<'a>(refs: &'a [GitRefEntry], query: &str) -> Vec<&'a GitRefEntry> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return refs.iter().collect();
    }

    let tokens: Vec<&str> = query.split_whitespace().filter(|t| !t.is_empty()).collect();
    if tokens.is_empty() {
        return refs.iter().collect();
    }

    refs.iter()
        .filter(|entry| {
            let name = entry.name.to_lowercase();
            let kind = match entry.kind {
                GitRefKind::LocalBranch => "branch",
                GitRefKind::RemoteBranch => "remote",
                GitRefKind::Tag => "tag",
            };

            tokens
                .iter()
                .all(|token| name.contains(token) || kind.contains(token))
        })
        .collect()
}
