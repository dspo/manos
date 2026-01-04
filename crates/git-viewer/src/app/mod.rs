use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::process::Command;

use gpui::InteractiveElement as _;
use gpui::StatefulInteractiveElement as _;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui::{KeyBinding, actions};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Root, Sizable as _, StyledExt as _,
    TitleBar, WindowExt as _,
    button::{Button, ButtonVariant, ButtonVariants as _},
    checkbox::Checkbox,
    dialog::DialogButtonProps,
    input::{Input, InputState},
    menu::{ContextMenuExt as _, PopupMenu, PopupMenuItem},
    notification::Notification,
    resizable::{h_resizable, resizable_panel, v_resizable},
    tooltip::Tooltip,
    tree::{TreeItem, TreeState},
};
use gpui_manos_components::animated_segmented::{AnimatedSegmented, AnimatedSegmentedItem};
use gpui_manos_components::assets::ExtrasAssetSource;
use gpui_manos_components::plate_toolbar::PlateIconName;

use crate::git::{
    detect_repo_root, fetch_git_branch, fetch_git_status, fetch_last_commit_message,
    is_conflict_status, is_ignored_status, is_modified_status_code, is_untracked_status,
    reveal_path_in_file_manager, rollback_path_on_disk, run_git, run_git_with_stdin, status_xy,
    workspace_name_from_path,
};
use crate::model::FileEntry;

mod file_view;
mod fonts;
mod sidebar;
mod ui;

const CONTEXT: &str = "GitViewer";

actions!(git_viewer, [OpenCommandPalette, Back, Next, Prev,]);

