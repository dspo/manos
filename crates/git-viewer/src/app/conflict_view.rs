use super::*;

impl GitViewerApp {
    pub(super) fn render_conflict_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let current_path = self
            .conflict_view
            .as_ref()
            .and_then(|view| view.path.clone());
        let (can_prev_file, can_next_file) = self.file_nav_state(current_path.as_deref());
        let can_refresh = self.git_available && !self.loading;

        let Some(conflict_view) = self.conflict_view.as_mut() else {
            return div().p(px(12.)).child("No conflict view");
        };

        let git_available = self.git_available;
        let two_pane = matches!(self.split_layout, SplitLayout::TwoPane);
        let show_result_editor = conflict_view.show_result_editor;
        let title = conflict_view.title.clone();
        let conflicts_count = conflict_view.conflicts.len();
        let can_prev = conflict_view.current_conflict > 0;
        let conflict_total = conflict_view.conflict_rows.len();
        let can_next = conflict_view.current_conflict + 1 < conflict_total;
        let has_path = conflict_view.path.is_some();
        let can_save = has_path && conflicts_count == 0;
        let can_add = can_save && git_available;
        let rows_len = conflict_view.rows.len();
        let scroll_handle = conflict_view.scroll_handle.clone();
        let scroll_state = conflict_view.scroll_state.clone();
        let row_height = window.line_height() + px(4.);
        let item_sizes = conflict_view.item_sizes(row_height);
        let conflict_position = if conflict_total == 0 {
            "0/0".to_string()
        } else {
            format!("{}/{}", conflict_view.current_conflict + 1, conflict_total)
        };

        let save_file_label: SharedString = if can_save {
            "保存到文件".into()
        } else if has_path {
            "保存到文件（仍有冲突未解决）".into()
        } else {
            "保存到文件（demo 不支持）".into()
        };
        let save_add_label: SharedString = if can_add {
            "保存并 git add".into()
        } else if !git_available {
            "保存并 git add（git 不可用）".into()
        } else if has_path {
            "保存并 git add（仍有冲突未解决）".into()
        } else {
            "保存并 git add（demo 不支持）".into()
        };

        let save_shortcut = if cfg!(target_os = "macos") {
            "Cmd+S"
        } else {
            "Ctrl+S"
        };
        let save_add_shortcut = if cfg!(target_os = "macos") {
            "Cmd+Shift+S"
        } else {
            "Ctrl+Shift+S"
        };
        let find_shortcut = if cfg!(target_os = "macos") {
            "Cmd+F"
        } else {
            "Ctrl+F"
        };
        let shortcuts_message: SharedString = format!(
            "快捷键：\n\
             Esc 返回\n\
             Alt+N / Alt+P 下一/上一冲突\n\
             Alt+L 切换对齐/分栏\n\
             Alt+A 应用编辑\n\
             {find_shortcut} 搜索（编辑器）\n\
             {save_shortcut} 保存（冲突清零后）\n\
             {save_add_shortcut} 保存并 git add（冲突清零后）\n"
        )
        .into();

