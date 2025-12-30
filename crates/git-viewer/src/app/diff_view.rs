use super::*;

impl GitViewerApp {
    pub(super) fn render_compare_target_control(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let git_available = self.git_available;
        let has_file_path = self
            .diff_view
            .as_ref()
            .and_then(|view| view.path.as_deref())
            .is_some();

        let active_target = self
            .diff_view
            .as_ref()
            .map(|view| view.compare_target.clone())
            .unwrap_or_else(|| self.compare_target_preference.clone());

        let compare_label: SharedString = self
            .diff_view
            .as_ref()
            .map(|view| format!("对比: {}", compare_target_label(&view.compare_target)))
            .unwrap_or_else(|| format!("对比: {}", compare_target_label(&active_target)))
            .into();

        let trigger_button = |id: &'static str, disabled: bool| {
            let tooltip: SharedString = if disabled {
                "未检测到 git 命令，无法切换对比目标".into()
            } else {
                compare_label.clone()
            };

            Button::new(id)
                .icon(PlateIconName::GitCompare)
                .ghost()
                .small()
                .compact()
                .disabled(disabled)
                .tooltip(tooltip)
                .on_click(|_, _, _| {})
        };

        if !git_available {
            return trigger_button("sidebar-compare-disabled", true).into_any_element();
        }

        let app = cx.entity();
        let compare_left_input = self.compare_left_input.clone();
        let compare_right_input = self.compare_right_input.clone();