fn init_keybindings(cx: &mut App) {
    cx.bind_keys([
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-k", OpenCommandPalette, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-k", OpenCommandPalette, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-p", OpenCommandPalette, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-p", OpenCommandPalette, Some(CONTEXT)),
        KeyBinding::new("escape", Back, Some(CONTEXT)),
        KeyBinding::new("alt-n", Next, Some(CONTEXT)),
        KeyBinding::new("alt-p", Prev, Some(CONTEXT)),
    ]);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatusFilter {
    All,
    Conflicts,
    Staged,
    Unstaged,
    Untracked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AppScreen {
    StatusList,
    FileView,
}

#[derive(Clone, Debug)]
enum ChangesTreeMeta {
    Directory {
        path: String,
        name: String,
        file_count: usize,
        selected_count: usize,
    },
    File {
        entry: FileEntry,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TriState {
    Unchecked,
    Partial,
    Checked,
}

impl TriState {
    fn for_counts(total: usize, selected: usize) -> Self {
        if total == 0 || selected == 0 {
            Self::Unchecked
        } else if selected >= total {
            Self::Checked
        } else {
            Self::Partial
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommandPaletteCommand {
    Back,
    Next,
    Prev,
    RefreshGitStatus,
}

#[derive(Clone)]
struct CommandPaletteOverlayState {
    input: Entity<InputState>,
    selected: usize,
}

#[derive(Clone, Copy)]
struct CommandPaletteItem {
    command: CommandPaletteCommand,
    title: &'static str,
    keywords: &'static str,
}

const COMMAND_PALETTE_ITEMS: &[CommandPaletteItem] = &[
    CommandPaletteItem {
        command: CommandPaletteCommand::Back,
        title: "返回",
        keywords: "back esc 返回 close",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::Next,
        title: "下一个文件",
        keywords: "next file 下一个 文件 navigate",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::Prev,
        title: "上一个文件",
        keywords: "prev file 上一个 文件 navigate",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::RefreshGitStatus,
        title: "刷新 Git 状态",
        keywords: "refresh reload git status 刷新 重新加载",
    },
];

struct GitViewerApp {
    repo_root: PathBuf,
    workspace_name: String,
    branch_name: String,
    files: Vec<FileEntry>,
    loading: bool,
    git_available: bool,
    commit_message_input: Entity<InputState>,
    commit_amend: bool,
    committing: bool,
    focus_handle: FocusHandle,
    screen: AppScreen,
    opened_file: Option<FileEntry>,
    command_palette_overlay: Option<CommandPaletteOverlayState>,
    status_filter: StatusFilter,
    changes_tree: Entity<TreeState>,
    changes_tree_item_cache: HashMap<SharedString, TreeItem>,
    changes_tree_root_items: Vec<TreeItem>,
    changes_tree_meta: HashMap<SharedString, ChangesTreeMeta>,
    changes_tree_revision: u64,
    changes_tree_built_revision: u64,
    selected_for_commit: HashSet<String>,
}

impl GitViewerApp {
    fn new(window: &mut Window, cx: &mut Context<Self>, start_dir: PathBuf) -> Self {
        let this = cx.entity();
        let repo_root = detect_repo_root(&start_dir);
        let repo_root_for_task = repo_root.clone();
        let git_available = Command::new("git").arg("--version").output().is_ok();
        let focus_handle = cx.focus_handle().tab_stop(true);
        let workspace_name = workspace_name_from_path(&repo_root);
        let branch_name = if git_available {
            "…".to_string()
        } else {
            "No Git".to_string()
        };

        window.set_window_title(&format!("{workspace_name} — {branch_name}"));

        if git_available {
            cx.spawn_in(window, async move |_, window| {
                let repo_root_for_branch = repo_root_for_task.clone();
                let (entries, branch_label) = window
                    .background_executor()
                    .spawn(async move {
                        let entries = fetch_git_status(&repo_root_for_task)
                            .map_err(|err| {
                                eprintln!("git status failed: {err:?}");
                                err
                            })
                            .unwrap_or_default();
                        let branch_label =
                            fetch_git_branch(&repo_root_for_branch).unwrap_or_else(|err| {
                                eprintln!("git branch detect failed: {err:?}");
                                "No Repo".to_string()
                            });
                        (entries, branch_label)
                    })
                    .await;

                let _ = window.update(|window, cx| {
                    this.update(cx, |this, cx| {
                        this.loading = false;
                        this.files = entries;
                        this.branch_name = branch_label;
                        this.invalidate_changes_tree();
                        cx.notify();
                    });
                    let title = this.read(cx).window_title();
                    window.set_window_title(&title);
                });

                Some(())
            })
            .detach();
        } else {
            window.push_notification(
                Notification::new().message("未检测到 git 命令：已禁用仓库状态与 Git 操作"),
                cx,
            );
        }

        let commit_message_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(6)
                .placeholder("Commit message")
                .default_value("")
        });

        let changes_tree = cx.new(|cx| TreeState::new(cx));

        Self {
            repo_root,
            workspace_name,
            branch_name,
            files: Vec::new(),
            loading: git_available,
            git_available,
            commit_message_input,
            commit_amend: false,
            committing: false,
            focus_handle,
            screen: AppScreen::StatusList,
            opened_file: None,
            command_palette_overlay: None,
            status_filter: StatusFilter::All,
            changes_tree,
            changes_tree_item_cache: HashMap::new(),
            changes_tree_root_items: Vec::new(),
            changes_tree_meta: HashMap::new(),
            changes_tree_revision: 1,
            changes_tree_built_revision: 0,
            selected_for_commit: HashSet::new(),
        }
    }

    fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    fn window_title(&self) -> String {
        format!("{} — {}", self.workspace_name, self.branch_name)
    }

    fn ordered_file_nav_entries(&self) -> Vec<FileEntry> {
        fn walk(
            item: &TreeItem,
            meta: &HashMap<SharedString, ChangesTreeMeta>,
            out: &mut Vec<FileEntry>,
        ) {
            if item.is_folder() {
                for child in &item.children {
                    walk(child, meta, out);
                }
            } else if let Some(ChangesTreeMeta::File { entry }) = meta.get(&item.id) {
                out.push(entry.clone());
            }
        }

        let mut entries = Vec::new();
        for item in &self.changes_tree_root_items {
            walk(item, &self.changes_tree_meta, &mut entries);
        }

        if !entries.is_empty() {
            return entries;
        }

        let mut tracked = Vec::new();
        let mut untracked = Vec::new();
        for entry in self
            .files
            .iter()
            .filter(|entry| matches_filter(entry, self.status_filter))
        {
            if is_untracked_status(&entry.status) {
                untracked.push(entry.clone());
            } else {
                tracked.push(entry.clone());
            }
        }
        tracked.sort_by(|a, b| a.path.cmp(&b.path));
        untracked.sort_by(|a, b| a.path.cmp(&b.path));
        tracked.extend(untracked);
        tracked
    }

    fn file_nav_state(&self, current_path: Option<&str>) -> (bool, bool) {
        let Some(current_path) = current_path else {
            return (false, false);
        };

        let entries = self.ordered_file_nav_entries();
        let Some(index) = entries.iter().position(|entry| entry.path == current_path) else {
            return (false, false);
        };

        (index > 0, index + 1 < entries.len())
    }

    fn open_adjacent_file(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let current_path = self.opened_file.as_ref().map(|entry| entry.path.clone());
        let Some(current_path) = current_path else {
            return;
        };

        let entries = self.ordered_file_nav_entries();
        if entries.is_empty() {
            return;
        }
        let Some(index) = entries.iter().position(|entry| entry.path == current_path) else {
            return;
        };

        let next_index = if delta.is_negative() {
            index.saturating_sub(delta.wrapping_abs() as usize)
        } else {
            (index + delta as usize).min(entries.len().saturating_sub(1))
        };

        if next_index == index {
            return;
        }

        let Some(entry) = entries.get(next_index) else {
            return;
        };

        self.open_file(entry.clone(), window, cx);
    }

    fn open_file(&mut self, entry: FileEntry, _window: &mut Window, cx: &mut Context<Self>) {
        self.sync_changes_tree_selection_for_entry(&entry, cx);
        self.opened_file = Some(entry);
        self.screen = AppScreen::FileView;
        cx.notify();
    }

    fn close_file_view(&mut self) {
        self.opened_file = None;
        self.screen = AppScreen::StatusList;
    }

    fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(overlay) = self.command_palette_overlay.as_ref() {
            overlay
                .input
                .update(cx, |state, cx| state.focus(window, cx));
            return;
        }

        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("输入命令…（↑↓ 选择，Enter 执行，Esc 关闭）")
                .default_value("")
        });

        input.update(cx, |state, cx| {
            state.set_value(String::new(), window, cx);
            state.focus(window, cx);
        });

        self.command_palette_overlay = Some(CommandPaletteOverlayState { input, selected: 0 });
        cx.notify();
    }

    fn close_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.command_palette_overlay.take().is_some() {
            window.focus(&self.focus_handle);
            cx.notify();
        }
    }

    fn command_palette_command_enabled(&self, command: CommandPaletteCommand) -> bool {
        match command {
            CommandPaletteCommand::Back => true,
            CommandPaletteCommand::RefreshGitStatus => self.git_available && !self.loading,
            CommandPaletteCommand::Next => {
                if self.screen != AppScreen::FileView {
                    return false;
                }
                let path = self.opened_file.as_ref().map(|entry| entry.path.as_str());
                self.file_nav_state(path).1
            }
            CommandPaletteCommand::Prev => {
                if self.screen != AppScreen::FileView {
                    return false;
                }
                let path = self.opened_file.as_ref().map(|entry| entry.path.as_str());
                self.file_nav_state(path).0
            }
        }
    }

    fn run_command_palette_command(
        &mut self,
        command: CommandPaletteCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match command {
            CommandPaletteCommand::Back => {
                if matches!(self.screen, AppScreen::FileView) {
                    self.close_file_view();
                    window.focus(&self.focus_handle);
                }
            }
            CommandPaletteCommand::Next => {
                if matches!(self.screen, AppScreen::FileView) {
                    self.open_adjacent_file(1, window, cx);
                }
            }
            CommandPaletteCommand::Prev => {
                if matches!(self.screen, AppScreen::FileView) {
                    self.open_adjacent_file(-1, window, cx);
                }
            }
            CommandPaletteCommand::RefreshGitStatus => self.refresh_git_status(window, cx),
        }
        cx.notify();
    }

    fn apply_selected_command_palette_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(overlay) = self.command_palette_overlay.as_ref() else {
            return;
        };

        let query = overlay.input.read(cx).value().to_string();
        let filtered = filter_command_palette_items(&query);
        let Some(item) = filtered.get(overlay.selected.min(filtered.len().saturating_sub(1)))
        else {
            return;
        };
        if !self.command_palette_command_enabled(item.command) {
            return;
        }

        let command = item.command;
        self.run_command_palette_command(command, window, cx);
        self.close_command_palette(window, cx);
    }

    fn handle_command_palette_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(overlay) = self.command_palette_overlay.as_mut() else {
            return false;
        };

        match event.keystroke.key.as_str() {
            "escape" => {
                self.close_command_palette(window, cx);
                true
            }
            "enter" => {
                self.apply_selected_command_palette_item(window, cx);
                true
            }
            "up" | "down" | "pageup" | "pagedown" => {
                let query = overlay.input.read(cx).value().to_string();
                let filtered_len = filter_command_palette_items(&query).len();
                if filtered_len == 0 {
                    return true;
                }

                let step = match event.keystroke.key.as_str() {
                    "pageup" | "pagedown" => 10usize,
                    _ => 1usize,
                };

                let mut selected = overlay.selected.min(filtered_len.saturating_sub(1));
                match event.keystroke.key.as_str() {
                    "up" => selected = selected.saturating_sub(step),
                    "pageup" => selected = selected.saturating_sub(step),
                    "down" => selected = (selected + step).min(filtered_len.saturating_sub(1)),
                    "pagedown" => selected = (selected + step).min(filtered_len.saturating_sub(1)),
                    _ => {}
                }
                overlay.selected = selected;
                cx.notify();
                true
            }
            _ => false,
        }
    }

    fn render_main_placeholder(&mut self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        div()
            .flex()
            .flex_col()
            .size_full()
            .items_center()
            .justify_center()
            .gap(px(10.))
            .text_color(theme.muted_foreground)
            .child(div().text_lg().child("Coming soon"))
            .child(div().text_base().child("请选择左侧文件查看占位视图"))
    }

    fn render_command_palette_overlay(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let overlay = self.command_palette_overlay.as_ref()?;
        let theme = cx.theme();
        let app = cx.entity();

        let query = overlay.input.read(cx).value().to_string();
        let filtered = filter_command_palette_items(&query);
        let selected = overlay.selected.min(filtered.len().saturating_sub(1));

        let mut list = Vec::new();
        for (index, item) in filtered.iter().take(80).enumerate() {
            let enabled = self.command_palette_command_enabled(item.command);
            let is_selected = index == selected;
            let label = item.title;
            let app = app.clone();
            let command = item.command;

            let row = div()
                .id(("command-palette-item", index))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.))
                .h(px(32.))
                .px(px(10.))
                .rounded(px(6.))
                .when(is_selected, |this| {
                    this.bg(theme.accent)
                        .text_color(theme.accent_foreground)
                        .cursor_default()
                })
                .when(!is_selected && enabled, |this| {
                    this.bg(theme.transparent)
                        .text_color(theme.popover_foreground)
                        .cursor_pointer()
                        .hover(|this| {
                            this.bg(theme.accent.alpha(0.4))
                                .text_color(theme.accent_foreground)
                        })
                        .active(|this| this.bg(theme.accent).text_color(theme.accent_foreground))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            window.prevent_default();
                            app.update(cx, |this, cx| {
                                this.run_command_palette_command(command, window, cx);
                                this.close_command_palette(window, cx);
                            });
                        })
                })
                .when(!enabled, |this| this.text_color(theme.muted_foreground))
                .child(div().truncate().child(label));

            list.push(row.into_any_element());
        }

        let overlay_container = div()
            .id("command-palette-overlay")
            .w(px(640.))
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
                        this.handle_command_palette_key(event, window, cx)
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
                    .child(div().flex().flex_col().gap(px(2.)).child("命令"))
                    .child(
                        Button::new("command-palette-close")
                            .label("关闭 (Esc)")
                            .ghost()
                            .on_click({
                                let app = app.clone();
                                move |_, window, cx| {
                                    app.update(cx, |this, cx| {
                                        this.close_command_palette(window, cx);
                                    });
                                }
                            }),
                    ),
            )
            .child(Input::new(&overlay.input).w_full())
            .child(
                div()
                    .id("command-palette-list")
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
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("↑↓ 选择 · Enter 执行 · Esc 关闭"),
            );

        Some(
            div()
                .id("command-palette-overlay-backdrop")
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
                            this.close_command_palette(window, cx);
                        });
                    }
                })
                .child(overlay_container)
                .into_any_element(),
        )
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct StatusCounts {
    all: usize,
    conflicts: usize,
    staged: usize,
    unstaged: usize,
    untracked: usize,
}

