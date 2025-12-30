use super::*;

impl Render for GitViewerApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let main_content = match self.screen {
            AppScreen::StatusList => self.render_main_placeholder(cx).into_any_element(),
            AppScreen::DiffView => self.render_diff_view(window, cx).into_any_element(),
            AppScreen::ConflictView => self.render_conflict_view(window, cx).into_any_element(),
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

        let file_history_overlay = self.render_file_history_overlay(window, cx);
        let ref_picker_overlay = self.render_ref_picker_overlay(window, cx);
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
                    if this.file_history_overlay.is_some() {
                        this.close_file_history_overlay(window, cx);
                        return;
                    }
                    if this.ref_picker_overlay.is_some() {
                        this.close_ref_picker_overlay(window, cx);
                        return;
                    }
                    if this.diff_find_open {
                        this.close_diff_find(window, cx);
                        return;
                    }
                    if matches!(this.screen, AppScreen::DiffView)
                        && this
                            .diff_view
                            .as_ref()
                            .is_some_and(|view| !view.selected_rows.is_empty())
                    {
                        this.clear_diff_selection();
                        cx.notify();
                        return;
                    }
                    match this.screen {
                        AppScreen::DiffView => this.close_diff_view(),
                        AppScreen::ConflictView => this.close_conflict_view(),
                        AppScreen::StatusList => {}
                    }
                    window.focus(&this.focus_handle);
                    cx.notify();
                }))
                .on_action(cx.listener(|this, _: &Next, _window, cx| {
                    match this.screen {
                        AppScreen::DiffView => this.jump_hunk(1),
                        AppScreen::ConflictView => this.jump_conflict(1),
                        AppScreen::StatusList => {}
                    }
                    cx.notify();
                }))
                .on_action(cx.listener(|this, _: &Prev, _window, cx| {
                    match this.screen {
                        AppScreen::DiffView => this.jump_hunk(-1),
                        AppScreen::ConflictView => this.jump_conflict(-1),
                        AppScreen::StatusList => {}
                    }
                    cx.notify();
                }))
                .on_action(cx.listener(|this, _: &ToggleViewMode, _window, cx| {
                    if matches!(this.screen, AppScreen::DiffView) {
                        let next = match this.view_mode {
                            DiffViewMode::Split => DiffViewMode::Inline,
                            DiffViewMode::Inline => DiffViewMode::Split,
                        };
                        this.set_view_mode(next);
                        cx.notify();
                    }
                }))
                .on_action(cx.listener(|this, _: &ToggleSplitLayout, _window, cx| {
                    match this.screen {
                        AppScreen::DiffView => {
                            if this.view_mode == DiffViewMode::Inline {
                                return;
                            }
                        }
                        AppScreen::ConflictView => {}
                        AppScreen::StatusList => return,
                    }

                    this.split_layout = match this.split_layout {
                        SplitLayout::Aligned => SplitLayout::TwoPane,
                        SplitLayout::TwoPane => SplitLayout::Aligned,
                    };
                    cx.notify();
                }))
                .on_action(cx.listener(|this, _: &ToggleWhitespace, window, cx| {
                    if matches!(this.screen, AppScreen::DiffView) {
                        let next = !this.diff_options.ignore_whitespace;
                        this.set_ignore_whitespace(next, window, cx);
                        cx.notify();
                    }
                }))
                .on_action(cx.listener(|this, _: &ToggleDiffFind, window, cx| {
                    if matches!(this.screen, AppScreen::DiffView) {
                        this.toggle_diff_find(window, cx);
                    }
                }))
                .on_action(cx.listener(|this, _: &ExpandAll, _window, cx| {
                    if matches!(this.screen, AppScreen::DiffView) {
                        this.expand_all_folds();
                        cx.notify();
                    }
                }))
                .on_action(
                    cx.listener(|this, _: &ApplyEditor, window, cx| match this.screen {
                        AppScreen::ConflictView => this.apply_conflict_editor(window, cx),
                        AppScreen::DiffView => {
                            if this.show_diff_worktree_editor {
                                this.apply_diff_worktree_editor_preview(window, cx);
                            }
                        }
                        AppScreen::StatusList => {}
                    }),
                )
                .on_action(cx.listener(|this, _: &SaveConflict, window, cx| {
                    match this.screen {
                        AppScreen::ConflictView => {
                            this.save_conflict_to_working_tree(false, window, cx);
                        }
                        AppScreen::DiffView => {
                            if this.show_diff_worktree_editor {
                                this.save_diff_worktree_editor_to_disk(window, cx);
                            }
                        }
                        AppScreen::StatusList => {}
                    }
                    cx.notify();
                }))
                .on_action(cx.listener(
                    |this, _: &SaveConflictAndAdd, window, cx| {
                        if matches!(this.screen, AppScreen::ConflictView) {
                            this.save_conflict_to_working_tree(true, window, cx);
                            cx.notify();
                        }
                    },
                ))
            })
            .child(content);

        if let Some(overlay) = file_history_overlay {
            root = root.child(overlay);
        }

        if let Some(overlay) = ref_picker_overlay {
            root = root.child(overlay);
        }

        if let Some(overlay) = command_palette_overlay {
            root = root.child(overlay);
        }

        root
    }
}