        let app_for_menu = app.clone();
        Popover::new("sidebar-compare-menu")
            .appearance(false)
            .trigger(trigger_button("sidebar-compare-trigger", false))
            .content(move |_, _window, cx| {
                let theme = cx.theme();
                let popover = cx.entity();

                let app_for_items = app_for_menu.clone();
                let popover_for_items = popover.clone();
                let active_target = active_target.clone();
                let make_item = |id: &'static str, label: SharedString, target: CompareTarget| {
                    let app = app_for_items.clone();
                    let popover = popover_for_items.clone();
                    let is_active = active_target == target;

                    div()
                        .id(id)
                        .flex()
                        .items_center()
                        .h(px(28.))
                        .px(px(10.))
                        .rounded(px(6.))
                        .text_sm()
                        .when(is_active, |this| {
                            this.bg(theme.accent)
                                .text_color(theme.accent_foreground)
                                .cursor_default()
                        })
                        .when(!is_active, |this| {
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
                                    let target = target.clone();
                                    app.update(cx, |this, cx| {
                                        this.set_compare_target(target, window, cx);
                                        cx.notify();
                                    });
                                    popover.update(cx, |state, cx| state.dismiss(window, cx));
                                })
                        })
                        .child(label)
                };

                let base_menu = div()
                    .text_sm()
                    .p(px(6.))
                    .w(px(360.))
                    .bg(theme.popover)
                    .border_1()
                    .border_color(theme.border)
                    .rounded(theme.radius)
                    .shadow_md()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .px(px(6.))
                            .py(px(4.))
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("选择对比目标"),
                    )
                    .child(make_item(
                        "diff-compare-head-worktree",
                        "HEAD ↔ 工作区".into(),
                        CompareTarget::HeadToWorktree,
                    ))
                    .child(make_item(
                        "diff-compare-index-worktree",
                        "暂存 ↔ 工作区".into(),
                        CompareTarget::IndexToWorktree,
                    ))
                    .child(make_item(
                        "diff-compare-head-index",
                        "HEAD ↔ 暂存".into(),
                        CompareTarget::HeadToIndex,
                    ));

                if !has_file_path {
                    return base_menu.into_any_element();
                }

                base_menu
                    .child(div().h(px(1.)).bg(theme.border.alpha(0.4)).my(px(4.)))
                    .child(
                        div()
                            .px(px(6.))
                            .py(px(4.))
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("任意版本对比"),
                    )
                    .child(
                        div()
                            .px(px(6.))
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(Input::new(&compare_left_input).w_full().small())
                            .child(Input::new(&compare_right_input).w_full().small())
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .justify_end()
                                    .gap(px(6.))
                                    .child(
                                        Button::new("diff-compare-swap")
                                            .icon(PlateIconName::ArrowLeftRight)
                                            .ghost()
                                            .small()
                                            .compact()
                                            .tooltip("交换左右 ref")
                                            .on_click({
                                                let app = app_for_menu.clone();
                                                move |_, window, cx| {
                                                    app.update(cx, |this, cx| {
                                                        this.swap_compare_refs_inputs(window, cx);
                                                    });
                                                }
                                            }),
                                    )
                                    .child(
                                        Button::new("diff-compare-apply")
                                            .label("应用")
                                            .primary()
                                            .small()
                                            .compact()
                                            .on_click({
                                                let app = app_for_menu.clone();
                                                let popover = popover.clone();
                                                move |_, window, cx| {
                                                    app.update(cx, |this, cx| {
                                                        this.apply_compare_refs_from_inputs(
                                                            window, cx,
                                                        );
                                                        cx.notify();
                                                    });
                                                    popover.update(cx, |state, cx| {
                                                        state.dismiss(window, cx)
                                                    });
                                                }
                                            }),
                                    ),
                            ),
                    )
                    .child(div().h(px(1.)).bg(theme.border.alpha(0.4)).my(px(4.)))
                    .child(
                        Button::new("diff-history-open")
                            .label("从历史选择 commit…")
                            .ghost()
                            .small()
                            .compact()
                            .w_full()
                            .on_click({
                                let app = app_for_menu.clone();
                                let popover = popover.clone();
                                move |_, window, cx| {
                                    app.update(cx, |this, cx| {
                                        this.open_file_history_overlay(window, cx);
                                    });
                                    popover.update(cx, |state, cx| state.dismiss(window, cx));
                                }
                            }),
                    )
                    .child(
                        Button::new("diff-ref-picker-open")
                            .label("从分支/标签选择 ref…")
                            .ghost()
                            .small()
                            .compact()
                            .w_full()
                            .on_click({
                                let app = app_for_menu.clone();
                                let popover = popover.clone();
                                move |_, window, cx| {
                                    app.update(cx, |this, cx| {
                                        this.open_ref_picker_overlay(
                                            RefPickerSide::Left,
                                            window,
                                            cx,
                                        );
                                    });
                                    popover.update(cx, |state, cx| state.dismiss(window, cx));
                                }
                            }),
                    )
                    .into_any_element()
            })
            .into_any_element()
    }

    pub(super) fn render_diff_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let current_path = self.diff_view.as_ref().and_then(|view| view.path.clone());
        let (can_prev_file, can_next_file) = self.file_nav_state(current_path.as_deref());

        if self.diff_view.is_none() {
            return div().p(px(12.)).child("No diff view");
        }

        self.update_diff_find_matches_if_needed(cx);
        let diff_view = self.diff_view.as_mut().unwrap();

        let view_mode = self.view_mode;
        let inline_mode = view_mode == DiffViewMode::Inline;
        let two_pane = matches!(self.split_layout, SplitLayout::TwoPane);
        let title = diff_view.title.clone();
        let compare_target = diff_view.compare_target.clone();
        let ignore_whitespace = self.diff_options.ignore_whitespace;
        let context_lines = self.diff_options.context_lines;
        let hunk_count = diff_view.hunk_rows.len();
        let can_prev_hunk = diff_view.current_hunk > 0;
        let can_next_hunk = diff_view.current_hunk + 1 < diff_view.hunk_rows.len();
        let file_status = diff_view.status.clone().unwrap_or_default();
        let has_file_path = diff_view.path.is_some();
        let has_file_status = !file_status.trim().is_empty();
        let is_default_compare = matches!(
            &compare_target,
            &CompareTarget::HeadToWorktree
                | &CompareTarget::IndexToWorktree
                | &CompareTarget::HeadToIndex
        );
        let can_edit_worktree = has_file_path && compare_target_new_is_worktree(&compare_target);
        let show_worktree_editor = self.show_diff_worktree_editor && can_edit_worktree;
        let git_available = self.git_available;
        let can_refresh = git_available && !self.loading;
        let can_stage = git_available
            && has_file_path
            && (is_untracked_status(&file_status)
                || status_xy(&file_status).is_some_and(|(_, y)| is_modified_status_code(y)));
        let can_unstage = git_available
            && has_file_path
            && status_xy(&file_status).is_some_and(|(x, _)| is_modified_status_code(x));
        let is_rename_or_copy = status_xy(&file_status)
            .is_some_and(|(x, y)| matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C'));
        let can_rollback_file = git_available
            && has_file_path
            && has_file_status
            && is_default_compare
            && !self.loading
            && !is_conflict_status(&file_status)
            && !is_rename_or_copy;
        let has_hunks = hunk_count > 0;
        let can_stage_hunk = git_available
            && has_file_path
            && has_hunks
            && !is_untracked_status(&file_status)
            && !is_rename_or_copy
            && compare_target == CompareTarget::IndexToWorktree;
        let can_revert_hunk = can_stage_hunk;
        let can_unstage_hunk = git_available
            && has_file_path
            && has_hunks
            && !is_rename_or_copy
            && compare_target == CompareTarget::HeadToIndex;
        let selection_count = diff_view.selected_rows.len();
        let has_selection = selection_count > 0;
        let can_stage_selection = git_available
            && has_file_path
            && has_selection
            && !is_untracked_status(&file_status)
            && !is_rename_or_copy
            && compare_target == CompareTarget::IndexToWorktree;
        let can_unstage_selection = git_available
            && has_file_path
            && has_selection
            && !is_untracked_status(&file_status)
            && !is_rename_or_copy
            && compare_target == CompareTarget::HeadToIndex;
        let can_revert_selection = git_available
            && has_file_path
            && has_selection
            && !is_untracked_status(&file_status)
            && !is_rename_or_copy
            && compare_target == CompareTarget::IndexToWorktree;
        let rows_len = diff_view.rows.len();
        let scroll_handle = diff_view.scroll_handle.clone();
        let scroll_state = diff_view.scroll_state.clone();
        let row_height = window.line_height() + px(4.);
        let item_sizes = diff_view.item_sizes(row_height);

        let app = cx.entity();

        let can_expand_all = diff_view
            .rows
            .iter()
            .any(|row| matches!(row, DisplayRow::Fold { .. }));

        let diff_find_open = self.diff_find_open;
        let diff_find_total = self.diff_find_matches.len();
        let diff_find_position = if diff_find_total == 0 {
            "0/0".to_string()
        } else {
            format!(
                "{}/{}",
                self.diff_find_selected
                    .min(diff_find_total.saturating_sub(1))
                    + 1,
                diff_find_total
            )
        };
        let can_prev_match = diff_find_total > 0;
        let can_next_match = diff_find_total > 0;

        let stage_file_label: SharedString = if can_stage {
            "Stage 文件".into()
        } else if !git_available {
            "Stage 文件（git 不可用）".into()
        } else if !has_file_path {
            "Stage 文件（demo 不支持）".into()
        } else {
            "Stage 文件（无可暂存变更）".into()
        };
        let unstage_file_label: SharedString = if can_unstage {
            "Unstage 文件".into()
        } else if !git_available {
            "Unstage 文件（git 不可用）".into()
        } else if !has_file_path {
            "Unstage 文件（demo 不支持）".into()
        } else {
            "Unstage 文件（无可取消暂存）".into()
        };
        let rollback_file_label: SharedString = if can_rollback_file {
            "Rollback 文件…".into()
        } else if !git_available {
            "Rollback 文件（git 不可用）".into()
        } else if !has_file_path {
            "Rollback 文件（demo 不支持）".into()
        } else if !is_default_compare {
            "Rollback 文件（历史/任意 ref 对比不可用）".into()
        } else if is_rename_or_copy {
            "Rollback 文件（rename/copy 暂不支持）".into()
        } else if !has_file_status {
            "Rollback 文件（状态未知）".into()
        } else {
            "Rollback 文件（不可用）".into()
        };

        let stage_hunk_label: SharedString = if can_stage_hunk {
            "Stage 当前 hunk".into()
        } else if !git_available {
            "Stage 当前 hunk（git 不可用）".into()
        } else if !has_file_path {
            "Stage 当前 hunk（demo 不支持）".into()
        } else if !has_hunks {
            "Stage 当前 hunk（无 hunk）".into()
        } else if is_rename_or_copy {
            "Stage 当前 hunk（rename/copy 暂不支持）".into()
        } else if compare_target != CompareTarget::IndexToWorktree {
            "Stage 当前 hunk（切到 暂存↔工作区）".into()
        } else {
            "Stage 当前 hunk（不可用）".into()
        };

        let unstage_hunk_label: SharedString = if can_unstage_hunk {
            "Unstage 当前 hunk".into()
        } else if !git_available {
            "Unstage 当前 hunk（git 不可用）".into()
        } else if !has_file_path {
            "Unstage 当前 hunk（demo 不支持）".into()
        } else if !has_hunks {
            "Unstage 当前 hunk（无 hunk）".into()
        } else if is_rename_or_copy {
            "Unstage 当前 hunk（rename/copy 暂不支持）".into()
        } else if compare_target != CompareTarget::HeadToIndex {
            "Unstage 当前 hunk（切到 HEAD↔暂存）".into()
        } else {
            "Unstage 当前 hunk（不可用）".into()
        };

        let revert_hunk_label: SharedString = if can_revert_hunk {
            "Revert 当前 hunk".into()
        } else if !git_available {
            "Revert 当前 hunk（git 不可用）".into()
        } else if !has_file_path {
            "Revert 当前 hunk（demo 不支持）".into()
        } else if !has_hunks {
            "Revert 当前 hunk（无 hunk）".into()
        } else if is_rename_or_copy {
            "Revert 当前 hunk（rename/copy 暂不支持）".into()
        } else if compare_target != CompareTarget::IndexToWorktree {
            "Revert 当前 hunk（切到 暂存↔工作区）".into()
        } else {
            "Revert 当前 hunk（不可用）".into()
        };

        let stage_selection_label: SharedString = if can_stage_selection {
            format!("Stage 选中行（{selection_count}）").into()
        } else if !git_available {
            "Stage 选中行（git 不可用）".into()
        } else if !has_file_path {
            "Stage 选中行（demo 不支持）".into()
        } else if !has_selection {
            "Stage 选中行（未选择）".into()
        } else if is_untracked_status(&file_status) {
            "Stage 选中行（untracked 不支持）".into()
        } else if is_rename_or_copy {
            "Stage 选中行（rename/copy 暂不支持）".into()
        } else if compare_target != CompareTarget::IndexToWorktree {
            "Stage 选中行（切到 暂存↔工作区）".into()
        } else {
            "Stage 选中行（不可用）".into()
        };

        let unstage_selection_label: SharedString = if can_unstage_selection {
            format!("Unstage 选中行（{selection_count}）").into()
        } else if !git_available {
            "Unstage 选中行（git 不可用）".into()
        } else if !has_file_path {
            "Unstage 选中行（demo 不支持）".into()
        } else if !has_selection {
            "Unstage 选中行（未选择）".into()
        } else if is_untracked_status(&file_status) {
            "Unstage 选中行（untracked 不支持）".into()
        } else if is_rename_or_copy {
            "Unstage 选中行（rename/copy 暂不支持）".into()
        } else if compare_target != CompareTarget::HeadToIndex {
            "Unstage 选中行（切到 HEAD↔暂存）".into()
        } else {
            "Unstage 选中行（不可用）".into()
        };

        let revert_selection_label: SharedString = if can_revert_selection {
            format!("Revert 选中行（{selection_count}）").into()
        } else if !git_available {
            "Revert 选中行（git 不可用）".into()
        } else if !has_file_path {
            "Revert 选中行（demo 不支持）".into()
        } else if !has_selection {
            "Revert 选中行（未选择）".into()
        } else if is_untracked_status(&file_status) {
            "Revert 选中行（untracked 不支持）".into()
        } else if is_rename_or_copy {
            "Revert 选中行（rename/copy 暂不支持）".into()
        } else if compare_target != CompareTarget::IndexToWorktree {
            "Revert 选中行（切到 暂存↔工作区）".into()
        } else {
            "Revert 选中行（不可用）".into()
        };

        let shortcuts_message: SharedString = concat!(
            "快捷键：\n",
            "Esc 返回\n",
            "Alt+N / Alt+P 下一/上一 hunk\n",
            "Alt+V 切换 Side-by-side/Unified\n",
            "Alt+L 切换对齐/分栏\n",
            "Alt+W 忽略空白\n",
            "Alt+E 展开全部\n",
        )
        .into();

        let stage_selection_label_for_menu = stage_selection_label.clone();
        let unstage_selection_label_for_menu = unstage_selection_label.clone();
        let revert_selection_label_for_menu = revert_selection_label.clone();

        let rollback_message: SharedString = if is_untracked_status(&file_status) {
            "将从磁盘删除该未跟踪文件。".into()
        } else if file_status.contains('A') {
            "将删除该新文件，并移除暂存区记录（如有）。".into()
        } else {
            "将把该文件恢复到 HEAD 版本（同时丢弃暂存与工作区改动）。".into()
        };
        let rollback_ok_text: SharedString = if is_untracked_status(&file_status) {
            "Delete".into()
        } else {
            "Rollback".into()
        };

        let app_for_rollback_file = app.clone();
        let path_for_rollback_file = diff_view.path.clone();
        let status_for_rollback_file = file_status.clone();
        let rollback_message_for_dialog = rollback_message.clone();
        let rollback_ok_text_for_dialog = rollback_ok_text.clone();
        let rollback_file = Rc::new(move |window: &mut Window, cx: &mut App| {
            let Some(path_for_ok) = path_for_rollback_file.clone() else {
                return;
            };
            let app_for_ok_outer = app_for_rollback_file.clone();
            let status_for_ok = status_for_rollback_file.clone();
            let message = rollback_message_for_dialog.clone();
            let ok_text = rollback_ok_text_for_dialog.clone();

            window.open_dialog(cx, move |dialog, _window, cx| {
                let theme = cx.theme();
                let path_label = path_for_ok.clone();
                let status_label = status_for_ok.clone();
                let action_path = path_for_ok.clone();
                let action_status = status_for_ok.clone();
                let app_for_ok = app_for_ok_outer.clone();
                dialog
                    .confirm()
                    .title(div().text_sm().font_semibold().child("Rollback changes?"))
                    .button_props(
                        DialogButtonProps::default()
                            .ok_text(ok_text.clone())
                            .ok_variant(ButtonVariant::Danger)
                            .cancel_text("Cancel"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .p(px(12.))
                            .child(
                                div()
                                    .text_sm()
                                    .font_family(theme.mono_font_family.clone())
                                    .child(format!("{status_label} {path_label}")),
                            )
                            .child(div().text_sm().child(message.clone()))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("此操作不可撤销。"),
                            ),
                    )
                    .on_ok({
                        let app_for_ok = app_for_ok.clone();
                        let action_path = action_path.clone();
                        let action_status = action_status.clone();
                        move |_, window, cx| {
                            app_for_ok.update(cx, |this, cx| {
                                this.rollback_path(
                                    action_path.clone(),
                                    action_status.clone(),
                                    window,
                                    cx,
                                );
                                cx.notify();
                            });
                            true
                        }
                    })
            });
        });

        let file_action_label: SharedString = if has_file_path {
            "在文件管理器中显示".into()
        } else {
            "在文件管理器中显示（demo 不支持）".into()
        };
        let copy_rel_label: SharedString = if has_file_path {
            "复制相对路径".into()
        } else {
            "复制相对路径（demo 不支持）".into()
        };
        let copy_abs_label: SharedString = if has_file_path {
            "复制绝对路径".into()
        } else {
            "复制绝对路径（demo 不支持）".into()
        };
        let app_for_reveal = app.clone();
        let repo_root_for_paths = self.repo_root.clone();
        let path_for_reveal = diff_view.path.clone();
        let reveal_file = Rc::new(move |window: &mut Window, cx: &mut App| {
            let Some(path) = path_for_reveal.clone() else {
                return;
            };
            let abs = repo_root_for_paths.join(&path);
            app_for_reveal.update(cx, |this, cx| {
                this.reveal_in_file_manager(abs, window, cx);
            });
        });

        let path_for_copy_rel = diff_view.path.clone();
        let copy_rel = Rc::new(move |_window: &mut Window, cx: &mut App| {
            let Some(path) = path_for_copy_rel.as_ref() else {
                return;
            };
            cx.write_to_clipboard(ClipboardItem::new_string(path.clone()));
        });

        let repo_root_for_abs = self.repo_root.clone();
        let path_for_copy_abs = diff_view.path.clone();
        let copy_abs = Rc::new(move |_window: &mut Window, cx: &mut App| {
            let Some(path) = path_for_copy_abs.as_ref() else {
                return;
            };
            let abs = repo_root_for_abs.join(path).display().to_string();
            cx.write_to_clipboard(ClipboardItem::new_string(abs));
        });

        let more_menu = {
            let app_for_menu = app.clone();
            Popover::new("diff-more-menu")
                .appearance(false)
                .trigger(
                    Button::new("diff-more-trigger")
                        .icon(IconName::Ellipsis)
                        .ghost()
                        .small()
                        .compact()
                        .tooltip("更多")
                        .on_click(|_, _, _| {}),
                )
                .content(move |_, _window, cx| {
                    let theme = cx.theme();
                    let popover = cx.entity();

                    let make_action = move |id: &'static str,
                                            label: SharedString,
                                            disabled: bool,
                                            action: Rc<dyn Fn(&mut Window, &mut App) + 'static>| {
                        let popover = popover.clone();
                        let label_for_click = label.clone();
                        div()
                            .id(id)
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .h(px(28.))
                            .px(px(8.))
                            .rounded(px(6.))
                            .text_sm()
                            .min_w(px(0.))
                            .when(disabled, |this| {
                                this.text_color(theme.muted_foreground)
                                    .cursor_default()
                                    .child(label_for_click.clone())
                            })
                            .when(!disabled, |this| {
                                this.text_color(theme.popover_foreground)
                                    .cursor_pointer()
                                    .hover(|this| {
                                        this.bg(theme.accent.alpha(0.4))
                                            .text_color(theme.accent_foreground)
                                    })
                                    .active(|this| {
                                        this.bg(theme.accent)
                                            .text_color(theme.accent_foreground)
                                    })
                                    .on_mouse_down(MouseButton::Left, {
                                        let action = action.clone();
                                        move |_, window, cx| {
                                            window.prevent_default();
                                            action(window, cx);
                                            popover.update(cx, |state, cx| {
                                                state.dismiss(window, cx)
                                            });
                                        }
                                    })
                                    .child(label_for_click.clone())
                            })
                            .into_any_element()
                    };

                    let view_toggle_split_label: SharedString = if inline_mode {
                        "分栏（Inline 模式不可用）".into()
                    } else if two_pane {
                        "分栏: 开".into()
                    } else {
                        "分栏: 关".into()
                    };
                    let view_toggle_ws_label: SharedString = if ignore_whitespace {
                        "忽略空白: 开".into()
                    } else {
                        "忽略空白: 关".into()
                    };

                    let app_for_split = app_for_menu.clone();
                    let toggle_split = Rc::new(move |_window: &mut Window, cx: &mut App| {
                        app_for_split.update(cx, |this, cx| {
                            if this.view_mode == DiffViewMode::Inline {
                                return;
                            }
                            this.split_layout = match this.split_layout {
                                SplitLayout::Aligned => SplitLayout::TwoPane,
                                SplitLayout::TwoPane => SplitLayout::Aligned,
                            };
                            cx.notify();
                        });
                    });

                    let app_for_ws = app_for_menu.clone();
                    let toggle_ws = Rc::new(move |window: &mut Window, cx: &mut App| {
                        app_for_ws.update(cx, |this, cx| {
                            let next = !this.diff_options.ignore_whitespace;
                            this.set_ignore_whitespace(next, window, cx);
                            cx.notify();
                        });
                    });

                    let app_for_expand = app_for_menu.clone();
                    let expand_all = Rc::new(move |_window: &mut Window, cx: &mut App| {
                        app_for_expand.update(cx, |this, cx| {
                            this.expand_all_folds();
                            cx.notify();
                        });
                    });

                    let shortcuts_message = shortcuts_message.clone();
                    let show_shortcuts = Rc::new(move |window: &mut Window, cx: &mut App| {
                        window.push_notification(
                            Notification::new().message(shortcuts_message.to_string()),
                            cx,
                        );
                    });

                    let app_for_stage_file = app_for_menu.clone();
                    let stage_file = Rc::new(move |window: &mut Window, cx: &mut App| {
                        app_for_stage_file.update(cx, |this, cx| {
                            this.stage_current_file(window, cx);
                            cx.notify();
                        });
                    });

                    let app_for_unstage_file = app_for_menu.clone();
                    let unstage_file = Rc::new(move |window: &mut Window, cx: &mut App| {
                        app_for_unstage_file.update(cx, |this, cx| {
                            this.unstage_current_file(window, cx);
                            cx.notify();
                        });
                    });

                    let app_for_stage_hunk = app_for_menu.clone();
                    let stage_hunk = Rc::new(move |window: &mut Window, cx: &mut App| {
                        app_for_stage_hunk.update(cx, |this, cx| {
                            this.stage_current_hunk(window, cx);
                            cx.notify();
                        });
                    });

                    let app_for_unstage_hunk = app_for_menu.clone();
                    let unstage_hunk = Rc::new(move |window: &mut Window, cx: &mut App| {
                        app_for_unstage_hunk.update(cx, |this, cx| {
                            this.unstage_current_hunk(window, cx);
                            cx.notify();
                        });
                    });

                    let app_for_revert_hunk = app_for_menu.clone();
                    let revert_hunk = Rc::new(move |window: &mut Window, cx: &mut App| {
                        app_for_revert_hunk.update(cx, |this, cx| {
                            this.revert_current_hunk(window, cx);
                            cx.notify();
                        });
                    });

                    let app_for_stage_selection = app_for_menu.clone();
                    let stage_selection = Rc::new(move |window: &mut Window, cx: &mut App| {
                        app_for_stage_selection.update(cx, |this, cx| {
                            this.stage_selected_rows(window, cx);
                            cx.notify();
                        });
                    });

                    let app_for_unstage_selection = app_for_menu.clone();
                    let unstage_selection = Rc::new(move |window: &mut Window, cx: &mut App| {
                        app_for_unstage_selection.update(cx, |this, cx| {
                            this.unstage_selected_rows(window, cx);
                            cx.notify();
                        });
                    });

                    let app_for_revert_selection = app_for_menu.clone();
                    let revert_selection = Rc::new(move |window: &mut Window, cx: &mut App| {
                        app_for_revert_selection.update(cx, |this, cx| {
                            this.revert_selected_rows(window, cx);
                            cx.notify();
                        });
                    });

                    let can_toggle_split = !inline_mode;

                    div()
                        .p(px(6.))
                        .w(px(280.))
                        .bg(theme.popover)
                        .border_1()
                        .border_color(theme.border)
                        .rounded(theme.radius)
                        .shadow_md()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .px(px(6.))
                                .py(px(4.))
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("视图"),
                        )
                        .child(make_action(
                            "diff-more-toggle-split",
                            view_toggle_split_label,
                            !can_toggle_split,
                            toggle_split,
                        ))
                        .child(make_action(
                            "diff-more-toggle-ws",
                            view_toggle_ws_label,
                            false,
                            toggle_ws,
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .gap(px(8.))
                                .h(px(28.))
                                .px(px(8.))
                                .rounded(px(6.))
                                .hover(|this| this.bg(theme.accent.alpha(0.15)))
                                .child(div().text_sm().child(format!("上下文 {context_lines}")))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(2.))
                                        .child(
                                            Button::new("diff-more-context-dec")
                                                .icon(IconName::Minus)
                                                .ghost()
                                                .xsmall()
                                                .compact()
                                                .disabled(context_lines == 0)
                                                .on_click({
                                                    let app = app_for_menu.clone();
                                                    move |_, window, cx| {
                                                        app.update(cx, |this, cx| {
                                                            let next = this
                                                                .diff_options
                                                                .context_lines
                                                                .saturating_sub(1);
                                                            this.set_context_lines(
                                                                next, window, cx,
                                                            );
                                                            cx.notify();
                                                        });
                                                    }
                                                }),
                                        )
                                        .child(
                                            Button::new("diff-more-context-inc")
                                                .icon(IconName::Plus)
                                                .ghost()
                                                .xsmall()
                                                .compact()
                                                .disabled(context_lines >= MAX_CONTEXT_LINES)
                                                .on_click({
                                                    let app = app_for_menu.clone();
                                                    move |_, window, cx| {
                                                        app.update(cx, |this, cx| {
                                                            let next =
                                                                this.diff_options.context_lines + 1;
                                                            this.set_context_lines(
                                                                next, window, cx,
                                                            );
                                                            cx.notify();
                                                        });
                                                    }
                                                }),
                                        ),
                                ),
                        )
                        .child(make_action(
                            "diff-more-expand-all",
                            "展开全部".into(),
                            !can_expand_all,
                            expand_all,
                        ))
                        .child(make_action(
                            "diff-more-shortcuts",
                            "快捷键…".into(),
                            false,
                            show_shortcuts,
                        ))
                        .child(div().h(px(1.)).bg(theme.border.alpha(0.4)))
                        .child(
                            div()
                                .px(px(6.))
                                .py(px(4.))
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("Git"),
                        )
                        .child(make_action(
                            "diff-more-stage-file",
                            stage_file_label.clone(),
                            !can_stage,
                            stage_file,
                        ))
                        .child(make_action(
                            "diff-more-unstage-file",
                            unstage_file_label.clone(),
                            !can_unstage,
                            unstage_file,
                        ))
                        .child(make_action(
                            "diff-more-rollback-file",
                            rollback_file_label.clone(),
                            !can_rollback_file,
                            rollback_file.clone(),
                        ))
                        .child(make_action(
                            "diff-more-stage-hunk",
                            stage_hunk_label.clone(),
                            !can_stage_hunk,
                            stage_hunk,
                        ))
                        .child(make_action(
                            "diff-more-unstage-hunk",
                            unstage_hunk_label.clone(),
                            !can_unstage_hunk,
                            unstage_hunk,
                        ))
                        .child(make_action(
                            "diff-more-revert-hunk",
                            revert_hunk_label.clone(),
                            !can_revert_hunk,
                            revert_hunk,
                        ))
                        .child(make_action(
                            "diff-more-stage-selection",
                            stage_selection_label_for_menu.clone(),
                            !can_stage_selection,
                            stage_selection,
                        ))
                        .child(make_action(
                            "diff-more-unstage-selection",
                            unstage_selection_label_for_menu.clone(),
                            !can_unstage_selection,
                            unstage_selection,
                        ))
                        .child(make_action(
                            "diff-more-revert-selection",
                            revert_selection_label_for_menu.clone(),
                            !can_revert_selection,
                            revert_selection,
                        ))
                        .child(div().h(px(1.)).bg(theme.border.alpha(0.4)))
                        .child(
                            div()
                                .px(px(6.))
                                .py(px(4.))
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("File"),
                        )
                        .child(make_action(
                            "diff-more-reveal-file",
                            file_action_label.clone(),
                            !has_file_path,
                            reveal_file.clone(),
                        ))
                        .child(make_action(
                            "diff-more-copy-rel",
                            copy_rel_label.clone(),
                            !has_file_path,
                            copy_rel.clone(),
                        ))
                        .child(make_action(
                            "diff-more-copy-abs",
                            copy_abs_label.clone(),
                            !has_file_path,
                            copy_abs.clone(),
                        ))
                })
        };

        let toolbar = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(12.))
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(div().flex_1().min_w(px(0.)).truncate().child(title)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .flex_wrap()
                    .child(
                        AnimatedSegmented::new("diff-view-mode-tabs")
                            .small()
                            .w(px(76.))
                            .selected_index(if view_mode == DiffViewMode::Split {
                                0
                            } else {
                                1
                            })
                            .on_click({
                                let app = app.clone();
                                move |ix, _window, cx| {
                                    let next = if *ix == 0 {
                                        DiffViewMode::Split
                                    } else {
                                        DiffViewMode::Inline
                                    };
                                    app.update(cx, |this, cx| {
                                        this.set_view_mode(next);
                                        cx.notify();
                                    });
                                }
                            })
                            .items(vec![
                                AnimatedSegmentedItem::new()
                                    .icon(
                                        div()
                                            .w(px(18.))
                                            .h(px(18.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(Icon::new(PlateIconName::Columns2).xsmall()),
                                    )
                                    .tooltip("Side-by-side"),
                                AnimatedSegmentedItem::new()
                                    .icon(
                                        div()
                                            .w(px(18.))
                                            .h(px(18.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                Icon::new(PlateIconName::GalleryHorizontal)
                                                    .xsmall(),
                                            ),
                                    )
                                    .tooltip("Unified"),
                            ]),
                    )
                    .child(
                        Button::new("prev-file")
                            .icon(PlateIconName::ArrowUpToLine)
                            .ghost()
                            .small()
                            .compact()
                            .disabled(!can_prev_file)
                            .tooltip("Prev file")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_adjacent_file(-1, window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("next-file")
                            .icon(PlateIconName::ArrowDownToLine)
                            .ghost()
                            .small()
                            .compact()
                            .disabled(!can_next_file)
                            .tooltip("Next file")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_adjacent_file(1, window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("prev-hunk")
                            .icon(IconName::ChevronUp)
                            .ghost()
                            .small()
                            .compact()
                            .tooltip_with_action("上一 hunk", &Prev, Some(CONTEXT))
                            .disabled(!can_prev_hunk)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.jump_hunk(-1);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("next-hunk")
                            .icon(IconName::ChevronDown)
                            .ghost()
                            .small()
                            .compact()
                            .tooltip_with_action("下一 hunk", &Next, Some(CONTEXT))
                            .disabled(!can_next_hunk)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.jump_hunk(1);
                                cx.notify();
                            })),
                    )
                    .child({
                        let button = Button::new("diff-find-toggle")
                            .icon(IconName::Search)
                            .small()
                            .compact()
                            .tooltip_with_action("查找", &ToggleDiffFind, Some(CONTEXT))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_diff_find(window, cx);
                            }));

                        if diff_find_open {
                            button.primary()
                        } else {
                            button.ghost()
                        }
                    })
                    .child({
                        let label: SharedString = if can_edit_worktree {
                            if show_worktree_editor {
                                "关闭工作区编辑器".into()
                            } else {
                                "打开工作区编辑器".into()
                            }
                        } else if !has_file_path {
                            "工作区编辑器（demo 不支持）".into()
                        } else {
                            "工作区编辑器（当前对比不包含工作区）".into()
                        };

                        let icon = if show_worktree_editor {
                            IconName::PanelBottomOpen
                        } else {
                            IconName::PanelBottom
                        };

                        let button = Button::new("diff-worktree-editor-toggle")
                            .icon(icon)
                            .small()
                            .compact()
                            .tooltip(label)
                            .disabled(!can_edit_worktree)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_diff_worktree_editor(window, cx);
                            }));

                        if show_worktree_editor {
                            button.primary()
                        } else {
                            button.ghost()
                        }
                    })
                    .child(
                        Button::new("diff-refresh")
                            .icon(PlateIconName::RefreshCw)
                            .ghost()
                            .small()
                            .compact()
                            .disabled(!can_refresh)
                            .tooltip("Refresh this")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.refresh_current_file(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(more_menu),
            );

        let search_bar = if diff_find_open {
            let theme = cx.theme();
            Some(
                div()
                    .id("diff-find-bar")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .on_key_down({
                        let app = app.clone();
                        move |event, window, cx| {
                            let handled =
                                app.update(cx, |this, cx| match event.keystroke.key.as_str() {
                                    "escape" => {
                                        this.close_diff_find(window, cx);
                                        true
                                    }
                                    "enter" => {
                                        let delta = if event.keystroke.modifiers.shift {
                                            -1
                                        } else {
                                            1
                                        };
                                        this.jump_diff_find_match(delta, window, cx);
                                        true
                                    }
                                    "down" => {
                                        this.jump_diff_find_match(1, window, cx);
                                        true
                                    }
                                    "up" => {
                                        this.jump_diff_find_match(-1, window, cx);
                                        true
                                    }
                                    _ => false,
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
                            .gap(px(8.))
                            .flex_1()
                            .min_w(px(0.))
                            .child(
                                Input::new(&self.diff_find_input)
                                    .w_full()
                                    .prefix(Icon::new(IconName::Search).xsmall()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(diff_find_position.clone()),
                            ),
                    )
                    .child(
                        Button::new("diff-find-prev")
                            .icon(Icon::new(IconName::ChevronUp).xsmall())
                            .ghost()
                            .compact()
                            .disabled(!can_prev_match)
                            .tooltip("上一处匹配 (↑/Shift+Enter)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.jump_diff_find_match(-1, window, cx);
                            })),
                    )
                    .child(
                        Button::new("diff-find-next")
                            .icon(Icon::new(IconName::ChevronDown).xsmall())
                            .ghost()
                            .compact()
                            .disabled(!can_next_match)
                            .tooltip("下一处匹配 (↓/Enter)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.jump_diff_find_match(1, window, cx);
                            })),
                    )
                    .child(
                        Button::new("diff-find-close")
                            .icon(Icon::new(IconName::Close).xsmall())
                            .ghost()
                            .compact()
                            .tooltip("关闭 (Esc)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_diff_find(window, cx);
                            })),
                    )
                    .into_any_element(),
            )
        } else {
            None
        };

        let toolbar = toolbar.when_some(search_bar, |this, bar| this.child(bar));

        let list = match view_mode {
            DiffViewMode::Inline => v_virtual_list(
                cx.entity(),
                "diff-inline-list",
                item_sizes,
                move |this, visible_range, window, cx| {
                    visible_range
                        .map(|index| this.render_inline_row(index, row_height, window, cx))
                        .collect::<Vec<_>>()
                },
            )
            .track_scroll(&scroll_handle)
            .into_any_element(),
            DiffViewMode::Split => match self.split_layout {
                SplitLayout::Aligned => v_virtual_list(
                    cx.entity(),
                    "diff-aligned-list",
                    item_sizes,
                    move |this, visible_range, window, cx| {
                        visible_range
                            .map(|index| this.render_demo_row(index, row_height, window, cx))
                            .collect::<Vec<_>>()
                    },
                )
                .track_scroll(&scroll_handle)
                .into_any_element(),
                SplitLayout::TwoPane => {
                    let old_list = v_virtual_list(
                        cx.entity(),
                        "diff-two-pane-old",
                        item_sizes.clone(),
                        move |this, visible_range, window, cx| {
                            visible_range
                                .map(|index| {
                                    this.render_pane_row(Side::Old, index, row_height, window, cx)
                                })
                                .collect::<Vec<_>>()
                        },
                    )
                    .track_scroll(&scroll_handle)
                    .into_any_element();

                    let new_list = v_virtual_list(
                        cx.entity(),
                        "diff-two-pane-new",
                        item_sizes,
                        move |this, visible_range, window, cx| {
                            visible_range
                                .map(|index| {
                                    this.render_pane_row(Side::New, index, row_height, window, cx)
                                })
                                .collect::<Vec<_>>()
                        },
                    )
                    .track_scroll(&scroll_handle)
                    .into_any_element();

                    let theme = cx.theme();
                    let pane_header_height = window.line_height() + px(6.);
                    let old_header = div()
                        .h(pane_header_height)
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .bg(theme.muted.alpha(0.2))
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(compare_target_side_label(&compare_target, Side::Old));
                    let new_header = div()
                        .h(pane_header_height)
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .bg(theme.muted.alpha(0.2))
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(compare_target_side_label(&compare_target, Side::New));

                    div()
                        .flex()
                        .flex_row()
                        .size_full()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w(px(0.))
                                .border_r_1()
                                .border_color(theme.border.alpha(0.6))
                                .child(old_header)
                                .child(div().flex_1().min_h(px(0.)).child(old_list)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w(px(0.))
                                .child(new_header)
                                .child(div().flex_1().min_h(px(0.)).child(new_list)),
                        )
                        .into_any_element()
                }
            },
        };

        let hunk_position = if hunk_count == 0 {
            "0/0".to_string()
        } else {
            format!("{}/{}", diff_view.current_hunk + 1, hunk_count)
        };
        let timing_label = if self.timing_enabled {
            self.last_diff_load_timing.map(|timing| {
                format!(
                    "load: {}ms (io {} + diff {})",
                    timing.total_ms, timing.io_ms, timing.diff_ms
                )
            })
        } else {
            None
        };
        let status_left = match view_mode {
            DiffViewMode::Split => div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .child("视图: Side-by-side")
                .child(format!("布局: {}", if two_pane { "分栏" } else { "对齐" }))
                .child(div().truncate().child(format!("文件: {}", diff_view.title)))
                .child(format!("对比: {}", compare_target_label(&compare_target)))
                .child(format!("上下文: {context_lines}"))
                .child(format!(
                    "忽略空白: {}",
                    if ignore_whitespace { "开" } else { "关" }
                )),
            DiffViewMode::Inline => div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .child("视图: Unified")
                .child(div().truncate().child(format!("文件: {}", diff_view.title)))
                .child(format!("对比: {}", compare_target_label(&compare_target)))
                .child(format!("上下文: {context_lines}"))
                .child(format!(
                    "忽略空白: {}",
                    if ignore_whitespace { "开" } else { "关" }
                )),
        };
        let status_right = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .child(format!("hunk: {hunk_position}"))
            .when_some(timing_label, |this, label| this.child(label))
            .child(
                div()
                    .truncate()
                    .text_color(cx.theme().muted_foreground)
                    .child("Esc 返回 · Alt+N/P 导航 · Alt+W 空白 · Alt+V 视图"),
            );
        let status_bar = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px(px(12.))
            .py(px(6.))
            .border_t_1()
            .border_color(cx.theme().border.alpha(0.6))
            .bg(cx.theme().muted.alpha(0.12))
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(status_left)
            .child(status_right);

        let diff_scroll_ruler = if rows_len > 0 && hunk_count > 0 {
            let theme = cx.theme();
            let total_rows = rows_len.max(1) as f32;
            let right_inset = px(14.);
            let width = px(8.);
            let max_markers = 200usize;
            let marker_step = (hunk_count / max_markers).max(1);
            let mut marker_indices: Vec<usize> = (0..hunk_count).step_by(marker_step).collect();
            marker_indices.push(diff_view.current_hunk);
            marker_indices.sort_unstable();
            marker_indices.dedup();

            let mut ruler = div()
                .id("diff-scroll-ruler")
                .absolute()
                .top(px(6.))
                .bottom(px(6.))
                .right(right_inset)
                .w(width)
                .rounded(px(999.))
                .bg(theme.muted.alpha(0.1))
                .border_1()
                .border_color(theme.border.alpha(0.35))
                .relative();

            for hunk_index in marker_indices {
                let row_index = diff_view.hunk_rows.get(hunk_index).copied().unwrap_or(0);
                let fraction = (row_index as f32 / total_rows).clamp(0.0, 1.0);
                let is_current = hunk_index == diff_view.current_hunk;
                let color = if is_current {
                    theme.blue
                } else {
                    theme.blue.alpha(0.65)
                };
                let height = if is_current { px(6.) } else { px(3.) };
                let radius = if is_current { px(6.) } else { px(4.) };

                ruler = ruler.child(
                    div()
                        .id(("diff-ruler-marker", hunk_index))
                        .absolute()
                        .top(relative(fraction))
                        .left(px(1.))
                        .right(px(1.))
                        .h(height)
                        .rounded(radius)
                        .bg(color)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _window, cx| {
                                if let Some(diff_view) = this.diff_view.as_mut() {
                                    if hunk_index < diff_view.hunk_rows.len() {
                                        diff_view.current_hunk = hunk_index;
                                        diff_view
                                            .scroll_handle
                                            .scroll_to_item(row_index, ScrollStrategy::Top);
                                    }
                                }
                                cx.notify();
                            }),
                        ),
                );
            }

            Some(ruler)
        } else {
            None
        };

        let mut viewport = div()
            .flex()
            .flex_col()
            .flex_1()
            .relative()
            .overflow_hidden()
            .child(list)
            .child(Scrollbar::uniform_scroll(&scroll_state, &scroll_handle));

        if let Some(ruler) = diff_scroll_ruler {
            viewport = viewport.child(ruler);
        }

        let body: AnyElement = if show_worktree_editor {
            let editor_panel = div()
                .id("diff-worktree-editor-panel")
                .flex()
                .flex_col()
                .size_full()
                .min_h(px(0.))
                .bg(cx.theme().muted.alpha(0.06))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .px(px(12.))
                        .py(px(8.))
                        .child(div().text_sm().child("工作区编辑（可保存）"))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    Button::new("diff-worktree-editor-apply")
                                        .icon(Icon::new(IconName::Check).xsmall())
                                        .ghost()
                                        .compact()
                                        .tooltip("应用到 Diff（不写入磁盘）")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.apply_diff_worktree_editor_preview(window, cx);
                                        })),
                                )
                                .child(
                                    Button::new("diff-worktree-editor-reload")
                                        .icon(Icon::new(IconName::Replace).xsmall())
                                        .ghost()
                                        .compact()
                                        .tooltip("从磁盘重载")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.reload_diff_worktree_editor_from_disk(window, cx);
                                        })),
                                )
                                .child(
                                    Button::new("diff-worktree-editor-save")
                                        .icon(Icon::new(PlateIconName::FileUp).xsmall())
                                        .ghost()
                                        .compact()
                                        .tooltip("保存到文件（并刷新）")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.save_diff_worktree_editor_to_disk(window, cx);
                                        })),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h(px(0.))
                        .p(px(12.))
                        .child(
                            Input::new(&self.diff_worktree_editor_input)
                                .h_full()
                                .w_full(),
                        ),
                );

            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .child(
                    v_resizable("diff-worktree-editor-split")
                        .child(resizable_panel().child(viewport.size_full()))
                        .child(
                            resizable_panel()
                                .size(px(240.))
                                .size_range(px(160.)..px(900.))
                                .child(editor_panel),
                        ),
                )
                .into_any_element()
        } else {
            viewport.into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(toolbar)
            .child(body)
            .child(status_bar)
    }
}

