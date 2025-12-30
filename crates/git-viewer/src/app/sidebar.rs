use anyhow::Context as _;
use gpui_component::IconNamed as _;
use gpui_component::list::ListItem;

use super::*;

impl GitViewerApp {
    pub(super) fn invalidate_changes_tree(&mut self) {
        self.changes_tree_revision = self.changes_tree_revision.wrapping_add(1);
    }

    pub(super) fn sync_changes_tree_selection_for_entry(
        &mut self,
        entry: &FileEntry,
        cx: &mut Context<Self>,
    ) {
        if self.changes_tree_root_items.is_empty() {
            return;
        }

        let path = entry.path.as_str();
        let select_ix = flat_tree_index(&self.changes_tree_root_items, path);
        if let Some(ix) = select_ix {
            self.changes_tree.update(cx, |state, cx| {
                state.set_selected_index(Some(ix), cx);
                state.scroll_to_item(ix, gpui::ScrollStrategy::Center);
            });
            return;
        }

        let status_hint = if !entry.status.is_empty() {
            entry.status.as_str()
        } else {
            self.files
                .iter()
                .find(|item| item.path == path)
                .map(|item| item.status.as_str())
                .unwrap_or("")
        };

        let group_id = if is_untracked_status(status_hint) {
            ":group:untracked"
        } else {
            ":group:changes"
        };

        if let Some(item) = self.changes_tree_item_cache.get(group_id) {
            let _ = item.clone().expanded(true);
        }

        let mut dir_id = group_id.to_string();
        let mut parts = path.split('/').peekable();
        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                break;
            }
            dir_id.push('/');
            dir_id.push_str(part);
            if let Some(item) = self.changes_tree_item_cache.get(dir_id.as_str()) {
                let _ = item.clone().expanded(true);
            }
        }

        let root_items = self.changes_tree_root_items.clone();
        self.changes_tree
            .update(cx, |state, cx| state.set_items(root_items, cx));

        if let Some(ix) = flat_tree_index(&self.changes_tree_root_items, path) {
            self.changes_tree.update(cx, |state, cx| {
                state.set_selected_index(Some(ix), cx);
                state.scroll_to_item(ix, gpui::ScrollStrategy::Center);
            });
        }
    }

    pub(super) fn refresh_current_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.git_available {
            window.push_notification(
                Notification::new().message("未检测到 git 命令，无法刷新当前文件"),
                cx,
            );
            return;
        }

        if self.loading {
            return;
        }

        let open_path = self
            .diff_view
            .as_ref()
            .and_then(|view| view.path.clone())
            .or_else(|| {
                self.conflict_view
                    .as_ref()
                    .and_then(|view| view.path.clone())
            });
        let Some(open_path) = open_path else {
            window.push_notification(Notification::new().message("未打开文件"), cx);
            return;
        };

        if self.conflict_view.is_some() {
            let entry = self
                .files
                .iter()
                .find(|entry| entry.path == open_path)
                .cloned()
                .unwrap_or(FileEntry {
                    path: open_path,
                    status: "UU".to_string(),
                    orig_path: None,
                });
            self.open_conflict_file(entry, window, cx);
            return;
        }

        let Some(diff_view) = self.diff_view.as_ref() else {
            return;
        };
        let compare_target = diff_view.compare_target.clone();

        if diff_view.path.is_none() {
            window.push_notification(
                Notification::new().message("当前 diff 为 demo，无法刷新文件 diff"),
                cx,
            );
            return;
        }

        let cache_key =
            DiffCacheKey::new(open_path.clone(), compare_target.clone(), self.diff_options);
        let _ = self.diff_cache.take(&cache_key);
        self.diff_prefetch_inflight.remove(&cache_key);
        if self.current_diff_cache_key.as_ref() == Some(&cache_key) {
            self.current_diff_cache_key = None;
        }

        let fallback_status = diff_view.status.clone().unwrap_or_default();
        let fallback_orig = diff_view.orig_path.clone();
        let entry = self
            .files
            .iter()
            .find(|entry| entry.path == open_path)
            .cloned()
            .unwrap_or(FileEntry {
                path: open_path.clone(),
                status: fallback_status,
                orig_path: fallback_orig,
            });

        match compare_target.clone() {
            CompareTarget::Refs { left, right } => {
                self.open_file_diff_with_refs(
                    open_path,
                    diff_view.status.clone(),
                    left,
                    right,
                    window,
                    cx,
                );
            }
            _ => {
                self.open_file_diff_with_target(entry, compare_target, window, cx);
            }
        }
    }

    fn set_changes_tree_expanded(&mut self, expanded: bool, cx: &mut Context<Self>) {
        fn walk(item: &TreeItem, expanded: bool) {
            if !item.is_folder() {
                return;
            }

            let is_group =
                item.id.as_str() == ":group:changes" || item.id.as_str() == ":group:untracked";
            let next = if is_group { true } else { expanded };
            let _ = item.clone().expanded(next);

            for child in &item.children {
                walk(child, expanded);
            }
        }

        for item in &self.changes_tree_root_items {
            walk(item, expanded);
        }

        let selected_id = self
            .changes_tree
            .read(cx)
            .selected_entry()
            .map(|entry| entry.item().id.clone());
        let root_items = self.changes_tree_root_items.clone();

        self.changes_tree.update(cx, |state, cx| {
            state.set_items(root_items.clone(), cx);

            if let Some(id) = selected_id {
                if let Some(ix) = flat_tree_index(&root_items, id.as_str()) {
                    state.set_selected_index(Some(ix), cx);
                    state.scroll_to_item(ix, gpui::ScrollStrategy::Center);
                }
            }
        });
    }

    fn collect_entries_under_directory(&self, dir_path: &str) -> Vec<FileEntry> {
        #[derive(Clone, Copy)]
        enum GroupScope {
            Any,
            TrackedOnly,
            UntrackedOnly,
        }

        let (scope, dir_prefix) = if dir_path == ":group:changes" {
            (GroupScope::TrackedOnly, None)
        } else if dir_path == ":group:untracked" {
            (GroupScope::UntrackedOnly, None)
        } else if let Some(rest) = dir_path.strip_prefix(":group:changes/") {
            (GroupScope::TrackedOnly, Some(rest))
        } else if let Some(rest) = dir_path.strip_prefix(":group:untracked/") {
            (GroupScope::UntrackedOnly, Some(rest))
        } else {
            (GroupScope::Any, Some(dir_path))
        };

        let prefix = dir_prefix.map(|dir| format!("{dir}/"));
        let mut entries = Vec::new();

        for entry in self
            .files
            .iter()
            .filter(|entry| matches_filter(entry, self.status_filter))
        {
            if is_ignored_status(&entry.status) {
                continue;
            }

            match scope {
                GroupScope::Any => {}
                GroupScope::TrackedOnly => {
                    if is_untracked_status(&entry.status) {
                        continue;
                    }
                }
                GroupScope::UntrackedOnly => {
                    if !is_untracked_status(&entry.status) {
                        continue;
                    }
                }
            }

            if let Some(prefix) = prefix.as_deref() {
                if !entry.path.starts_with(prefix) {
                    continue;
                }
            }

            entries.push(entry.clone());
        }

        entries
    }

    fn open_rollback_selected_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.git_available {
            window.push_notification(
                Notification::new().message("未检测到 git 命令，无法回滚文件"),
                cx,
            );
            return;
        }
        if self.loading {
            return;
        }

        let Some(selected_entry) = self.changes_tree.read(cx).selected_entry() else {
            window.push_notification(Notification::new().message("未选择要回滚的文件/目录"), cx);
            return;
        };

        let selected_id = selected_entry.item().id.clone();
        let selected_label = selected_entry.item().label.clone();
        let mut entries: Vec<FileEntry> = match self.changes_tree_meta.get(&selected_id) {
            Some(ChangesTreeMeta::File { entry }) => vec![entry.clone()],
            Some(ChangesTreeMeta::Directory { path, .. }) => {
                self.collect_entries_under_directory(path)
            }
            None => {
                if selected_entry.is_folder() {
                    self.collect_entries_under_directory(selected_id.as_ref())
                } else {
                    Vec::new()
                }
            }
        };

        entries.retain(|entry| {
            !is_ignored_status(&entry.status) && !is_conflict_status(&entry.status)
        });

        if entries.is_empty() {
            window.push_notification(Notification::new().message("没有可回滚的文件"), cx);
            return;
        }

        let file_count = entries.len();
        let has_untracked = entries
            .iter()
            .any(|entry| is_untracked_status(&entry.status));
        let has_new_file = entries.iter().any(|entry| entry.status.contains('A'));

        let message: SharedString = if file_count == 1 {
            let status = entries[0].status.as_str();
            if is_untracked_status(status) {
                "将从磁盘删除该未跟踪文件。".into()
            } else if status.contains('A') {
                "将删除该新文件，并移除暂存区记录（如有）。".into()
            } else {
                "将把该文件恢复到 HEAD 版本（同时丢弃暂存与工作区改动）。".into()
            }
        } else {
            let mut parts = Vec::new();
            parts.push(format!("将对选中的 {file_count} 个文件执行 Rollback。"));
            if has_untracked {
                parts.push("其中未跟踪文件将从磁盘删除。".to_string());
            }
            if has_new_file {
                parts.push("其中新增文件将被删除，并移除暂存区记录（如有）。".to_string());
            }
            parts.push("此操作不可撤销。".to_string());
            parts.join("\n").into()
        };

        let ok_text: SharedString = if file_count == 1 && has_untracked {
            "Delete".into()
        } else {
            "Rollback".into()
        };

        let app = cx.entity();
        window.open_dialog(cx, move |dialog, _window, cx| {
            let theme = cx.theme();
            let selected_label = selected_label.clone();
            let entries_for_ok = entries.clone();
            let ok_text = ok_text.clone();
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
                                .child(selected_label.clone()),
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
                    let app = app.clone();
                    move |_, window, cx| {
                        app.update(cx, |this, cx| {
                            this.rollback_paths(entries_for_ok.clone(), window, cx);
                            cx.notify();
                        });
                        true
                    }
                })
        });
    }

    fn shelve_silently(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.git_available {
            window.push_notification(
                Notification::new().message("未检测到 git 命令，无法 Shelve"),
                cx,
            );
            return;
        }
        if self.loading {
            return;
        }

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        window.push_notification(
            Notification::new().message("git stash push -u -m \"git-viewer shelve\""),
            cx,
        );

        cx.spawn_in(window, async move |_, window| {
            let result = window
                .background_executor()
                .spawn(async move {
                    run_git(
                        repo_root.as_path(),
                        ["stash", "push", "-u", "-m", "git-viewer shelve"],
                    )
                    .context("执行 git stash 失败")
                })
                .await;

            window
                .update(|window, cx| {
                    match result {
                        Ok(()) => {
                            window.push_notification(Notification::new().message("Shelve 成功"), cx)
                        }
                        Err(err) => window.push_notification(
                            Notification::new().message(format!("Shelve 失败：{err:#}")),
                            cx,
                        ),
                    }

                    this.update(cx, |this, cx| {
                        this.refresh_git_status(window, cx);
                        cx.notify();
                    });
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn toggle_directory_selection(&mut self, dir_path: String, select: bool) {
        #[derive(Clone, Copy)]
        enum GroupScope {
            Any,
            TrackedOnly,
            UntrackedOnly,
        }

        let (scope, dir_prefix) = if dir_path == ":group:changes" {
            (GroupScope::TrackedOnly, None)
        } else if dir_path == ":group:untracked" {
            (GroupScope::UntrackedOnly, None)
        } else if let Some(rest) = dir_path.strip_prefix(":group:changes/") {
            (GroupScope::TrackedOnly, Some(rest))
        } else if let Some(rest) = dir_path.strip_prefix(":group:untracked/") {
            (GroupScope::UntrackedOnly, Some(rest))
        } else {
            (GroupScope::Any, Some(dir_path.as_str()))
        };

        let prefix = dir_prefix.map(|dir| format!("{dir}/"));
        for entry in self
            .files
            .iter()
            .filter(|entry| matches_filter(entry, self.status_filter))
        {
            match scope {
                GroupScope::Any => {}
                GroupScope::TrackedOnly => {
                    if is_untracked_status(&entry.status) {
                        continue;
                    }
                }
                GroupScope::UntrackedOnly => {
                    if !is_untracked_status(&entry.status) {
                        continue;
                    }
                }
            }

            if let Some(prefix) = prefix.as_deref() {
                if !entry.path.starts_with(prefix) {
                    continue;
                }
            }

            if select {
                self.selected_for_commit.insert(entry.path.clone());
            } else {
                self.selected_for_commit.remove(&entry.path);
            }
        }
        self.invalidate_changes_tree();
    }

    fn directory_selection_counts(&self, dir_path: &str) -> (usize, usize) {
        #[derive(Clone, Copy)]
        enum GroupScope {
            Any,
            TrackedOnly,
            UntrackedOnly,
        }

        let (scope, dir_prefix) = if dir_path == ":group:changes" {
            (GroupScope::TrackedOnly, None)
        } else if dir_path == ":group:untracked" {
            (GroupScope::UntrackedOnly, None)
        } else if let Some(rest) = dir_path.strip_prefix(":group:changes/") {
            (GroupScope::TrackedOnly, Some(rest))
        } else if let Some(rest) = dir_path.strip_prefix(":group:untracked/") {
            (GroupScope::UntrackedOnly, Some(rest))
        } else {
            (GroupScope::Any, Some(dir_path))
        };

        let prefix = dir_prefix.map(|dir| format!("{dir}/"));
        let mut total = 0usize;
        let mut selected = 0usize;

        for entry in self
            .files
            .iter()
            .filter(|entry| matches_filter(entry, self.status_filter))
        {
            if is_ignored_status(&entry.status) {
                continue;
            }

            match scope {
                GroupScope::Any => {}
                GroupScope::TrackedOnly => {
                    if is_untracked_status(&entry.status) {
                        continue;
                    }
                }
                GroupScope::UntrackedOnly => {
                    if !is_untracked_status(&entry.status) {
                        continue;
                    }
                }
            }

            if let Some(prefix) = prefix.as_deref() {
                if !entry.path.starts_with(prefix) {
                    continue;
                }
            }

            total += 1;
            if self.selected_for_commit.contains(&entry.path) {
                selected += 1;
            }
        }

        (total, selected)
    }

    fn rebuild_changes_tree(&mut self, files: &[FileEntry], cx: &mut Context<Self>) {
        let selected_path = self
            .diff_view
            .as_ref()
            .and_then(|view| view.path.clone())
            .or_else(|| {
                self.conflict_view
                    .as_ref()
                    .and_then(|view| view.path.clone())
            });

        let (root_items, meta) = build_changes_tree_items(
            files,
            &self.selected_for_commit,
            &mut self.changes_tree_item_cache,
        );
        self.changes_tree_root_items = root_items.clone();
        self.changes_tree_meta = meta;

        let selection_ix = selected_path
            .as_deref()
            .and_then(|id| flat_tree_index(&root_items, id));

        self.changes_tree.update(cx, |state, cx| {
            state.set_items(root_items, cx);
            if let Some(ix) = selection_ix {
                state.set_selected_index(Some(ix), cx);
                state.scroll_to_item(ix, gpui::ScrollStrategy::Center);
            }
        });
    }

    #[cfg(any())]
    fn render_changes_tree_row(
        &mut self,
        index: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let selected_path = self
            .diff_view
            .as_ref()
            .and_then(|view| view.path.clone())
            .or_else(|| {
                self.conflict_view
                    .as_ref()
                    .and_then(|view| view.path.clone())
            });

        let Some(row) = self.changes_tree_rows.get(index).cloned() else {
            return div().h(px(24.)).into_any_element();
        };

        match row {
            ChangeTreeRow::Directory {
                path,
                name,
                depth,
                file_count,
                selected_count,
                collapsed,
            } => {
                let row_id = stable_hash_for_id(&path);
                let indent_width = px(8. + (depth as f32) * 12.);
                let checkbox_state = TriState::for_counts(file_count, selected_count);
                let checkbox_disabled = file_count == 0 || !self.git_available || self.loading;
                let app = cx.entity();
                let menu_path = path.clone();
                let menu_name = name.clone();
                let menu_file_count = file_count;
                let menu_selected_count = selected_count;
                let menu_collapsed = collapsed;
                let reveal_path = (!path.starts_with(':')).then(|| self.repo_root.join(&path));
                let select_path_for_click = path.clone();
                div()
                    .id(("changes-dir-row", row_id))
                    .flex()
                    .flex_row()
                    .items_center()
                    .h(px(24.))
                    .px(px(4.))
                    .rounded(px(4.))
                    .hover(|this| this.bg(theme.list_hover))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, {
                        let path = path.clone();
                        cx.listener(move |this, _, _window, cx| {
                            this.toggle_changes_dir(&path);
                            cx.notify();
                        })
                    })
                    .child(div().w(indent_width).flex_none())
                    .child(
                        div()
                            .w(px(12.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(theme.muted_foreground)
                            .child(
                                Icon::new(if collapsed {
                                    IconName::ChevronRight
                                } else {
                                    IconName::ChevronDown
                                })
                                .xsmall(),
                            ),
                    )
                    .child(
                        div()
                            .w(px(18.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .on_mouse_down(MouseButton::Left, |_, _window, cx| {
                                cx.stop_propagation()
                            })
                            .child(tristate_checkbox(
                                ("changes-dir-select", row_id),
                                checkbox_state,
                                checkbox_disabled,
                                theme,
                                {
                                    cx.listener(move |this, _, _window, cx| {
                                        let (total, selected) =
                                            this.directory_selection_counts(&select_path_for_click);
                                        if total == 0 {
                                            return;
                                        }
                                        let select = selected < total;
                                        this.toggle_directory_selection(
                                            select_path_for_click.clone(),
                                            select,
                                        );
                                        cx.notify();
                                    })
                                },
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.))
                            .min_w(px(0.))
                            .child(
                                div()
                                    .w(px(16.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(theme.muted_foreground)
                                    .child(
                                        Icon::new(if collapsed {
                                            IconName::FolderClosed
                                        } else {
                                            IconName::FolderOpen
                                        })
                                        .xsmall(),
                                    ),
                            )
                            .child(div().truncate().child(name))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("{file_count} files")),
                            ),
                    )
                    .context_menu(move |menu: PopupMenu, _window: &mut Window, _cx| {
                        let select = menu_selected_count < menu_file_count;
                        let app_for_toggle = app.clone();
                        let dir_for_toggle = menu_path.clone();
                        let toggle_label: SharedString = if menu_collapsed {
                            "展开".into()
                        } else {
                            "折叠".into()
                        };

                        let app_for_select = app.clone();
                        let dir_for_select = menu_path.clone();
                        let select_label: SharedString = if select {
                            format!("选择该目录（{menu_selected_count}/{menu_file_count}）").into()
                        } else {
                            format!("取消选择该目录（{menu_selected_count}/{menu_file_count}）")
                                .into()
                        };

                        let mut menu = menu
                            .item(
                                PopupMenuItem::new(toggle_label)
                                    .icon(
                                        Icon::new(if menu_collapsed {
                                            IconName::ChevronRight
                                        } else {
                                            IconName::ChevronDown
                                        })
                                        .xsmall(),
                                    )
                                    .on_click(move |_, _window, cx| {
                                        app_for_toggle.update(cx, |this, cx| {
                                            this.toggle_changes_dir(&dir_for_toggle);
                                            cx.notify();
                                        });
                                    }),
                            )
                            .item(
                                PopupMenuItem::new(select_label)
                                    .icon(Icon::new(IconName::Check).xsmall())
                                    .disabled(menu_file_count == 0)
                                    .on_click(move |_, _window, cx| {
                                        app_for_select.update(cx, |this, cx| {
                                            this.toggle_directory_selection(
                                                dir_for_select.clone(),
                                                select,
                                            );
                                            cx.notify();
                                        });
                                    }),
                            );

                        if !menu_path.starts_with(':') && !menu_name.trim().is_empty() {
                            let path_for_copy = menu_path.clone();
                            menu = menu.item(
                                PopupMenuItem::new("复制目录路径")
                                    .icon(Icon::new(IconName::Copy).xsmall())
                                    .on_click(move |_, _window, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            path_for_copy.clone(),
                                        ));
                                    }),
                            );
                        }

                        if let Some(reveal_path) = reveal_path.clone() {
                            let app_for_reveal = app.clone();
                            menu = menu.item(
                                PopupMenuItem::new("在文件管理器中显示")
                                    .icon(Icon::new(IconName::FolderOpen).xsmall())
                                    .on_click(move |_, window, cx| {
                                        app_for_reveal.update(cx, |this, cx| {
                                            this.reveal_in_file_manager(
                                                reveal_path.clone(),
                                                window,
                                                cx,
                                            );
                                        });
                                    }),
                            );
                        }

                        menu
                    })
                    .into_any_element()
            }
            ChangeTreeRow::File { entry, depth } => {
                let path = entry.path.clone();
                let status = entry.status.clone();
                let entry_for_open_click = entry.clone();
                let entry_for_open_menu = entry.clone();
                let path_for_select = path.clone();
                let row_id = stable_hash_for_id(&path);
                let is_selected = selected_path.as_deref() == Some(path.as_str());
                let name = path.rsplit('/').next().unwrap_or(path.as_str()).to_string();
                let rename_hint = entry
                    .orig_path
                    .as_deref()
                    .and_then(|path| path.rsplit('/').next())
                    .map(|name| format!("← {name}"));
                // 文件行本身没有展开箭头，但为了与目录行对齐（目录行在缩进后会占用 12px 的箭头列），
                // 这里仍然保留一个 12px 的占位列，因此缩进宽度应与目录行保持一致。
                let indent_width = px(8. + (depth as f32) * 12.);
                let checkbox_checked = self.selected_for_commit.contains(&path);
                let checkbox_disabled =
                    !self.git_available || self.loading || is_ignored_status(&status);
                let can_stage = self.git_available
                    && !self.loading
                    && !is_conflict_status(&status)
                    && (is_untracked_status(&status)
                        || status_xy(&status).is_some_and(|(_, y)| is_modified_status_code(y)));
                let can_unstage = self.git_available
                    && !self.loading
                    && !is_conflict_status(&status)
                    && status_xy(&status).is_some_and(|(x, _)| is_modified_status_code(x));
                let app = cx.entity();
                let repo_root = self.repo_root.clone();
                let abs_path_buf = repo_root.join(&path);
                let abs_path = abs_path_buf.display().to_string();

                div()
                    .id(("changes-file-row", row_id))
                    .flex()
                    .flex_row()
                    .items_center()
                    .h(px(24.))
                    .px(px(4.))
                    .rounded(px(4.))
                    .when(is_selected, |this| this.bg(theme.list_active))
                    .when(!is_selected, |this| {
                        this.hover(|this| this.bg(theme.list_hover))
                    })
                    .cursor_pointer()
                    .child(div().w(indent_width).flex_none())
                    .child(div().w(px(12.)).flex_none())
                    .child(
                        div()
                            .w(px(18.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Checkbox::new(("changes-file-select", row_id))
                                    .checked(checkbox_checked)
                                    .disabled(checkbox_disabled)
                                    .tab_stop(false)
                                    .on_click({
                                        let app = cx.entity();
                                        move |checked, _window, cx| {
                                            app.update(cx, |this, cx| {
                                                if *checked {
                                                    this.selected_for_commit
                                                        .insert(path_for_select.clone());
                                                } else {
                                                    this.selected_for_commit
                                                        .remove(&path_for_select);
                                                }
                                                this.invalidate_changes_tree();
                                                cx.notify();
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
                            .gap(px(6.))
                            .flex_1()
                            .min_w(px(0.))
                            .on_mouse_down(MouseButton::Left, {
                                cx.listener(move |this, _, window, cx| {
                                    this.open_file(entry_for_open_click.clone(), window, cx);
                                    cx.notify();
                                })
                            })
                            .child(
                                div()
                                    .w(px(16.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(render_status_badge(
                                        ("changes-file-status", row_id),
                                        &status,
                                        theme,
                                    )),
                            )
                            .child({
                                let name = name.clone();
                                let Some(hint) = rename_hint.clone() else {
                                    return div().truncate().child(name).into_any_element();
                                };
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(6.))
                                    .min_w(px(0.))
                                    .child(div().flex_1().min_w(px(0.)).truncate().child(name))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .truncate()
                                            .child(hint),
                                    )
                                    .into_any_element()
                            }),
                    )
                    .context_menu(move |menu: PopupMenu, _window: &mut Window, _cx| {
                        let app_for_open = app.clone();
                        let entry_for_open = entry_for_open_menu.clone();
                        let menu = menu
                            .item(
                                PopupMenuItem::new("打开 Diff")
                                    .icon(Icon::new(IconName::File).xsmall())
                                    .on_click(move |_, window, cx| {
                                        app_for_open.update(cx, |this, cx| {
                                            this.open_file(entry_for_open.clone(), window, cx);
                                            cx.notify();
                                        });
                                    }),
                            )
                            .item(PopupMenuItem::separator());

                        let app_for_stage = app.clone();
                        let entry_for_stage = entry_for_open_menu.clone();
                        let menu = menu.item(
                            PopupMenuItem::new("Stage 文件")
                                .icon(Icon::new(IconName::Plus).xsmall())
                                .disabled(!can_stage)
                                .on_click(move |_, window, cx| {
                                    app_for_stage.update(cx, |this, cx| {
                                        this.stage_path(entry_for_stage.clone(), window, cx);
                                        cx.notify();
                                    });
                                }),
                        );

                        let app_for_unstage = app.clone();
                        let entry_for_unstage = entry_for_open_menu.clone();
                        let menu = menu.item(
                            PopupMenuItem::new("Unstage 文件")
                                .icon(Icon::new(IconName::Minus).xsmall())
                                .disabled(!can_unstage)
                                .on_click(move |_, window, cx| {
                                    app_for_unstage.update(cx, |this, cx| {
                                        this.unstage_path(entry_for_unstage.clone(), window, cx);
                                        cx.notify();
                                    });
                                }),
                        );

                        let is_rename_or_copy = status_xy(&status)
                            .is_some_and(|(x, y)| matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C'));
                        let can_rollback = app.read(_cx).git_available
                            && !app.read(_cx).loading
                            && !is_conflict_status(&status)
                            && !is_rename_or_copy;
                        let rollback_message: SharedString = if is_untracked_status(&status) {
                            "将从磁盘删除该未跟踪文件。".into()
                        } else if status.contains('A') {
                            "将删除该新文件，并移除暂存区记录（如有）。".into()
                        } else {
                            "将把该文件恢复到 HEAD 版本（同时丢弃暂存与工作区改动）。".into()
                        };
                        let rollback_ok_text: SharedString = if is_untracked_status(&status) {
                            "Delete".into()
                        } else {
                            "Rollback".into()
                        };

                        let app_for_rollback = app.clone();
                        let rollback_path = path.clone();
                        let rollback_status = status.clone();
                        let rollback_message_for_dialog = rollback_message.clone();
                        let rollback_ok_text_for_dialog = rollback_ok_text.clone();

                        let menu = menu
                            .item(PopupMenuItem::separator())
                            .item(
                                PopupMenuItem::new("Rollback…")
                                    .icon(Icon::new(PlateIconName::Undo2).xsmall())
                                    .disabled(!can_rollback)
                                    .on_click(move |_, window, cx| {
                                        let app_for_ok = app_for_rollback.clone();
                                        let path_for_ok = rollback_path.clone();
                                        let status_for_ok = rollback_status.clone();
                                        let message = rollback_message_for_dialog.clone();
                                        let ok_text = rollback_ok_text_for_dialog.clone();

                                        window.open_dialog(cx, move |dialog, _window, cx| {
                                            let theme = cx.theme();
                                            let path_label = path_for_ok.clone();
                                            let status_label = status_for_ok.clone();
                                            let action_path = path_for_ok.clone();
                                            let action_status = status_for_ok.clone();
                                            let app_for_ok = app_for_ok.clone();
                                            dialog
                                                .confirm()
                                                .title(
                                                    div()
                                                        .text_sm()
                                                        .font_semibold()
                                                        .child("Rollback changes?"),
                                                )
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
                                                                .font_family(
                                                                    theme.mono_font_family.clone(),
                                                                )
                                                                .child(format!(
                                                                    "{status_label} {path_label}"
                                                                )),
                                                        )
                                                        .child(
                                                            div().text_sm().child(message.clone()),
                                                        )
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
                                    }),
                            )
                            .item(PopupMenuItem::separator());

                        let app_for_reveal = app.clone();
                        let reveal_path = abs_path_buf.clone();
                        let menu = menu.item(
                            PopupMenuItem::new("在文件管理器中显示")
                                .icon(Icon::new(IconName::FolderOpen).xsmall())
                                .on_click(move |_, window, cx| {
                                    app_for_reveal.update(cx, |this, cx| {
                                        this.reveal_in_file_manager(
                                            reveal_path.clone(),
                                            window,
                                            cx,
                                        );
                                    });
                                }),
                        );

                        let path_for_copy = path.clone();
                        let abs_for_copy = abs_path.clone();
                        menu.item(
                            PopupMenuItem::new("复制相对路径")
                                .icon(Icon::new(IconName::Copy).xsmall())
                                .on_click(move |_, _window, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        path_for_copy.clone(),
                                    ));
                                }),
                        )
                        .item(
                            PopupMenuItem::new("复制绝对路径")
                                .icon(Icon::new(IconName::Copy).xsmall())
                                .on_click(move |_, _window, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        abs_for_copy.clone(),
                                    ));
                                }),
                        )
                    })
                    .into_any_element()
            }
        }
    }

    fn commit_from_panel(&mut self, push_after: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.git_available {
            window.push_notification(
                Notification::new().message("未检测到 git 命令，无法提交"),
                cx,
            );
            return;
        }

        if self.committing {
            return;
        }

        let message = self.commit_message_input.read(cx).value().to_string();
        if message.trim().is_empty() {
            window.push_notification(Notification::new().message("Commit message 不能为空"), cx);
            return;
        }

        let counts = StatusCounts::from_entries(&self.files);
        if counts.conflicts > 0 {
            window.push_notification(
                Notification::new().message("存在冲突文件，暂不能提交（请先解决冲突）"),
                cx,
            );
            return;
        }

        let mut selected_paths: Vec<String> = self.selected_for_commit.iter().cloned().collect();
        selected_paths.sort();
        if selected_paths.is_empty() && !self.commit_amend {
            window.push_notification(Notification::new().message("请先勾选要提交的文件"), cx);
            return;
        }

        let mut commit_pathspecs = selected_paths.clone();
        if !commit_pathspecs.is_empty() {
            for entry in &self.files {
                if selected_paths.iter().any(|path| path == &entry.path) {
                    if let Some(orig_path) = entry.orig_path.as_deref() {
                        let orig_path = orig_path.trim();
                        if !orig_path.is_empty() && orig_path != entry.path {
                            commit_pathspecs.push(orig_path.to_string());
                        }
                    }
                }
            }
            commit_pathspecs.sort();
            commit_pathspecs.dedup();
        }

        let amend = self.commit_amend;
        self.committing = true;
        cx.notify();

        window.push_notification(
            Notification::new().message(if amend {
                "正在提交（--amend）…".to_string()
            } else if push_after {
                "正在提交并 Push…".to_string()
            } else {
                "正在提交…".to_string()
            }),
            cx,
        );

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let files_snapshot = self.files.clone();
        cx.spawn_in(window, async move |_, window| {
            let repo_root_bg = repo_root.clone();
            let message_bg = message.clone();
            let (commit_result, push_result, status_result) = window
                .background_executor()
                .spawn(async move {
                    if !selected_paths.is_empty() {
                        let mut untracked_paths = Vec::new();
                        for entry in &files_snapshot {
                            if selected_paths.iter().any(|path| path == &entry.path)
                                && is_untracked_status(&entry.status)
                            {
                                untracked_paths.push(entry.path.clone());
                            }
                        }

                        if !untracked_paths.is_empty() {
                            let mut add_args: Vec<String> = vec!["add".into(), "--".into()];
                            add_args.extend(untracked_paths);
                            if let Err(err) = run_git(repo_root_bg.as_path(), add_args) {
                                let status_result = fetch_git_status(&repo_root_bg);
                                return (Err(err), None, status_result);
                            }
                        }
                    }

                    let mut commit_args: Vec<String> =
                        vec!["commit".into(), "-F".into(), "-".into()];
                    if amend {
                        commit_args.push("--amend".into());
                    }
                    if !selected_paths.is_empty() {
                        commit_args.push("--only".into());
                        commit_args.push("--".into());
                        commit_args.extend(commit_pathspecs.clone());
                    }

                    let commit_result =
                        run_git_with_stdin(repo_root_bg.as_path(), commit_args, &message_bg);
                    let push_result = if push_after && commit_result.is_ok() {
                        Some(run_git(repo_root_bg.as_path(), ["push"]))
                    } else {
                        None
                    };
                    let status_result = fetch_git_status(&repo_root_bg);
                    (commit_result, push_result, status_result)
                })
                .await;

            window
                .update(|window, cx| {
                    this.update(cx, |this, cx| {
                        this.clear_diff_cache();
                        this.committing = false;
                        if commit_result.is_ok() {
                            this.commit_amend = false;
                            this.selected_for_commit.clear();
                            this.commit_message_input.update(cx, |state, cx| {
                                state.set_value(String::new(), window, cx);
                            });
                        }

                        if let Ok(entries) = &status_result {
                            this.files = entries.clone();
                        }
                        this.invalidate_changes_tree();
                        cx.notify();
                    });

                    match &commit_result {
                        Ok(_) => {
                            window.push_notification(Notification::new().message("提交成功"), cx)
                        }
                        Err(err) => window.push_notification(
                            Notification::new().message(format!("提交失败：{err:#}")),
                            cx,
                        ),
                    }

                    if let Some(push_result) = push_result {
                        match push_result {
                            Ok(_) => window
                                .push_notification(Notification::new().message("Push 成功"), cx),
                            Err(err) => window.push_notification(
                                Notification::new().message(format!("Push 失败：{err:#}")),
                                cx,
                            ),
                        }
                    }

                    if let Err(err) = status_result {
                        window.push_notification(
                            Notification::new().message(format!("刷新 git 状态失败：{err:#}")),
                            cx,
                        );
                    }
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn prefill_amend_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.git_available {
            return;
        }

        let this = cx.entity();
        let repo_root = self.repo_root.clone();

        cx.spawn_in(window, async move |_, window| {
            let repo_root_bg = repo_root.clone();
            let message = window
                .background_executor()
                .spawn(async move { fetch_last_commit_message(&repo_root_bg) })
                .await;

            window
                .update(|window, cx| {
                    this.update(cx, |this, cx| {
                        if !this.commit_amend {
                            return;
                        }

                        match message {
                            Ok(message) => {
                                this.commit_message_input.update(cx, |state, cx| {
                                    state.set_value(message, window, cx);
                                });
                            }
                            Err(err) => window.push_notification(
                                Notification::new()
                                    .message(format!("读取上一条 commit message 失败：{err:#}")),
                                cx,
                            ),
                        }

                        cx.notify();
                    });
                })
                .ok();

            Some(())
        })
        .detach();
    }

    pub(super) fn refresh_git_status(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.git_available {
            window.push_notification(
                Notification::new().message("未检测到 git 命令，无法刷新状态"),
                cx,
            );
            return;
        }

        if self.loading {
            return;
        }

        self.clear_diff_cache();
        self.loading = true;
        cx.notify();

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        cx.spawn_in(window, async move |_, window| {
            let repo_root_bg = repo_root.clone();
            let (entries, branch_label) = window
                .background_executor()
                .spawn(async move {
                    let entries = fetch_git_status(&repo_root_bg)
                        .map_err(|err| {
                            eprintln!("git status failed: {err:?}");
                            err
                        })
                        .unwrap_or_default();
                    let branch_label = fetch_git_branch(&repo_root_bg).unwrap_or_else(|err| {
                        eprintln!("git branch detect failed: {err:?}");
                        "No Repo".to_string()
                    });
                    (entries, branch_label)
                })
                .await;

            window
                .update(|window, cx| {
                    this.update(cx, |this, cx| {
                        this.loading = false;
                        this.files = entries;
                        this.branch_name = branch_label;
                        let mut valid_paths = HashSet::new();
                        for entry in &this.files {
                            if !is_ignored_status(&entry.status) {
                                valid_paths.insert(entry.path.clone());
                            }
                        }
                        this.selected_for_commit
                            .retain(|path| valid_paths.contains(path));

                        let open_path = this
                            .diff_view
                            .as_ref()
                            .and_then(|view| view.path.clone())
                            .or_else(|| {
                                this.conflict_view
                                    .as_ref()
                                    .and_then(|view| view.path.clone())
                            });

                        if let Some(open_path) = open_path {
                            this.current_diff_cache_key = None;
                            let fallback_orig = this
                                .diff_view
                                .as_ref()
                                .and_then(|view| view.orig_path.clone());
                            let entry = this
                                .files
                                .iter()
                                .find(|entry| entry.path == open_path)
                                .cloned()
                                .unwrap_or(FileEntry {
                                    path: open_path,
                                    status: String::new(),
                                    orig_path: fallback_orig,
                                });

                            if is_conflict_status(&entry.status) {
                                this.open_file(entry, window, cx);
                            } else if let Some(diff_view) = this.diff_view.as_ref() {
                                let target = diff_view.compare_target.clone();
                                this.open_file_diff_with_target(entry, target, window, cx);
                            } else {
                                this.open_file(entry, window, cx);
                            }
                        }
                        this.invalidate_changes_tree();
                        cx.notify();
                    });
                    let title = this.read(cx).window_title();
                    window.set_window_title(&title);
                })
                .ok();

            Some(())
        })
        .detach();
    }

    pub(super) fn render_sidebar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let changes_panel = self.render_changes_panel(window, cx);
        let commit_panel = self.render_commit_panel(window, cx);
        let theme = cx.theme();

        div()
            .id("git-viewer-sidebar")
            .flex()
            .flex_col()
            .size_full()
            .min_w(px(0.))
            .bg(theme.background)
            .child(
                v_resizable("sidebar-split")
                    .child(resizable_panel().child(changes_panel.size_full()))
                    .child(
                        resizable_panel()
                            .size(px(260.))
                            .size_range(px(160.)..px(720.))
                            .child(
                                div()
                                    .size_full()
                                    .bg(theme.background)
                                    .child(commit_panel.size_full()),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_changes_panel(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let counts = StatusCounts::from_entries(&self.files);
        let filtered_len = self
            .files
            .iter()
            .filter(|entry| matches_filter(entry, self.status_filter))
            .count();

        let selected_index = match self.status_filter {
            StatusFilter::All => 0,
            StatusFilter::Conflicts => 1,
            StatusFilter::Staged => 2,
            StatusFilter::Unstaged => 3,
            StatusFilter::Untracked => 4,
        };
        let app = cx.entity();
        fn filter_icon(icon: impl IntoElement) -> Div {
            div()
                .w(px(18.))
                .h(px(18.))
                .flex()
                .items_center()
                .justify_center()
                .child(icon)
        }

        let filter_bar = AnimatedSegmented::new("changes-filter-tabs")
            .small()
            .w(px(152.))
            .selected_index(selected_index)
            .on_click(move |ix, _window, cx| {
                let filter = match *ix {
                    0 => StatusFilter::All,
                    1 => StatusFilter::Conflicts,
                    2 => StatusFilter::Staged,
                    3 => StatusFilter::Unstaged,
                    4 => StatusFilter::Untracked,
                    _ => StatusFilter::All,
                };
                app.update(cx, |this, cx| {
                    this.status_filter = filter;
                    this.invalidate_changes_tree();
                    cx.notify();
                });
            })
            .items(vec![
                AnimatedSegmentedItem::new()
                    .icon(filter_icon(
                        Icon::new(PlateIconName::Layers).with_size(px(16.)),
                    ))
                    .tooltip(format!("All {}", counts.all)),
                AnimatedSegmentedItem::new()
                    .icon(filter_icon(
                        Icon::new(PlateIconName::SendToBack).with_size(px(16.)),
                    ))
                    .tooltip(format!("Conflicts {}", counts.conflicts)),
                AnimatedSegmentedItem::new()
                    .icon(filter_icon(
                        Icon::new(PlateIconName::LayersPlus).with_size(px(16.)),
                    ))
                    .tooltip(format!("Staged {}", counts.staged)),
                AnimatedSegmentedItem::new()
                    .icon(filter_icon(
                        Icon::new(PlateIconName::Layers2).with_size(px(16.)),
                    ))
                    .tooltip(format!("Unstaged {}", counts.unstaged)),
                AnimatedSegmentedItem::new()
                    .icon(filter_icon(
                        Icon::new(PlateIconName::CirclePile).with_size(px(16.)),
                    ))
                    .tooltip(format!("Untracked {}", counts.untracked)),
            ]);

        let can_refresh = self.git_available && !self.loading;
        let can_shelve = can_refresh && counts.all > 0;
        let can_expand_tree = !self.loading && filtered_len > 0;
        let can_collapse_tree = !self.loading && filtered_len > 0;
        let can_rollback = can_refresh
            && self
                .changes_tree
                .read(cx)
                .selected_entry()
                .is_some_and(|entry| {
                    let id = entry.item().id.clone();
                    self.changes_tree_meta
                        .get(&id)
                        .is_some_and(|meta| match meta {
                            ChangesTreeMeta::Directory { file_count, .. } => *file_count > 0,
                            ChangesTreeMeta::File { entry } => {
                                !is_ignored_status(&entry.status)
                                    && !is_conflict_status(&entry.status)
                            }
                        })
                });

        let tree_view: AnyElement = if self.loading {
            self.changes_tree_built_revision = 0;
            self.changes_tree_meta.clear();
            self.changes_tree
                .update(cx, |state, cx| state.set_items(Vec::new(), cx));
            div()
                .px(px(10.))
                .py(px(8.))
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("加载中…")
                .into_any_element()
        } else {
            let filtered_files: Vec<FileEntry> = self
                .files
                .iter()
                .filter(|entry| matches_filter(entry, self.status_filter))
                .cloned()
                .collect();

            if self.changes_tree_built_revision != self.changes_tree_revision {
                self.rebuild_changes_tree(&filtered_files, cx);
                self.changes_tree_built_revision = self.changes_tree_revision;
            }

            if filtered_files.is_empty() {
                div()
                    .px(px(10.))
                    .py(px(8.))
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("没有变更文件")
                    .into_any_element()
            } else {
                let app = cx.entity();
                let tree_state = self.changes_tree.clone();
                div()
                    .id("changes-tree-viewport")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .rounded(theme.radius)
                    .border_1()
                    .border_color(theme.border.alpha(0.6))
                    .overflow_hidden()
                    .child(gpui_component::tree::tree(
                        &tree_state,
                        move |ix, entry, selected, window, cx| {
                            render_changes_tree_item(app.clone(), ix, entry, selected, window, cx)
                        },
                    ))
                    .into_any_element()
            }
        };

        let compare_control = self.render_compare_target_control(cx);
        let app = cx.entity();

        let toolbar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .w_full()
            .min_w(px(0.))
            .overflow_hidden()
            .child(div().flex_none().child(filter_bar))
            .child(div().flex_none().child(compare_control))
            .child(
                div().flex_none().child(
                    Button::new("changes-toolbar-refresh")
                        .icon(PlateIconName::FolderSync)
                        .ghost()
                        .small()
                        .compact()
                        .disabled(!can_refresh)
                        .tooltip("Refresh")
                        .on_click({
                            let app = app.clone();
                            move |_, window, cx| {
                                app.update(cx, |this, cx| {
                                    this.refresh_git_status(window, cx);
                                    cx.notify();
                                });
                            }
                        }),
                ),
            )
            .child(
                div().flex_none().child(
                    Button::new("changes-toolbar-rollback")
                        .icon(PlateIconName::Undo2)
                        .ghost()
                        .small()
                        .compact()
                        .disabled(!can_rollback)
                        .tooltip("Rollback")
                        .on_click({
                            let app = app.clone();
                            move |_, window, cx| {
                                app.update(cx, |this, cx| {
                                    this.open_rollback_selected_dialog(window, cx);
                                    cx.notify();
                                });
                            }
                        }),
                ),
            )
            .child(
                div().flex_none().child(
                    Button::new("changes-toolbar-shelve")
                        .icon(PlateIconName::Download)
                        .ghost()
                        .small()
                        .compact()
                        .disabled(!can_shelve)
                        .tooltip("Shelve silently")
                        .on_click({
                            let app = app.clone();
                            move |_, window, cx| {
                                app.update(cx, |this, cx| {
                                    this.shelve_silently(window, cx);
                                    cx.notify();
                                });
                            }
                        }),
                ),
            )
            .child(
                div().flex_none().child(
                    Button::new("changes-toolbar-expand-all")
                        .icon(PlateIconName::ChevronsUpDown)
                        .ghost()
                        .small()
                        .compact()
                        .disabled(!can_expand_tree)
                        .tooltip("Expand all")
                        .on_click({
                            let app = app.clone();
                            move |_, _window, cx| {
                                app.update(cx, |this, cx| {
                                    this.set_changes_tree_expanded(true, cx);
                                    cx.notify();
                                });
                            }
                        }),
                ),
            )
            .child(
                div().flex_none().child(
                    Button::new("changes-toolbar-collapse-all")
                        .icon(PlateIconName::ChevronsDownUp)
                        .ghost()
                        .small()
                        .compact()
                        .disabled(!can_collapse_tree)
                        .tooltip("Collapse all")
                        .on_click({
                            let app = app.clone();
                            move |_, _window, cx| {
                                app.update(cx, |this, cx| {
                                    this.set_changes_tree_expanded(false, cx);
                                    cx.notify();
                                });
                            }
                        }),
                ),
            );

        div()
            .flex()
            .flex_col()
            .w_full()
            .min_w(px(0.))
            .min_h(px(0.))
            .p(px(12.))
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .min_w(px(0.))
                    .child(toolbar),
            )
            .child(tree_view)
    }

    fn render_commit_panel(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let git_available = self.git_available;
        let committing = self.committing;
        let counts = StatusCounts::from_entries(&self.files);
        let selected_count = self.selected_for_commit.len();

        let can_commit = git_available
            && !committing
            && counts.conflicts == 0
            && (selected_count > 0 || self.commit_amend);
        let commit_label: SharedString = if !git_available {
            "Commit（git 不可用）".into()
        } else if committing {
            "Commit（提交中…）".into()
        } else if counts.conflicts > 0 {
            "Commit（存在冲突）".into()
        } else {
            "Commit".into()
        };

        let commit_btn = Button::new("commit-button")
            .label(commit_label)
            .primary()
            .small()
            .disabled(!can_commit)
            .on_click(cx.listener(|this, _, window, cx| {
                this.commit_from_panel(false, window, cx);
                cx.notify();
            }));

        let outline_variant = gpui_component::button::ButtonCustomVariant::new(cx)
            .color(theme.background)
            .foreground(theme.foreground)
            .border(theme.border)
            .hover(theme.list_hover)
            .active(theme.list_active)
            .shadow(false);

        let commit_and_push_btn = Button::new("commit-push-button")
            .label("Commit and Push…")
            .custom(outline_variant)
            .small()
            .disabled(!can_commit)
            .on_click(cx.listener(|this, _, window, cx| {
                this.commit_from_panel(true, window, cx);
                cx.notify();
            }));

        div()
            .flex()
            .flex_col()
            .text_sm()
            .w_full()
            .min_w(px(0.))
            .p(px(12.))
            .gap(px(8.))
            // .child(
            //     div()
            //         .flex()
            //         .flex_row()
            //         .items_center()
            //         .justify_between()
            //         .gap(px(8.))
            //         .child(div().text_sm().child("Commit"))
            //         .child(
            //             Button::new("commit-info")
            //                 .icon(IconName::Info)
            //                 .ghost()
            //                 .small()
            //                 .compact()
            //                 .tooltip(
            //                     "Commit 只提交勾选的文件（使用 git commit --only）；Diff 里的 Stage/Unstage 依然可用",
            //                 )
            //                 .on_click(|_, _, _| {}),
            //         )
            // )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        Checkbox::new("commit-amend")
                            .label("Amend")
                            .checked(self.commit_amend)
                            .small()
                            .disabled(!git_available || committing)
                            .on_click({
                                let app = cx.entity();
                                move |checked, window, cx| {
                                    let checked = *checked;
                                    app.update(cx, |this, cx| {
                                        this.commit_amend = checked;
                                        if checked {
                                            this.prefill_amend_message(window, cx);
                                        }
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("Selected {selected_count}")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .border_1()
                    .border_color(theme.border.alpha(0.6))
                    .rounded(theme.radius)
                    .p(px(4.))
                    .child(
                        Input::new(&self.commit_message_input)
                            .h_full()
                            .w_full()
                            .small(),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(commit_btn)
                    .child(commit_and_push_btn),
            )
    }
}

fn render_changes_tree_item(
    app: Entity<GitViewerApp>,
    ix: usize,
    entry: &gpui_component::tree::TreeEntry,
    selected: bool,
    _window: &mut Window,
    cx: &mut App,
) -> ListItem {
    let theme = cx.theme();
    let id = entry.item().id.clone();
    let indent_width = px(4.) + px(12.) * entry.depth();

    let (meta, git_available, loading, repo_root, checkbox_checked) = {
        let this = app.read(cx);
        let meta = this.changes_tree_meta.get(&id).cloned();
        let checkbox_checked = this.selected_for_commit.contains(id.as_str());
        (
            meta,
            this.git_available,
            this.loading,
            this.repo_root.clone(),
            checkbox_checked,
        )
    };

    match meta {
        Some(ChangesTreeMeta::Directory {
            path,
            name,
            file_count,
            selected_count,
        }) => {
            let is_group = path == ":group:changes" || path == ":group:untracked";
            let row_id = stable_hash_for_id(&path);
            let checkbox_state = TriState::for_counts(file_count, selected_count);
            let checkbox_disabled = file_count == 0 || !git_available || loading;
            let expanded = entry.is_expanded();
            let toggle_id = id.clone();
            let dir_path_for_checkbox = path.clone();
            let dir_path_for_menu = path.clone();
            let dir_name_for_menu = name.clone();
            let reveal_path = (!path.starts_with(':')).then(|| repo_root.join(&path));

            let row = div()
                .flex()
                .flex_row()
                .items_center()
                .min_w(px(0.))
                .w_full()
                .child(div().w(indent_width).flex_none())
                .child(
                    div()
                        .w(px(10.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(theme.muted_foreground)
                        .child(
                            Icon::new(if expanded {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .xsmall(),
                        ),
                )
                .child(
                    div()
                        .w(px(16.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_mouse_down(MouseButton::Left, |_, window, cx| {
                            window.prevent_default();
                            cx.stop_propagation();
                        })
                        .child(tristate_checkbox(
                            ("changes-dir-select", row_id),
                            checkbox_state,
                            checkbox_disabled,
                            theme,
                            {
                                let app = app.clone();
                                let select_path = dir_path_for_checkbox.clone();
                                move |_, window, cx| {
                                    window.prevent_default();
                                    cx.stop_propagation();
                                    app.update(cx, |this, cx| {
                                        let (total, selected) =
                                            this.directory_selection_counts(&select_path);
                                        if total == 0 {
                                            return;
                                        }
                                        let select = selected < total;
                                        this.toggle_directory_selection(
                                            select_path.clone(),
                                            select,
                                        );
                                        cx.notify();
                                    });
                                }
                            },
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.))
                        .min_w(px(0.))
                        .when(!is_group, |this| {
                            this.child(
                                div()
                                    .w(px(16.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(if selected {
                                        theme.foreground
                                    } else {
                                        theme.muted_foreground
                                    })
                                    .child(
                                        Icon::new(if expanded {
                                            IconName::FolderOpen
                                        } else {
                                            IconName::FolderClosed
                                        })
                                        .xsmall(),
                                    ),
                            )
                        })
                        .child(div().truncate().child(name))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!("{file_count} files")),
                        ),
                )
                .context_menu(move |menu: PopupMenu, _window: &mut Window, _cx| {
                    let expanded_label: SharedString = if expanded {
                        "折叠".into()
                    } else {
                        "展开".into()
                    };
                    let app_for_toggle = app.clone();
                    let toggle_id = toggle_id.clone();
                    let mut menu = menu.item(
                        PopupMenuItem::new(expanded_label)
                            .icon(
                                Icon::new(if expanded {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
                                .xsmall(),
                            )
                            .on_click(move |_, _window, cx| {
                                app_for_toggle.update(cx, |this, cx| {
                                    if let Some(item) = this.changes_tree_item_cache.get(&toggle_id)
                                    {
                                        let new_expanded = !item.is_expanded();
                                        let _ = item.clone().expanded(new_expanded);
                                    }
                                    this.invalidate_changes_tree();
                                    cx.notify();
                                });
                            }),
                    );

                    let select = selected_count < file_count;
                    let select_label: SharedString = if select {
                        format!("选择该目录（{selected_count}/{file_count}）").into()
                    } else {
                        format!("取消选择该目录（{selected_count}/{file_count}）").into()
                    };
                    let app_for_select = app.clone();
                    let dir_for_select = dir_path_for_menu.clone();
                    menu = menu.item(
                        PopupMenuItem::new(select_label)
                            .icon(Icon::new(IconName::Check).xsmall())
                            .disabled(file_count == 0)
                            .on_click(move |_, _window, cx| {
                                app_for_select.update(cx, |this, cx| {
                                    this.toggle_directory_selection(dir_for_select.clone(), select);
                                    cx.notify();
                                });
                            }),
                    );

                    if !dir_path_for_menu.starts_with(':') && !dir_name_for_menu.trim().is_empty() {
                        let path_for_copy = dir_path_for_menu.clone();
                        menu = menu.item(
                            PopupMenuItem::new("复制目录路径")
                                .icon(Icon::new(IconName::Copy).xsmall())
                                .on_click(move |_, _window, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        path_for_copy.clone(),
                                    ));
                                }),
                        );
                    }

                    if let Some(reveal_path) = reveal_path.clone() {
                        let app_for_reveal = app.clone();
                        menu = menu.item(
                            PopupMenuItem::new("在文件管理器中显示")
                                .icon(Icon::new(IconName::FolderOpen).xsmall())
                                .on_click(move |_, window, cx| {
                                    app_for_reveal.update(cx, |this, cx| {
                                        this.reveal_in_file_manager(
                                            reveal_path.clone(),
                                            window,
                                            cx,
                                        );
                                    });
                                }),
                        );
                    }

                    menu
                });

            ListItem::new(ix)
                .px(px(4.))
                .py(px(0.))
                .h(px(22.))
                .text_sm()
                .rounded(px(4.))
                .child(row)
        }
        Some(ChangesTreeMeta::File { entry: file_entry }) => {
            let path = file_entry.path.clone();
            let status = file_entry.status.clone();
            let row_id = stable_hash_for_id(&path);
            let rename_hint = file_entry
                .orig_path
                .as_deref()
                .and_then(|path| path.rsplit('/').next())
                .map(|name| format!("← {name}"));
            let checkbox_disabled = !git_available || loading || is_ignored_status(&status);
            let can_stage = git_available
                && !loading
                && !is_conflict_status(&status)
                && (is_untracked_status(&status)
                    || status_xy(&status).is_some_and(|(_, y)| is_modified_status_code(y)));
            let can_unstage = git_available
                && !loading
                && !is_conflict_status(&status)
                && status_xy(&status).is_some_and(|(x, _)| is_modified_status_code(x));
            let abs_path_buf = repo_root.join(&path);
            let abs_path = abs_path_buf.display().to_string();
            let name = path.rsplit('/').next().unwrap_or(path.as_str()).to_string();
            let entry_for_open = file_entry.clone();
            let path_for_select = path.clone();

            let menu_app = app.clone();
            let row = div()
                .flex()
                .flex_row()
                .items_center()
                .min_w(px(0.))
                .w_full()
                .child(div().w(indent_width).flex_none())
                .child(div().w(px(10.)).flex_none())
                .child(
                    div()
                        .w(px(16.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_mouse_down(MouseButton::Left, |_, window, cx| {
                            window.prevent_default();
                            cx.stop_propagation();
                        })
                        .child(
                            Checkbox::new(("changes-file-select", row_id))
                                .small()
                                .checked(checkbox_checked)
                                .disabled(checkbox_disabled)
                                .tab_stop(false)
                                .on_click({
                                    let app = app.clone();
                                    move |checked, _window, cx| {
                                        app.update(cx, |this, cx| {
                                            if *checked {
                                                this.selected_for_commit
                                                    .insert(path_for_select.clone());
                                            } else {
                                                this.selected_for_commit.remove(&path_for_select);
                                            }
                                            this.invalidate_changes_tree();
                                            cx.notify();
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
                        .gap(px(4.))
                        .flex_1()
                        .min_w(px(0.))
                        .child(
                            div()
                                .w(px(16.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(render_status_badge(
                                    ("changes-file-status", row_id),
                                    &status,
                                    theme,
                                )),
                        )
                        .child(if let Some(hint) = rename_hint.clone() {
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(4.))
                                .min_w(px(0.))
                                .child(div().flex_1().min_w(px(0.)).truncate().child(name.clone()))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .truncate()
                                        .child(hint),
                                )
                                .into_any_element()
                        } else {
                            div().truncate().child(name.clone()).into_any_element()
                        }),
                )
                .context_menu(move |menu: PopupMenu, _window: &mut Window, _cx| {
                    let app_for_open = menu_app.clone();
                    let entry_for_open = file_entry.clone();
                    let menu = menu
                        .item(
                            PopupMenuItem::new("打开 Diff")
                                .icon(Icon::new(IconName::File).xsmall())
                                .on_click(move |_, window, cx| {
                                    app_for_open.update(cx, |this, cx| {
                                        this.open_file(entry_for_open.clone(), window, cx);
                                        cx.notify();
                                    });
                                }),
                        )
                        .item(PopupMenuItem::separator());

                    let app_for_stage = menu_app.clone();
                    let entry_for_stage = file_entry.clone();
                    let menu = menu.item(
                        PopupMenuItem::new("Stage 文件")
                            .icon(Icon::new(IconName::Plus).xsmall())
                            .disabled(!can_stage)
                            .on_click(move |_, window, cx| {
                                app_for_stage.update(cx, |this, cx| {
                                    this.stage_path(entry_for_stage.clone(), window, cx);
                                    cx.notify();
                                });
                            }),
                    );

                    let app_for_unstage = menu_app.clone();
                    let entry_for_unstage = file_entry.clone();
                    let menu = menu.item(
                        PopupMenuItem::new("Unstage 文件")
                            .icon(Icon::new(IconName::Minus).xsmall())
                            .disabled(!can_unstage)
                            .on_click(move |_, window, cx| {
                                app_for_unstage.update(cx, |this, cx| {
                                    this.unstage_path(entry_for_unstage.clone(), window, cx);
                                    cx.notify();
                                });
                            }),
                    );

                    let app_for_rollback = menu_app.clone();
                    let rollback_path = path.clone();
                    let rollback_status = status.clone();
                    let menu = menu.item(
                        PopupMenuItem::new(if is_untracked_status(&rollback_status) {
                            "删除未跟踪文件"
                        } else {
                            "Rollback（丢弃变更）"
                        })
                        .icon(Icon::new(PlateIconName::Undo2).xsmall())
                        .disabled(!git_available)
                        .on_click(move |_, window, cx| {
                            app_for_rollback.update(cx, |this, cx| {
                                this.rollback_path(
                                    rollback_path.clone(),
                                    rollback_status.clone(),
                                    window,
                                    cx,
                                );
                            });
                        }),
                    );

                    let menu = menu.item(PopupMenuItem::separator());

                    let app_for_reveal = menu_app.clone();
                    let reveal_path = abs_path_buf.clone();
                    let menu = menu.item(
                        PopupMenuItem::new("在文件管理器中显示")
                            .icon(Icon::new(IconName::FolderOpen).xsmall())
                            .on_click(move |_, window, cx| {
                                app_for_reveal.update(cx, |this, cx| {
                                    this.reveal_in_file_manager(reveal_path.clone(), window, cx);
                                });
                            }),
                    );

                    let path_for_copy = path.clone();
                    let abs_for_copy = abs_path.clone();
                    menu.item(
                        PopupMenuItem::new("复制相对路径")
                            .icon(Icon::new(IconName::Copy).xsmall())
                            .on_click(move |_, _window, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    path_for_copy.clone(),
                                ));
                            }),
                    )
                    .item(
                        PopupMenuItem::new("复制绝对路径")
                            .icon(Icon::new(IconName::Copy).xsmall())
                            .on_click(move |_, _window, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    abs_for_copy.clone(),
                                ));
                            }),
                    )
                });

            ListItem::new(ix)
                .px(px(4.))
                .py(px(0.))
                .h(px(22.))
                .text_sm()
                .rounded(px(4.))
                .on_click({
                    let app = app.clone();
                    move |_, window, cx| {
                        app.update(cx, |this, cx| {
                            this.open_file(entry_for_open.clone(), window, cx);
                            cx.notify();
                        });
                    }
                })
                .child(row)
        }
        None => ListItem::new(ix)
            .px(px(4.))
            .py(px(0.))
            .h(px(22.))
            .text_sm()
            .rounded(px(4.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .min_w(px(0.))
                    .child(div().w(indent_width).flex_none())
                    .when(entry.is_folder(), |this| {
                        let expanded = entry.is_expanded();
                        let id = entry.item().id.as_ref();
                        let is_group = id == ":group:changes" || id == ":group:untracked";
                        this.child(
                            div()
                                .w(px(10.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(theme.muted_foreground)
                                .child(
                                    Icon::new(if expanded {
                                        IconName::ChevronDown
                                    } else {
                                        IconName::ChevronRight
                                    })
                                    .xsmall(),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(4.))
                                .min_w(px(0.))
                                .when(!is_group, |this| {
                                    this.child(
                                        div()
                                            .w(px(16.))
                                            .flex_none()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_color(theme.muted_foreground)
                                            .child(
                                                Icon::new(if expanded {
                                                    IconName::FolderOpen
                                                } else {
                                                    IconName::FolderClosed
                                                })
                                                .xsmall(),
                                            ),
                                    )
                                })
                                .child(div().truncate().child(entry.item().label.clone())),
                        )
                    })
                    .when(!entry.is_folder(), |this| {
                        this.child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(4.))
                                .min_w(px(0.))
                                .child(
                                    div()
                                        .w(px(16.))
                                        .flex_none()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(theme.muted_foreground)
                                        .child(Icon::new(IconName::File).xsmall()),
                                )
                                .child(div().truncate().child(entry.item().label.clone())),
                        )
                    }),
            ),
    }
}

fn tristate_checkbox(
    id: impl Into<ElementId>,
    state: TriState,
    disabled: bool,
    theme: &gpui_component::Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let border_color = if matches!(state, TriState::Unchecked) {
        theme.input
    } else {
        theme.primary
    };
    let border_color = border_color.alpha(if disabled { 0.5 } else { 1.0 });
    let radius = theme.radius.min(px(4.));
    let icon = match state {
        TriState::Unchecked => None,
        TriState::Partial => Some(IconName::Minus),
        TriState::Checked => Some(IconName::Check),
    };

    div()
        .id(id)
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .size(px(14.))
        .border_1()
        .border_color(border_color)
        .rounded(radius)
        .when(theme.shadow && !disabled, |this| this.shadow_xs())
        .bg(if matches!(state, TriState::Unchecked) {
            theme.background
        } else {
            theme.primary.alpha(if disabled { 0.5 } else { 1.0 })
        })
        .when(disabled, |this| this.cursor_default())
        .when(!disabled, |this| this.cursor_pointer().on_click(on_click))
        .when_some(icon, |this, icon| {
            this.child(
                svg()
                    .flex_none()
                    .size_2p5()
                    .text_color(
                        theme
                            .primary_foreground
                            .alpha(if disabled { 0.5 } else { 1.0 }),
                    )
                    .path(icon.path()),
            )
        })
        .into_any_element()
}

fn stable_hash_for_id(value: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn status_code_description(code: char) -> &'static str {
    match code {
        '.' | ' ' => "无变更",
        'M' => "修改",
        'A' => "新增",
        'D' => "删除",
        'R' => "重命名",
        'C' => "复制",
        'T' => "类型变更",
        'U' => "未合并/冲突",
        '?' => "未跟踪",
        '!' => "忽略",
        _ => "未知",
    }
}

fn status_code_tooltip(status: &str) -> SharedString {
    if is_untracked_status(status) {
        return "未跟踪文件（Untracked）：尚未加入版本控制。".into();
    }

    if is_ignored_status(status) {
        return "忽略文件（Ignored）：被 .gitignore 等规则忽略。".into();
    }

    if is_conflict_status(status) {
        return format!("存在冲突/未合并（{status}）。\n请在冲突视图中解决后保存并 git add。")
            .into();
    }

    let Some((x, y)) = status_xy(status) else {
        return format!("Git 状态：{status}").into();
    };

    let mut extra = String::new();
    if is_modified_status_code(x) && is_modified_status_code(y) {
        extra.push_str("\n提示：该文件既有已暂存的变更，也有未暂存的变更。");
    } else if is_modified_status_code(x) {
        extra.push_str("\n提示：该文件只有暂存区有变更（已暂存）。");
    } else if is_modified_status_code(y) {
        extra.push_str("\n提示：该文件只有工作区有变更（未暂存）。");
    }

    format!(
        "Git 状态：{status}\n暂存区（Index）：{x}（{}）\n工作区（Worktree）：{y}（{}）{extra}",
        status_code_description(x),
        status_code_description(y),
    )
    .into()
}

fn render_status_badge(
    id: impl Into<ElementId>,
    status: &str,
    theme: &gpui_component::Theme,
) -> AnyElement {
    let tooltip = status_code_tooltip(status);

    let (left, right) = status_xy(status).unwrap_or(('?', '?'));
    let (left_color, right_color) = if is_conflict_status(status) {
        (theme.red, theme.red)
    } else if is_untracked_status(status) {
        (theme.warning, theme.warning)
    } else if is_ignored_status(status) {
        (theme.muted_foreground, theme.muted_foreground)
    } else {
        let left_color = if is_modified_status_code(left) {
            theme.blue
        } else {
            theme.muted_foreground
        };
        let right_color = if is_modified_status_code(right) {
            theme.green
        } else {
            theme.muted_foreground
        };
        (left_color, right_color)
    };

    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(0.))
        .text_xs()
        .font_family(theme.mono_font_family.clone())
        .line_height(relative(1.))
        .child(div().text_color(left_color).child(left.to_string()))
        .child(div().text_color(right_color).child(right.to_string()))
        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        .into_any_element()
}

pub(super) fn default_compare_target(status: &str) -> CompareTarget {
    if is_untracked_status(status) {
        return CompareTarget::HeadToWorktree;
    }

    let Some((x, y)) = status_xy(status) else {
        return CompareTarget::HeadToWorktree;
    };

    if is_modified_status_code(y) {
        return CompareTarget::IndexToWorktree;
    }
    if is_modified_status_code(x) {
        return CompareTarget::HeadToIndex;
    }

    CompareTarget::HeadToWorktree
}

#[derive(Default)]
struct ChangesTreeNode {
    dirs: BTreeMap<String, ChangesTreeNode>,
    files: Vec<FileEntry>,
    total_files: usize,
    selected_files: usize,
}

fn flat_tree_index(items: &[TreeItem], target_id: &str) -> Option<usize> {
    fn walk(items: &[TreeItem], target_id: &str, ix: &mut usize) -> Option<usize> {
        for item in items {
            if item.id.as_str() == target_id {
                return Some(*ix);
            }
            *ix += 1;
            if item.is_folder() && item.is_expanded() {
                if let Some(found) = walk(&item.children, target_id, ix) {
                    return Some(found);
                }
            }
        }
        None
    }

    let mut ix = 0usize;
    walk(items, target_id, &mut ix)
}

fn build_changes_tree_items(
    files: &[FileEntry],
    selected_for_commit: &HashSet<String>,
    item_cache: &mut HashMap<SharedString, TreeItem>,
) -> (Vec<TreeItem>, HashMap<SharedString, ChangesTreeMeta>) {
    fn basename(path: &str) -> SharedString {
        path.rsplit('/').next().unwrap_or(path).to_string().into()
    }

    fn insert_node(root: &mut ChangesTreeNode, entry: FileEntry) {
        let mut node = root;
        let mut parts = entry.path.split('/').peekable();
        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                node.files.push(entry);
                return;
            }
            node = node.dirs.entry(part.to_string()).or_default();
        }
    }

    fn compute_counts(
        node: &mut ChangesTreeNode,
        selected_for_commit: &HashSet<String>,
    ) -> (usize, usize) {
        let mut total = node.files.len();
        let mut selected = node
            .files
            .iter()
            .filter(|entry| selected_for_commit.contains(&entry.path))
            .count();

        for child in node.dirs.values_mut() {
            let (child_total, child_selected) = compute_counts(child, selected_for_commit);
            total += child_total;
            selected += child_selected;
        }

        node.total_files = total;
        node.selected_files = selected;
        (total, selected)
    }

    fn build_dir_item(
        node: &ChangesTreeNode,
        id: SharedString,
        name: SharedString,
        meta: &mut HashMap<SharedString, ChangesTreeMeta>,
        item_cache: &mut HashMap<SharedString, TreeItem>,
    ) -> TreeItem {
        let mut children = Vec::new();
        for (dir_name, dir_node) in node.dirs.iter() {
            let child_path = format!("{}/{}", id, dir_name);
            children.push(build_dir_item(
                dir_node,
                child_path.into(),
                dir_name.to_string().into(),
                meta,
                item_cache,
            ));
        }

        let mut dir_files = node.files.clone();
        dir_files.sort_by(|a, b| a.path.cmp(&b.path));
        for file in dir_files {
            let file_id: SharedString = file.path.clone().into();
            meta.insert(
                file_id.clone(),
                ChangesTreeMeta::File {
                    entry: file.clone(),
                },
            );
            let mut item = item_cache
                .entry(file_id.clone())
                .or_insert_with(|| TreeItem::new(file_id.clone(), basename(&file.path)))
                .clone();
            item.label = basename(&file.path);
            item.children.clear();
            children.push(item);
        }

        meta.insert(
            id.clone(),
            ChangesTreeMeta::Directory {
                path: id.to_string(),
                name: name.to_string(),
                file_count: node.total_files,
                selected_count: node.selected_files,
            },
        );

        let mut item = item_cache
            .entry(id.clone())
            .or_insert_with(|| TreeItem::new(id.clone(), name.clone()).expanded(true))
            .clone();
        item.label = name;
        item.children = children;
        item
    }

    let mut tracked = Vec::new();
    let mut untracked = Vec::new();
    for entry in files.iter().cloned() {
        if is_untracked_status(&entry.status) {
            untracked.push(entry);
        } else if !is_ignored_status(&entry.status) {
            tracked.push(entry);
        }
    }

    let mut root_items = Vec::new();
    let mut meta: HashMap<SharedString, ChangesTreeMeta> = HashMap::new();

    let mut push_group = |key: &str,
                          label: &str,
                          group_files: &[FileEntry],
                          root_items: &mut Vec<TreeItem>| {
        if group_files.is_empty() {
            return;
        }

        let mut root = ChangesTreeNode::default();
        for entry in group_files.iter().cloned() {
            insert_node(&mut root, entry);
        }
        compute_counts(&mut root, selected_for_commit);

        let group_path = format!(":group:{key}");
        let group_id: SharedString = group_path.clone().into();

        let mut children = Vec::new();
        for (dir_name, dir_node) in root.dirs.iter() {
            let dir_path = format!("{group_path}/{dir_name}");
            children.push(build_dir_item(
                dir_node,
                dir_path.into(),
                dir_name.to_string().into(),
                &mut meta,
                item_cache,
            ));
        }

        let mut root_files = root.files.clone();
        root_files.sort_by(|a, b| a.path.cmp(&b.path));
        for file in root_files {
            let file_id: SharedString = file.path.clone().into();
            meta.insert(
                file_id.clone(),
                ChangesTreeMeta::File {
                    entry: file.clone(),
                },
            );
            let mut item = item_cache
                .entry(file_id.clone())
                .or_insert_with(|| TreeItem::new(file_id.clone(), basename(&file.path)))
                .clone();
            item.label = basename(&file.path);
            item.children.clear();
            children.push(item);
        }

        meta.insert(
            group_id.clone(),
            ChangesTreeMeta::Directory {
                path: group_path.clone(),
                name: label.to_string(),
                file_count: root.total_files,
                selected_count: root.selected_files,
            },
        );

        let mut group_item = item_cache
            .entry(group_id.clone())
            .or_insert_with(|| TreeItem::new(group_id.clone(), label.to_string()).expanded(true))
            .clone();
        group_item.label = label.to_string().into();
        group_item.children = children;
        root_items.push(group_item);
    };

    push_group("changes", "Changes", &tracked, &mut root_items);
    push_group(
        "untracked",
        "Unversioned Files",
        &untracked,
        &mut root_items,
    );

    (root_items, meta)
}