        let app = cx.entity();
        let more_menu = {
            let app_for_menu = app.clone();
            Popover::new("conflict-more-menu")
                .appearance(false)
                .trigger(
                    Button::new("conflict-more-trigger")
                        .icon(IconName::Ellipsis)
                        .ghost()
                        .small()
                        .compact()
                        .tooltip("更多操作")
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
                        Button::new(id)
                            .label(label)
                            .ghost()
                            .disabled(disabled)
                            .w_full()
                            .on_click({
                                let action = action.clone();
                                move |_, window, cx| {
                                    action(window, cx);
                                    popover.update(cx, |state, cx| state.dismiss(window, cx));
                                }
                            })
                            .into_any_element()
                    };

                    let split_label: SharedString = if two_pane {
                        "分栏: 开".into()
                    } else {
                        "分栏: 关".into()
                    };
                    let result_label: SharedString = if show_result_editor {
                        "结果面板: 开".into()
                    } else {
                        "结果面板: 关".into()
                    };

                    let app_for_split = app_for_menu.clone();
                    let toggle_split = Rc::new(move |_window: &mut Window, cx: &mut App| {
                        app_for_split.update(cx, |this, cx| {
                            this.split_layout = match this.split_layout {
                                SplitLayout::Aligned => SplitLayout::TwoPane,
                                SplitLayout::TwoPane => SplitLayout::Aligned,
                            };
                            cx.notify();
                        });
                    });

                    let app_for_result = app_for_menu.clone();
                    let toggle_result = Rc::new(move |_window: &mut Window, cx: &mut App| {
                        app_for_result.update(cx, |this, cx| {
                            if let Some(view) = this.conflict_view.as_mut() {
                                view.show_result_editor = !view.show_result_editor;
                            }
                            cx.notify();
                        });
                    });

                    let app_for_apply = app_for_menu.clone();
                    let apply_editor = Rc::new(move |window: &mut Window, cx: &mut App| {
                        app_for_apply.update(cx, |this, cx| {
                            this.apply_conflict_editor(window, cx);
                        });
                    });

                    let shortcuts_message = shortcuts_message.clone();
                    let show_shortcuts = Rc::new(move |window: &mut Window, cx: &mut App| {
                        window.push_notification(
                            Notification::new().message(shortcuts_message.to_string()),
                            cx,
                        );
                    });

                    let app_for_save = app_for_menu.clone();
                    let save_to_file = Rc::new(move |window: &mut Window, cx: &mut App| {
                        app_for_save.update(cx, |this, cx| {
                            this.save_conflict_to_working_tree(false, window, cx);
                            cx.notify();
                        });
                    });

                    let app_for_save_add = app_for_menu.clone();
                    let save_and_add = Rc::new(move |window: &mut Window, cx: &mut App| {
                        app_for_save_add.update(cx, |this, cx| {
                            this.save_conflict_to_working_tree(true, window, cx);
                            cx.notify();
                        });
                    });

                    div()
                        .p(px(8.))
                        .bg(theme.popover)
                        .border_1()
                        .border_color(theme.border)
                        .rounded(theme.radius)
                        .shadow_md()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .px(px(4.))
                                .py(px(2.))
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("视图"),
                        )
                        .child(make_action(
                            "conflict-more-toggle-split",
                            split_label,
                            false,
                            toggle_split,
                        ))
                        .child(make_action(
                            "conflict-more-toggle-result",
                            result_label,
                            false,
                            toggle_result,
                        ))
                        .child(make_action(
                            "conflict-more-apply",
                            "应用编辑".into(),
                            !show_result_editor,
                            apply_editor,
                        ))
                        .child(div().h(px(1.)).bg(theme.border.alpha(0.4)))
                        .child(
                            div()
                                .px(px(4.))
                                .py(px(2.))
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("保存"),
                        )
                        .child(make_action(
                            "conflict-more-save",
                            save_file_label.clone(),
                            !can_save,
                            save_to_file,
                        ))
                        .child(make_action(
                            "conflict-more-save-add",
                            save_add_label.clone(),
                            !can_add,
                            save_and_add,
                        ))
                        .child(make_action(
                            "conflict-more-shortcuts",
                            "快捷键…".into(),
                            false,
                            show_shortcuts,
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
                    .child(div().child(format!("冲突: {conflict_position}")))
                    .child(
                        Button::new("conflict-prev-file")
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
                        Button::new("conflict-next-file")
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
                        Button::new("conflict-prev")
                            .icon(IconName::ChevronUp)
                            .ghost()
                            .small()
                            .compact()
                            .tooltip_with_action("上一冲突", &Prev, Some(CONTEXT))
                            .disabled(!can_prev)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.jump_conflict(-1);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("conflict-next")
                            .icon(IconName::ChevronDown)
                            .ghost()
                            .small()
                            .compact()
                            .tooltip_with_action("下一冲突", &Next, Some(CONTEXT))
                            .disabled(!can_next)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.jump_conflict(1);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("conflict-refresh")
                            .icon(PlateIconName::Redo2)
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

        let list = match self.split_layout {
            SplitLayout::Aligned => v_virtual_list(
                cx.entity(),
                "conflict-aligned-list",
                item_sizes,
                move |this, visible_range, window, cx| {
                    visible_range
                        .map(|index| this.render_conflict_row(index, row_height, window, cx))
                        .collect::<Vec<_>>()
                },
            )
            .track_scroll(&scroll_handle)
            .into_any_element(),
            SplitLayout::TwoPane => {
                let show_base = conflict_view
                    .conflicts
                    .iter()
                    .any(|conflict| conflict.base.is_some());

                let ours_list = v_virtual_list(
                    cx.entity(),
                    "conflict-pane-ours",
                    item_sizes.clone(),
                    move |this, visible_range, window, cx| {
                        visible_range
                            .map(|index| {
                                this.render_conflict_pane_row(
                                    ConflictPane::Ours,
                                    index,
                                    row_height,
                                    window,
                                    cx,
                                )
                            })
                            .collect::<Vec<_>>()
                    },
                )
                .track_scroll(&scroll_handle)
                .into_any_element();

                let base_list = show_base.then(|| {
                    v_virtual_list(
                        cx.entity(),
                        "conflict-pane-base",
                        item_sizes.clone(),
                        move |this, visible_range, window, cx| {
                            visible_range
                                .map(|index| {
                                    this.render_conflict_pane_row(
                                        ConflictPane::Base,
                                        index,
                                        row_height,
                                        window,
                                        cx,
                                    )
                                })
                                .collect::<Vec<_>>()
                        },
                    )
                    .track_scroll(&scroll_handle)
                    .into_any_element()
                });

                let theirs_list = v_virtual_list(
                    cx.entity(),
                    "conflict-pane-theirs",
                    item_sizes,
                    move |this, visible_range, window, cx| {
                        visible_range
                            .map(|index| {
                                this.render_conflict_pane_row(
                                    ConflictPane::Theirs,
                                    index,
                                    row_height,
                                    window,
                                    cx,
                                )
                            })
                            .collect::<Vec<_>>()
                    },
                )
                .track_scroll(&scroll_handle)
                .into_any_element();

                let theme = cx.theme();
                let pane_header_height = window.line_height() + px(6.);
                let ours_label = conflict_view
                    .conflicts
                    .first()
                    .map(|c| c.ours_branch_name.clone())
                    .unwrap_or_else(|| "Ours".to_string());
                let theirs_label = conflict_view
                    .conflicts
                    .first()
                    .map(|c| c.theirs_branch_name.clone())
                    .unwrap_or_else(|| "Theirs".to_string());

                let old_header = div()
                    .h(pane_header_height)
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .bg(theme.muted.alpha(0.2))
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(format!("Ours: {ours_label}"));
                let base_header = div()
                    .h(pane_header_height)
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .bg(theme.muted.alpha(0.2))
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Base（diff3）");
                let theirs_header = div()
                    .h(pane_header_height)
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .bg(theme.muted.alpha(0.2))
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(format!("Theirs: {theirs_label}"));

                let border_color = theme.border.alpha(0.6);
                let mut row = div().flex().flex_row().size_full().child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(px(0.))
                        .border_r_1()
                        .border_color(border_color)
                        .child(old_header)
                        .child(div().flex_1().min_h(px(0.)).child(ours_list)),
                );

                if let Some(base_list) = base_list {
                    row = row.child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.))
                            .border_r_1()
                            .border_color(border_color)
                            .child(base_header)
                            .child(div().flex_1().min_h(px(0.)).child(base_list)),
                    );
                }

                row.child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(px(0.))
                        .child(theirs_header)
                        .child(div().flex_1().min_h(px(0.)).child(theirs_list)),
                )
                .into_any_element()
            }
        };

        let status_left = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .child(div().truncate().child(format!(
                "文件: {}",
                conflict_view.path.as_deref().unwrap_or("<demo>")
            )))
            .child(format!("未解决: {conflicts_count}"));
        let status_hint = div()
            .truncate()
            .text_color(cx.theme().muted_foreground)
            .child("Esc 返回 · Alt+N/P 导航 · Alt+A 应用 · Cmd/Ctrl+S 保存");
        let status_right = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .child(if can_add {
                "可保存并标记已解决".to_string()
            } else if can_save {
                "可保存（git 不可用，无法标记已解决）".to_string()
            } else if has_path {
                "仍有冲突未解决（编辑后点“应用”）".to_string()
            } else {
                "demo（不可保存）".to_string()
            })
            .child(status_hint);
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

        let conflict_scroll_ruler = if rows_len > 0 && !conflict_view.conflict_rows.is_empty() {
            let theme = cx.theme();
            let total_rows = rows_len.max(1) as f32;
            let right_inset = px(14.);
            let width = px(8.);
            let conflict_total = conflict_view.conflict_rows.len();
            let max_markers = 200usize;
            let marker_step = (conflict_total / max_markers).max(1);
            let mut marker_indices: Vec<usize> = (0..conflict_total).step_by(marker_step).collect();
            marker_indices.push(conflict_view.current_conflict);
            marker_indices.sort_unstable();
            marker_indices.dedup();

            let mut ruler = div()
                .id("conflict-scroll-ruler")
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

            for conflict_index in marker_indices {
                let row_index = conflict_view
                    .conflict_rows
                    .get(conflict_index)
                    .copied()
                    .unwrap_or(0);
                let fraction = (row_index as f32 / total_rows).clamp(0.0, 1.0);
                let is_current = conflict_index == conflict_view.current_conflict;
                let color = if is_current {
                    theme.red
                } else {
                    theme.red.alpha(0.65)
                };
                let height = if is_current { px(6.) } else { px(3.) };
                let radius = if is_current { px(6.) } else { px(4.) };

                ruler = ruler.child(
                    div()
                        .id(("conflict-ruler-marker", conflict_index))
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
                                if let Some(conflict_view) = this.conflict_view.as_mut() {
                                    if conflict_index < conflict_view.conflict_rows.len() {
                                        conflict_view.current_conflict = conflict_index;
                                        conflict_view
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

        if let Some(ruler) = conflict_scroll_ruler {
            viewport = viewport.child(ruler);
        }

        let body: AnyElement = if show_result_editor {
            let result_panel = div()
                .id("conflict-result-panel")
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
                        .child(div().text_sm().child("合并结果（可编辑）"))
                        .child(
                            div().flex().flex_row().items_center().gap(px(6.)).child(
                                Button::new("conflict-apply-editor-inline")
                                    .label("应用")
                                    .ghost()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.apply_conflict_editor(window, cx);
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
                        .child(Input::new(&conflict_view.result_input).h_full().w_full()),
                );

            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .child(
                    v_resizable("conflict-result-split")
                        .child(resizable_panel().child(viewport.size_full()))
                        .child(
                            resizable_panel()
                                .size(px(240.))
                                .size_range(px(140.)..px(900.))
                                .child(result_panel),
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
    fn render_conflict_row(
        &mut self,
        index: usize,
        height: Pixels,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = cx.theme();
        let show_base = self.conflict_view.as_ref().is_some_and(|view| {
            view.conflicts
                .iter()
                .any(|conflict| conflict.base.is_some())
        });
        let Some(row) = self
            .conflict_view
            .as_ref()
            .and_then(|view| view.rows.get(index))
            .cloned()
        else {
            return div();
        };

        match row {
            ConflictRow::EmptyState { text } => div()
                .h(height)
                .px(px(12.))
                .flex()
                .items_center()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(text),
            ConflictRow::BlockHeader {
                conflict_index,
                ours_branch_name,
                theirs_branch_name,
                has_base,
            } => {
                let base_button = if has_base {
                    Button::new(("conflict-base", conflict_index))
                        .label("采纳 base")
                        .ghost()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.resolve_conflict(
                                conflict_index,
                                ConflictResolution::Base,
                                window,
                                cx,
                            );
                            cx.notify();
                        }))
                        .into_any_element()
                } else {
                    div().into_any_element()
                };

                div()
                    .h(height)
                    .px(px(12.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .bg(theme.muted.alpha(0.35))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .font_family(theme.mono_font_family.clone())
                            .text_sm()
                            .child(format!(
                                "CONFLICT #{}  ours: {}  theirs: {}",
                                conflict_index + 1,
                                ours_branch_name,
                                theirs_branch_name
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                Button::new(("conflict-ours", conflict_index))
                                    .label("采纳 ours")
                                    .ghost()
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.resolve_conflict(
                                            conflict_index,
                                            ConflictResolution::Ours,
                                            window,
                                            cx,
                                        );
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new(("conflict-theirs", conflict_index))
                                    .label("采纳 theirs")
                                    .ghost()
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.resolve_conflict(
                                            conflict_index,
                                            ConflictResolution::Theirs,
                                            window,
                                            cx,
                                        );
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new(("conflict-both", conflict_index))
                                    .label("保留两侧")
                                    .ghost()
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.resolve_conflict(
                                            conflict_index,
                                            ConflictResolution::Both,
                                            window,
                                            cx,
                                        );
                                        cx.notify();
                                    })),
                            )
                            .child(base_button),
                    )
            }
            ConflictRow::Code {
                kind,
                ours_segments,
                base_segments,
                theirs_segments,
            } => {
                let border = theme.border;
                let mono = theme.mono_font_family.clone();
                let mut row = div()
                    .h(height)
                    .flex()
                    .flex_row()
                    .items_center()
                    .border_b_1()
                    .border_color(border.alpha(0.35))
                    .child(
                        render_side(Side::Old, kind, None, &ours_segments, mono.clone(), theme)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _window, cx| {
                                    if let Some(view) = this.conflict_view.as_ref() {
                                        view.scroll_handle
                                            .scroll_to_item(index, ScrollStrategy::Top);
                                    }
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(div().w(px(1.)).h_full().bg(border.alpha(0.6)));

                if show_base {
                    row = row
                        .child(
                            render_side(
                                Side::Old,
                                diffview::DiffRowKind::Unchanged,
                                None,
                                &base_segments,
                                mono.clone(),
                                theme,
                            )
                            .bg(theme.muted.alpha(0.06))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _window, cx| {
                                    if let Some(view) = this.conflict_view.as_ref() {
                                        view.scroll_handle
                                            .scroll_to_item(index, ScrollStrategy::Top);
                                    }
                                    cx.notify();
                                }),
                            ),
                        )
                        .child(div().w(px(1.)).h_full().bg(border.alpha(0.6)));
                }

                row.child(
                    render_side(Side::New, kind, None, &theirs_segments, mono, theme)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _window, cx| {
                                if let Some(view) = this.conflict_view.as_ref() {
                                    view.scroll_handle
                                        .scroll_to_item(index, ScrollStrategy::Top);
                                }
                                cx.notify();
                            }),
                        ),
                )
            }
        }
    }

    fn render_conflict_pane_row(
        &mut self,
        pane: ConflictPane,
        index: usize,
        height: Pixels,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = cx.theme();
        let Some(row) = self
            .conflict_view
            .as_ref()
            .and_then(|view| view.rows.get(index))
            .cloned()
        else {
            return div();
        };

        match row {
            ConflictRow::EmptyState { text } => div()
                .h(height)
                .px(px(12.))
                .flex()
                .items_center()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(text),
            ConflictRow::BlockHeader {
                conflict_index,
                ours_branch_name,
                theirs_branch_name,
                has_base,
            } => match pane {
                ConflictPane::Ours => {
                    let base_button = if has_base {
                        Button::new(("conflict-base", conflict_index))
                            .label("采纳 base")
                            .ghost()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.resolve_conflict(
                                    conflict_index,
                                    ConflictResolution::Base,
                                    window,
                                    cx,
                                );
                                cx.notify();
                            }))
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    };

                    div()
                        .h(height)
                        .px(px(12.))
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .bg(theme.muted.alpha(0.35))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(8.))
                                .font_family(theme.mono_font_family.clone())
                                .text_sm()
                                .child(format!(
                                    "CONFLICT #{}  ours: {}",
                                    conflict_index + 1,
                                    ours_branch_name
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(6.))
                                .flex_wrap()
                                .child(
                                    Button::new(("conflict-ours", conflict_index))
                                        .label("采纳 ours")
                                        .ghost()
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.resolve_conflict(
                                                conflict_index,
                                                ConflictResolution::Ours,
                                                window,
                                                cx,
                                            );
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new(("conflict-theirs", conflict_index))
                                        .label("采纳 theirs")
                                        .ghost()
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.resolve_conflict(
                                                conflict_index,
                                                ConflictResolution::Theirs,
                                                window,
                                                cx,
                                            );
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new(("conflict-both", conflict_index))
                                        .label("保留两侧")
                                        .ghost()
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.resolve_conflict(
                                                conflict_index,
                                                ConflictResolution::Both,
                                                window,
                                                cx,
                                            );
                                            cx.notify();
                                        })),
                                )
                                .child(base_button),
                        )
                }
                ConflictPane::Base => div()
                    .h(height)
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .bg(theme.muted.alpha(0.35))
                    .font_family(theme.mono_font_family.clone())
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(if has_base {
                        format!("CONFLICT #{}  base（diff3）", conflict_index + 1)
                    } else {
                        format!("CONFLICT #{}  base（无）", conflict_index + 1)
                    }),
                ConflictPane::Theirs => div()
                    .h(height)
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .bg(theme.muted.alpha(0.35))
                    .font_family(theme.mono_font_family.clone())
                    .text_sm()
                    .child(format!(
                        "CONFLICT #{}  theirs: {}",
                        conflict_index + 1,
                        theirs_branch_name
                    )),
            },
            ConflictRow::Code {
                kind,
                ours_segments,
                base_segments,
                theirs_segments,
            } => {
                let border = theme.border;
                let mono = theme.mono_font_family.clone();
                let (side, kind, segments, base_bg) = match pane {
                    ConflictPane::Ours => (Side::Old, kind, ours_segments, None),
                    ConflictPane::Theirs => (Side::New, kind, theirs_segments, None),
                    ConflictPane::Base => (
                        Side::Old,
                        diffview::DiffRowKind::Unchanged,
                        base_segments,
                        Some(theme.muted.alpha(0.06)),
                    ),
                };

                div()
                    .h(height)
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(border.alpha(0.35))
                    .child({
                        let mut cell = render_side(side, kind, None, &segments, mono, theme);
                        if let Some(bg) = base_bg {
                            cell = cell.bg(bg);
                        }
                        cell.on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _window, cx| {
                                if let Some(view) = this.conflict_view.as_ref() {
                                    view.scroll_handle
                                        .scroll_to_item(index, ScrollStrategy::Top);
                                }
                                cx.notify();
                            }),
                        )
                    })
            }
        }
    }
}