impl GitViewerApp {
    fn render_demo_row(
        &mut self,
        index: usize,
        height: Pixels,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let Some(row) = self
            .diff_view
            .as_ref()
            .and_then(|diff_view| diff_view.rows.get(index))
            .cloned()
        else {
            return div().into_any_element();
        };

        let (compare_target, file_status, has_file_path, selection_count) = self
            .diff_view
            .as_ref()
            .map(|view| {
                (
                    view.compare_target.clone(),
                    view.status.clone().unwrap_or_default(),
                    view.path.is_some(),
                    view.selected_rows.len(),
                )
            })
            .unwrap_or((CompareTarget::HeadToWorktree, String::new(), false, 0usize));
        let git_available = self.git_available;
        let app = cx.entity();

        match row {
            DisplayRow::HunkHeader { text } => {
                let hunk_index = self
                    .diff_view
                    .as_ref()
                    .and_then(|view| view.hunk_rows.binary_search(&index).ok());
                let menu = Self::diff_context_menu_builder(
                    app.clone(),
                    None,
                    hunk_index,
                    diffview::DiffRowKind::Unchanged,
                    compare_target.clone(),
                    file_status.clone(),
                    has_file_path,
                    selection_count,
                    git_available,
                );

                let is_untracked = is_untracked_status(&file_status);
                let is_rename_or_copy = status_xy(&file_status)
                    .is_some_and(|(x, y)| matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C'));
                let can_stage_hunk = git_available
                    && has_file_path
                    && hunk_index.is_some()
                    && !is_untracked
                    && !is_rename_or_copy
                    && compare_target == CompareTarget::IndexToWorktree;
                let can_unstage_hunk = git_available
                    && has_file_path
                    && hunk_index.is_some()
                    && !is_untracked
                    && !is_rename_or_copy
                    && compare_target == CompareTarget::HeadToIndex;
                let can_revert_hunk = git_available
                    && has_file_path
                    && hunk_index.is_some()
                    && !is_untracked
                    && !is_rename_or_copy
                    && compare_target == CompareTarget::IndexToWorktree;

                let stage_hunk_button = {
                    let hunk_index_for_click = hunk_index;
                    Button::new(("diff-hunk-stage", hunk_index.unwrap_or(usize::MAX)))
                        .icon(Icon::new(IconName::Plus).xsmall())
                        .ghost()
                        .compact()
                        .disabled(!can_stage_hunk)
                        .tooltip("Stage 该 hunk")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let Some(hunk_index) = hunk_index_for_click else {
                                return;
                            };
                            if let Some(view) = this.diff_view.as_mut() {
                                view.current_hunk =
                                    hunk_index.min(view.diff_model.hunks.len().saturating_sub(1));
                            }
                            this.stage_current_hunk(window, cx);
                            cx.notify();
                        }))
                };
                let unstage_hunk_button = {
                    let hunk_index_for_click = hunk_index;
                    Button::new(("diff-hunk-unstage", hunk_index.unwrap_or(usize::MAX)))
                        .icon(Icon::new(IconName::Minus).xsmall())
                        .ghost()
                        .compact()
                        .disabled(!can_unstage_hunk)
                        .tooltip("Unstage 该 hunk")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let Some(hunk_index) = hunk_index_for_click else {
                                return;
                            };
                            if let Some(view) = this.diff_view.as_mut() {
                                view.current_hunk =
                                    hunk_index.min(view.diff_model.hunks.len().saturating_sub(1));
                            }
                            this.unstage_current_hunk(window, cx);
                            cx.notify();
                        }))
                };
                let revert_hunk_button = {
                    let hunk_index_for_click = hunk_index;
                    Button::new(("diff-hunk-revert", hunk_index.unwrap_or(usize::MAX)))
                        .icon(Icon::new(PlateIconName::Undo2).xsmall())
                        .ghost()
                        .compact()
                        .disabled(!can_revert_hunk)
                        .tooltip("Revert 该 hunk")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let Some(hunk_index) = hunk_index_for_click else {
                                return;
                            };
                            if let Some(view) = this.diff_view.as_mut() {
                                view.current_hunk =
                                    hunk_index.min(view.diff_model.hunks.len().saturating_sub(1));
                            }
                            this.revert_current_hunk(window, cx);
                            cx.notify();
                        }))
                };
                let actions = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.))
                    .on_mouse_down(MouseButton::Left, |_, _window, cx| cx.stop_propagation())
                    .when(compare_target == CompareTarget::IndexToWorktree, |this| {
                        this.child(stage_hunk_button).child(revert_hunk_button)
                    })
                    .when(compare_target == CompareTarget::HeadToIndex, |this| {
                        this.child(unstage_hunk_button)
                    });

                div()
                    .h(height)
                    .px(px(12.))
                    .flex()
                    .justify_between()
                    .items_center()
                    .bg(theme.muted.alpha(0.35))
                    .hover(|this| this.bg(theme.muted.alpha(0.45)))
                    .cursor_pointer()
                    .font_family(theme.mono_font_family.clone())
                    .text_sm()
                    .child(div().flex_1().min_w(px(0.)).truncate().child(text))
                    .child(actions)
                    .on_mouse_down(MouseButton::Left, {
                        let hunk_index = hunk_index;
                        cx.listener(move |this, _, _window, cx| {
                            let Some(hunk_index) = hunk_index else {
                                return;
                            };
                            if let Some(view) = this.diff_view.as_mut() {
                                view.current_hunk =
                                    hunk_index.min(view.diff_model.hunks.len().saturating_sub(1));
                                view.scroll_handle
                                    .scroll_to_item(index, ScrollStrategy::Top);
                            }
                            cx.notify();
                        })
                    })
                    .context_menu(menu)
                    .into_any_element()
            }
            DisplayRow::Fold {
                old_start,
                new_start,
                len,
            } => {
                let label = format!(
                    "… 隐藏了 {len} 行未变更内容（old: {}..{}, new: {}..{}）点击展开",
                    old_start + 1,
                    old_start + len,
                    new_start + 1,
                    new_start + len
                );
                div()
                    .h(height)
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .bg(theme.muted.alpha(0.25))
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(label)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, cx| {
                            this.expand_fold(index);
                            if let Some(diff_view) = this.diff_view.as_ref() {
                                diff_view
                                    .scroll_handle
                                    .scroll_to_item(index, ScrollStrategy::Top);
                            }
                            cx.notify();
                        }),
                    )
                    .into_any_element()
            }
            DisplayRow::Code {
                source,
                kind,
                old_line,
                new_line,
                old_segments,
                new_segments,
            } => {
                let border = theme.border;
                let mono = theme.mono_font_family.clone();
                let highlight = self.diff_find_highlight_color(index, theme);
                let is_selected = source.is_some_and(|source| {
                    self.diff_view
                        .as_ref()
                        .is_some_and(|view| view.selected_rows.contains(&source))
                });

                let mut row = div()
                    .h(height)
                    .flex()
                    .flex_row()
                    .items_center()
                    .border_b_1()
                    .border_color(border.alpha(0.35))
                    .relative();

                if is_selected {
                    row = row
                        .child(
                            div()
                                .absolute()
                                .top(px(0.))
                                .bottom(px(0.))
                                .left(px(0.))
                                .right(px(0.))
                                .bg(theme.accent.alpha(0.1)),
                        )
                        .child(
                            div()
                                .absolute()
                                .top(px(0.))
                                .bottom(px(0.))
                                .left(px(0.))
                                .w(px(2.))
                                .bg(theme.accent),
                        );
                }

                row.child(
                    render_side(
                        Side::Old,
                        kind,
                        old_line,
                        &old_segments,
                        mono.clone(),
                        theme,
                    )
                    .when_some(highlight, |this, color| {
                        this.child(div().w(px(2.)).h_full().bg(color))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            if let Some(diff_view) = this.diff_view.as_ref() {
                                diff_view
                                    .scroll_handle
                                    .scroll_to_item(index, ScrollStrategy::Top);
                            }
                            this.handle_diff_row_click(index, event.modifiers, window, cx);
                        }),
                    ),
                )
                .child(div().w(px(1.)).h_full().bg(border.alpha(0.6)))
                .child(
                    render_side(Side::New, kind, new_line, &new_segments, mono, theme)
                        .when_some(highlight, |this, color| {
                            this.child(div().w(px(2.)).h_full().bg(color))
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                if let Some(diff_view) = this.diff_view.as_ref() {
                                    diff_view
                                        .scroll_handle
                                        .scroll_to_item(index, ScrollStrategy::Top);
                                }
                                this.handle_diff_row_click(index, event.modifiers, window, cx);
                            }),
                        ),
                )
                .context_menu(Self::diff_context_menu_builder(
                    app.clone(),
                    source,
                    source.map(|source| source.hunk_index),
                    kind,
                    compare_target.clone(),
                    file_status.clone(),
                    has_file_path,
                    selection_count,
                    git_available,
                ))
                .into_any_element()
            }
        }
    }

    fn render_inline_row(
        &mut self,
        index: usize,
        height: Pixels,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let Some(row) = self
            .diff_view
            .as_ref()
            .and_then(|diff_view| diff_view.rows.get(index))
            .cloned()
        else {
            return div().into_any_element();
        };

        let (compare_target, file_status, has_file_path, selection_count) = self
            .diff_view
            .as_ref()
            .map(|view| {
                (
                    view.compare_target.clone(),
                    view.status.clone().unwrap_or_default(),
                    view.path.is_some(),
                    view.selected_rows.len(),
                )
            })
            .unwrap_or((CompareTarget::HeadToWorktree, String::new(), false, 0usize));
        let git_available = self.git_available;
        let app = cx.entity();

        match row {
            DisplayRow::HunkHeader { text } => {
                let hunk_index = self
                    .diff_view
                    .as_ref()
                    .and_then(|view| view.hunk_rows.binary_search(&index).ok());
                let menu = Self::diff_context_menu_builder(
                    app.clone(),
                    None,
                    hunk_index,
                    diffview::DiffRowKind::Unchanged,
                    compare_target.clone(),
                    file_status.clone(),
                    has_file_path,
                    selection_count,
                    git_available,
                );

                let is_untracked = is_untracked_status(&file_status);
                let is_rename_or_copy = status_xy(&file_status)
                    .is_some_and(|(x, y)| matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C'));
                let can_stage_hunk = git_available
                    && has_file_path
                    && hunk_index.is_some()
                    && !is_untracked
                    && !is_rename_or_copy
                    && compare_target == CompareTarget::IndexToWorktree;
                let can_unstage_hunk = git_available
                    && has_file_path
                    && hunk_index.is_some()
                    && !is_untracked
                    && !is_rename_or_copy
                    && compare_target == CompareTarget::HeadToIndex;
                let can_revert_hunk = git_available
                    && has_file_path
                    && hunk_index.is_some()
                    && !is_untracked
                    && !is_rename_or_copy
                    && compare_target == CompareTarget::IndexToWorktree;

                let stage_hunk_button = {
                    let hunk_index_for_click = hunk_index;
                    Button::new(("diff-hunk-stage-inline", hunk_index.unwrap_or(usize::MAX)))
                        .icon(Icon::new(IconName::Plus).xsmall())
                        .ghost()
                        .compact()
                        .disabled(!can_stage_hunk)
                        .tooltip("Stage 该 hunk")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let Some(hunk_index) = hunk_index_for_click else {
                                return;
                            };
                            if let Some(view) = this.diff_view.as_mut() {
                                view.current_hunk =
                                    hunk_index.min(view.diff_model.hunks.len().saturating_sub(1));
                            }
                            this.stage_current_hunk(window, cx);
                            cx.notify();
                        }))
                };
                let unstage_hunk_button = {
                    let hunk_index_for_click = hunk_index;
                    Button::new(("diff-hunk-unstage-inline", hunk_index.unwrap_or(usize::MAX)))
                        .icon(Icon::new(IconName::Minus).xsmall())
                        .ghost()
                        .compact()
                        .disabled(!can_unstage_hunk)
                        .tooltip("Unstage 该 hunk")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let Some(hunk_index) = hunk_index_for_click else {
                                return;
                            };
                            if let Some(view) = this.diff_view.as_mut() {
                                view.current_hunk =
                                    hunk_index.min(view.diff_model.hunks.len().saturating_sub(1));
                            }
                            this.unstage_current_hunk(window, cx);
                            cx.notify();
                        }))
                };
                let revert_hunk_button = {
                    let hunk_index_for_click = hunk_index;
                    Button::new(("diff-hunk-revert-inline", hunk_index.unwrap_or(usize::MAX)))
                        .icon(Icon::new(PlateIconName::Undo2).xsmall())
                        .ghost()
                        .compact()
                        .disabled(!can_revert_hunk)
                        .tooltip("Revert 该 hunk")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let Some(hunk_index) = hunk_index_for_click else {
                                return;
                            };
                            if let Some(view) = this.diff_view.as_mut() {
                                view.current_hunk =
                                    hunk_index.min(view.diff_model.hunks.len().saturating_sub(1));
                            }
                            this.revert_current_hunk(window, cx);
                            cx.notify();
                        }))
                };
                let actions = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.))
                    .on_mouse_down(MouseButton::Left, |_, _window, cx| cx.stop_propagation())
                    .when(compare_target == CompareTarget::IndexToWorktree, |this| {
                        this.child(stage_hunk_button).child(revert_hunk_button)
                    })
                    .when(compare_target == CompareTarget::HeadToIndex, |this| {
                        this.child(unstage_hunk_button)
                    });

                div()
                    .h(height)
                    .px(px(12.))
                    .flex()
                    .justify_between()
                    .items_center()
                    .bg(theme.muted.alpha(0.35))
                    .hover(|this| this.bg(theme.muted.alpha(0.45)))
                    .cursor_pointer()
                    .font_family(theme.mono_font_family.clone())
                    .text_sm()
                    .child(div().flex_1().min_w(px(0.)).truncate().child(text))
                    .child(actions)
                    .on_mouse_down(MouseButton::Left, {
                        let hunk_index = hunk_index;
                        cx.listener(move |this, _, _window, cx| {
                            let Some(hunk_index) = hunk_index else {
                                return;
                            };
                            if let Some(view) = this.diff_view.as_mut() {
                                view.current_hunk =
                                    hunk_index.min(view.diff_model.hunks.len().saturating_sub(1));
                                view.scroll_handle
                                    .scroll_to_item(index, ScrollStrategy::Top);
                            }
                            cx.notify();
                        })
                    })
                    .context_menu(menu)
                    .into_any_element()
            }
            DisplayRow::Fold {
                old_start,
                new_start,
                len,
            } => {
                let label = format!(
                    "… 隐藏了 {len} 行未变更内容（old: {}..{}, new: {}..{}）点击展开",
                    old_start + 1,
                    old_start + len,
                    new_start + 1,
                    new_start + len
                );
                div()
                    .h(height)
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .bg(theme.muted.alpha(0.25))
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(label)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, cx| {
                            this.expand_fold(index);
                            if let Some(diff_view) = this.diff_view.as_ref() {
                                diff_view
                                    .scroll_handle
                                    .scroll_to_item(index, ScrollStrategy::Top);
                            }
                            cx.notify();
                        }),
                    )
                    .into_any_element()
            }
            DisplayRow::Code {
                source,
                kind,
                old_line,
                new_line,
                old_segments,
                new_segments,
            } => {
                let border = theme.border;
                let mono = theme.mono_font_family.clone();
                let highlight = self.diff_find_highlight_color(index, theme);
                let is_selected = source.is_some_and(|source| {
                    self.diff_view
                        .as_ref()
                        .is_some_and(|view| view.selected_rows.contains(&source))
                });
                let sign = match kind {
                    diffview::DiffRowKind::Added => "+",
                    diffview::DiffRowKind::Removed => "-",
                    diffview::DiffRowKind::Modified => "~",
                    diffview::DiffRowKind::Unchanged => " ",
                };
                let marker_color = match kind {
                    diffview::DiffRowKind::Added => theme.green,
                    diffview::DiffRowKind::Removed => theme.red,
                    diffview::DiffRowKind::Modified => theme.yellow,
                    diffview::DiffRowKind::Unchanged => theme.transparent,
                };
                let (bg, gutter_bg) = match kind {
                    diffview::DiffRowKind::Added => {
                        (theme.green.alpha(0.12), theme.green.alpha(0.08))
                    }
                    diffview::DiffRowKind::Removed => {
                        (theme.red.alpha(0.12), theme.red.alpha(0.08))
                    }
                    diffview::DiffRowKind::Modified => {
                        (theme.muted.alpha(0.25), theme.muted.alpha(0.2))
                    }
                    diffview::DiffRowKind::Unchanged => (theme.transparent, theme.transparent),
                };

                let old_no = old_line.map(|n| n.to_string()).unwrap_or_default();
                let new_no = new_line.map(|n| n.to_string()).unwrap_or_default();
                let segments = match kind {
                    diffview::DiffRowKind::Added => new_segments,
                    diffview::DiffRowKind::Removed => old_segments,
                    _ => {
                        if !new_segments.is_empty() {
                            new_segments
                        } else {
                            old_segments
                        }
                    }
                };

                let gutter_width = px(94.);
                let num_width = px(32.);

                let mut row = div()
                    .h(height)
                    .flex()
                    .flex_row()
                    .items_center()
                    .border_b_1()
                    .border_color(border.alpha(0.35))
                    .bg(bg)
                    .relative();

                if is_selected {
                    row = row
                        .child(
                            div()
                                .absolute()
                                .top(px(0.))
                                .bottom(px(0.))
                                .left(px(0.))
                                .right(px(0.))
                                .bg(theme.accent.alpha(0.1)),
                        )
                        .child(
                            div()
                                .absolute()
                                .top(px(0.))
                                .bottom(px(0.))
                                .left(px(0.))
                                .w(px(2.))
                                .bg(theme.accent),
                        );
                }

                row.child(div().w(px(3.)).h_full().bg(marker_color))
                    .child(
                        div()
                            .w(gutter_width)
                            .h_full()
                            .px(px(8.))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.))
                            .bg(gutter_bg)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .font_family(mono.clone())
                            .child(div().w(num_width).text_right().child(old_no))
                            .child(div().w(num_width).text_right().child(new_no))
                            .child(div().w(px(12.)).text_center().child(sign)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(0.))
                            .flex_1()
                            .min_w(px(0.))
                            .px(px(8.))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .font_family(mono)
                            .text_sm()
                            .children(render_segments(&segments, theme)),
                    )
                    .when_some(highlight, |this, color| {
                        this.child(div().w(px(2.)).h_full().bg(color))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            if let Some(diff_view) = this.diff_view.as_ref() {
                                diff_view
                                    .scroll_handle
                                    .scroll_to_item(index, ScrollStrategy::Top);
                            }
                            this.handle_diff_row_click(index, event.modifiers, window, cx);
                        }),
                    )
                    .context_menu(Self::diff_context_menu_builder(
                        app.clone(),
                        source,
                        source.map(|source| source.hunk_index),
                        kind,
                        compare_target.clone(),
                        file_status.clone(),
                        has_file_path,
                        selection_count,
                        git_available,
                    ))
                    .into_any_element()
            }
        }
    }

    fn render_pane_row(
        &mut self,
        side: Side,
        index: usize,
        height: Pixels,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let Some(row) = self
            .diff_view
            .as_ref()
            .and_then(|diff_view| diff_view.rows.get(index))
            .cloned()
        else {
            return div().into_any_element();
        };

        let (compare_target, file_status, has_file_path, selection_count) = self
            .diff_view
            .as_ref()
            .map(|view| {
                (
                    view.compare_target.clone(),
                    view.status.clone().unwrap_or_default(),
                    view.path.is_some(),
                    view.selected_rows.len(),
                )
            })
            .unwrap_or((CompareTarget::HeadToWorktree, String::new(), false, 0usize));
        let git_available = self.git_available;
        let app = cx.entity();

        match row {
            DisplayRow::HunkHeader { text } => {
                let hunk_index = self
                    .diff_view
                    .as_ref()
                    .and_then(|view| view.hunk_rows.binary_search(&index).ok());
                let menu = Self::diff_context_menu_builder(
                    app.clone(),
                    None,
                    hunk_index,
                    diffview::DiffRowKind::Unchanged,
                    compare_target.clone(),
                    file_status.clone(),
                    has_file_path,
                    selection_count,
                    git_available,
                );

                let is_untracked = is_untracked_status(&file_status);
                let is_rename_or_copy = status_xy(&file_status)
                    .is_some_and(|(x, y)| matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C'));
                let can_stage_hunk = git_available
                    && has_file_path
                    && hunk_index.is_some()
                    && !is_untracked
                    && !is_rename_or_copy
                    && compare_target == CompareTarget::IndexToWorktree;
                let can_unstage_hunk = git_available
                    && has_file_path
                    && hunk_index.is_some()
                    && !is_untracked
                    && !is_rename_or_copy
                    && compare_target == CompareTarget::HeadToIndex;
                let can_revert_hunk = git_available
                    && has_file_path
                    && hunk_index.is_some()
                    && !is_untracked
                    && !is_rename_or_copy
                    && compare_target == CompareTarget::IndexToWorktree;

                let pane_key = ((side as u64) << 32) | hunk_index.unwrap_or(usize::MAX) as u64;
                let stage_hunk_button = {
                    let hunk_index_for_click = hunk_index;
                    Button::new(("diff-hunk-stage-pane", pane_key))
                        .icon(Icon::new(IconName::Plus).xsmall())
                        .ghost()
                        .compact()
                        .disabled(!can_stage_hunk)
                        .tooltip("Stage 该 hunk")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let Some(hunk_index) = hunk_index_for_click else {
                                return;
                            };
                            if let Some(view) = this.diff_view.as_mut() {
                                view.current_hunk =
                                    hunk_index.min(view.diff_model.hunks.len().saturating_sub(1));
                            }
                            this.stage_current_hunk(window, cx);
                            cx.notify();
                        }))
                };
                let unstage_hunk_button = {
                    let hunk_index_for_click = hunk_index;
                    Button::new(("diff-hunk-unstage-pane", pane_key))
                        .icon(Icon::new(IconName::Minus).xsmall())
                        .ghost()
                        .compact()
                        .disabled(!can_unstage_hunk)
                        .tooltip("Unstage 该 hunk")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let Some(hunk_index) = hunk_index_for_click else {
                                return;
                            };
                            if let Some(view) = this.diff_view.as_mut() {
                                view.current_hunk =
                                    hunk_index.min(view.diff_model.hunks.len().saturating_sub(1));
                            }
                            this.unstage_current_hunk(window, cx);
                            cx.notify();
                        }))
                };
                let revert_hunk_button = {
                    let hunk_index_for_click = hunk_index;
                    Button::new(("diff-hunk-revert-pane", pane_key))
                        .icon(Icon::new(PlateIconName::Undo2).xsmall())
                        .ghost()
                        .compact()
                        .disabled(!can_revert_hunk)
                        .tooltip("Revert 该 hunk")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let Some(hunk_index) = hunk_index_for_click else {
                                return;
                            };
                            if let Some(view) = this.diff_view.as_mut() {
                                view.current_hunk =
                                    hunk_index.min(view.diff_model.hunks.len().saturating_sub(1));
                            }
                            this.revert_current_hunk(window, cx);
                            cx.notify();
                        }))
                };
                let actions = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.))
                    .on_mouse_down(MouseButton::Left, |_, _window, cx| cx.stop_propagation())
                    .when(compare_target == CompareTarget::IndexToWorktree, |this| {
                        this.child(stage_hunk_button).child(revert_hunk_button)
                    })
                    .when(compare_target == CompareTarget::HeadToIndex, |this| {
                        this.child(unstage_hunk_button)
                    });

                div()
                    .h(height)
                    .px(px(12.))
                    .flex()
                    .justify_between()
                    .items_center()
                    .bg(theme.muted.alpha(0.35))
                    .hover(|this| this.bg(theme.muted.alpha(0.45)))
                    .cursor_pointer()
                    .font_family(theme.mono_font_family.clone())
                    .text_sm()
                    .child(div().flex_1().min_w(px(0.)).truncate().child(text))
                    .child(actions)
                    .on_mouse_down(MouseButton::Left, {
                        let hunk_index = hunk_index;
                        cx.listener(move |this, _, _window, cx| {
                            let Some(hunk_index) = hunk_index else {
                                return;
                            };
                            if let Some(view) = this.diff_view.as_mut() {
                                view.current_hunk =
                                    hunk_index.min(view.diff_model.hunks.len().saturating_sub(1));
                                view.scroll_handle
                                    .scroll_to_item(index, ScrollStrategy::Top);
                            }
                            cx.notify();
                        })
                    })
                    .context_menu(menu)
                    .into_any_element()
            }
            DisplayRow::Fold { len, .. } => div()
                .h(height)
                .px(px(12.))
                .flex()
                .items_center()
                .bg(theme.muted.alpha(0.25))
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(format!("… 隐藏了 {len} 行未变更内容，点击展开"))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _window, cx| {
                        this.expand_fold(index);
                        if let Some(diff_view) = this.diff_view.as_ref() {
                            diff_view
                                .scroll_handle
                                .scroll_to_item(index, ScrollStrategy::Top);
                        }
                        cx.notify();
                    }),
                )
                .into_any_element(),
            DisplayRow::Code {
                source,
                kind,
                old_line,
                new_line,
                old_segments,
                new_segments,
            } => {
                let border = theme.border;
                let mono = theme.mono_font_family.clone();
                let (line_no, segments) = match side {
                    Side::Old => (old_line, old_segments),
                    Side::New => (new_line, new_segments),
                };
                let highlight = self.diff_find_highlight_color(index, theme);
                let is_selected = source.is_some_and(|source| {
                    self.diff_view
                        .as_ref()
                        .is_some_and(|view| view.selected_rows.contains(&source))
                });

                let mut row = div()
                    .h(height)
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(border.alpha(0.35))
                    .relative();

                if is_selected {
                    row = row.child(
                        div()
                            .absolute()
                            .top(px(0.))
                            .bottom(px(0.))
                            .left(px(0.))
                            .right(px(0.))
                            .bg(theme.accent.alpha(0.1)),
                    );
                }

                row.child(
                    render_side(side, kind, line_no, &segments, mono, theme)
                        .when_some(highlight, |this, color| {
                            this.child(div().w(px(2.)).h_full().bg(color))
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                if let Some(diff_view) = this.diff_view.as_ref() {
                                    diff_view
                                        .scroll_handle
                                        .scroll_to_item(index, ScrollStrategy::Top);
                                }
                                this.handle_diff_row_click(index, event.modifiers, window, cx);
                            }),
                        ),
                )
                .context_menu(Self::diff_context_menu_builder(
                    app.clone(),
                    source,
                    source.map(|source| source.hunk_index),
                    kind,
                    compare_target.clone(),
                    file_status.clone(),
                    has_file_path,
                    selection_count,
                    git_available,
                ))
                .into_any_element()
            }
        }
    }
}