impl StatusCounts {
    fn from_entries(entries: &[FileEntry]) -> Self {
        let mut counts = Self::default();

        for entry in entries {
            let status = entry.status.as_str();
            if is_ignored_status(status) {
                continue;
            }
            counts.all += 1;
            if is_untracked_status(status) {
                counts.untracked += 1;
                continue;
            }
            if is_conflict_status(status) {
                counts.conflicts += 1;
                continue;
            }

            if let Some((x, y)) = status_xy(status) {
                if is_modified_status_code(x) {
                    counts.staged += 1;
                }
                if is_modified_status_code(y) {
                    counts.unstaged += 1;
                }
            }
        }

        counts
    }
}

fn matches_filter(entry: &FileEntry, filter: StatusFilter) -> bool {
    if is_ignored_status(&entry.status) {
        return false;
    }

    match filter {
        StatusFilter::All => true,
        StatusFilter::Conflicts => is_conflict_status(&entry.status),
        StatusFilter::Untracked => is_untracked_status(&entry.status),
        StatusFilter::Staged => {
            if is_conflict_status(&entry.status) || is_untracked_status(&entry.status) {
                return false;
            }
            status_xy(&entry.status).is_some_and(|(x, _)| is_modified_status_code(x))
        }
        StatusFilter::Unstaged => {
            if is_conflict_status(&entry.status) || is_untracked_status(&entry.status) {
                return false;
            }
            status_xy(&entry.status).is_some_and(|(_, y)| is_modified_status_code(y))
        }
    }
}

fn filter_command_palette_items(query: &str) -> Vec<&'static CommandPaletteItem> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return COMMAND_PALETTE_ITEMS.iter().collect();
    }

    let tokens: Vec<&str> = query.split_whitespace().filter(|t| !t.is_empty()).collect();
    if tokens.is_empty() {
        return COMMAND_PALETTE_ITEMS.iter().collect();
    }

    COMMAND_PALETTE_ITEMS
        .iter()
        .filter(|item| {
            let title = item.title.to_lowercase();
            let keywords = item.keywords.to_lowercase();
            tokens
                .iter()
                .all(|token| title.contains(token) || keywords.contains(token))
        })
        .collect()
}

pub(crate) fn run(start_dir: PathBuf) {
    let app = Application::new().with_assets(ExtrasAssetSource::new());

    app.run(move |cx| {
        gpui_component::init(cx);
        init_keybindings(cx);
        cx.activate(true);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let loaded_fonts = fonts::try_load_user_fonts(cx);
        let applied_theme_fonts = fonts::select_theme_fonts_from_env(cx);
        if applied_theme_fonts.mono_font_family {
            eprintln!("git-viewer: using mono font family from GIT_VIEWER_MONO_FONT_FAMILY");
        }
        if !applied_theme_fonts.ui_font_family {
            if let Some(loaded_fonts) = loaded_fonts.as_ref() {
                if loaded_fonts.detected_families.len() == 1 {
                    let family = loaded_fonts.detected_families[0].clone();
                    gpui_component::Theme::global_mut(cx).font_family = family.clone().into();
                    eprintln!("git-viewer: using loaded font family: {family}");
                } else if !loaded_fonts.detected_families.is_empty() {
                    eprintln!(
                        "git-viewer: loaded fonts, detected families: {} (set GIT_VIEWER_FONT_FAMILY to choose)",
                        loaded_fonts.detected_families.join(", ")
                    );
                }
            }
        }
        if let Some(loaded_fonts) = loaded_fonts {
            if loaded_fonts.loaded_files > 0 && loaded_fonts.detected_families.is_empty() {
                eprintln!(
                    "git-viewer: loaded {} font file(s), but detected no new font families",
                    loaded_fonts.loaded_files
                );
            }
        }

        cx.spawn(async move |cx| {
            cx.open_window(
                WindowOptions {
                    titlebar: Some(TitleBar::title_bar_options()),
                    ..Default::default()
                },
                {
                    let start_dir = start_dir.clone();
                    move |window, cx| {
                        let view = cx.new(|cx| GitViewerApp::new(window, cx, start_dir.clone()));
                        let handle = view.read(cx).focus_handle();
                        window.focus(&handle);
                        cx.new(|cx| Root::new(view, window, cx))
                    }
                },
            )?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
