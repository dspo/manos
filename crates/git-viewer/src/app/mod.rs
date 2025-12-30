use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use anyhow::anyhow;
use gpui::InteractiveElement as _;
use gpui::StatefulInteractiveElement as _;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui::{KeyBinding, actions};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Root, Sizable as _, StyledExt as _,
    TitleBar, VirtualListScrollHandle, WindowExt as _,
    button::{Button, ButtonVariant, ButtonVariants as _},
    checkbox::Checkbox,
    dialog::DialogButtonProps,
    input::{Input, InputState},
    menu::{ContextMenuExt as _, PopupMenu, PopupMenuItem},
    notification::Notification,
    popover::Popover,
    resizable::{h_resizable, resizable_panel, v_resizable},
    scroll::{Scrollbar, ScrollbarState},
    tooltip::Tooltip,
    tree::{TreeItem, TreeState},
    v_virtual_list,
};
use gpui_manos_components::animated_segmented::{AnimatedSegmented, AnimatedSegmentedItem};
use gpui_manos_components::assets::ExtrasAssetSource;
use gpui_manos_components::plate_toolbar::PlateIconName;

use crate::git::{
    GitRefEntry, GitRefKind, detect_repo_root, fetch_file_history, fetch_git_branch,
    fetch_git_refs, fetch_git_status, fetch_last_commit_message, is_clean_status_code,
    is_conflict_status, is_ignored_status, is_modified_status_code, is_untracked_status,
    read_head_file, read_index_file, read_specified_file, read_working_file,
    reveal_path_in_file_manager, rollback_path_on_disk, run_git, run_git_with_stdin, status_xy,
    workspace_name_from_path,
};
use crate::model::{CommitEntry, FileEntry};

mod conflict_view;
mod diff_helpers;
mod diff_view;
mod fonts;
mod ref_picker;
mod sidebar;
mod ui;

use diff_helpers::*;

const CONTEXT: &str = "GitViewer";

actions!(
    git_viewer,
    [
        OpenCommandPalette,
        Back,
        Next,
        Prev,
        ToggleDiffFind,
        ToggleViewMode,
        ToggleSplitLayout,
        ToggleWhitespace,
        ExpandAll,
        ApplyEditor,
        SaveConflict,
        SaveConflictAndAdd,
    ]
);

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
        KeyBinding::new("alt-v", ToggleViewMode, Some(CONTEXT)),
        KeyBinding::new("alt-l", ToggleSplitLayout, Some(CONTEXT)),
        KeyBinding::new("alt-w", ToggleWhitespace, Some(CONTEXT)),
        KeyBinding::new("alt-e", ExpandAll, Some(CONTEXT)),
        KeyBinding::new("alt-a", ApplyEditor, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-f", ToggleDiffFind, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-f", ToggleDiffFind, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-s", SaveConflict, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-s", SaveConflict, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-s", SaveConflictAndAdd, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-s", SaveConflictAndAdd, Some(CONTEXT)),
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
    DiffView,
    ConflictView,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct DiffViewOptions {
    ignore_whitespace: bool,
    context_lines: usize,
}

#[derive(Clone, Copy, Debug)]
struct DiffLoadTiming {
    io_ms: u128,
    diff_ms: u128,
    total_ms: u128,
}

const MAX_CONTEXT_LINES: usize = 20;
const DIFF_REBUILD_DEBOUNCE_MS: u64 = 120;
const DIFF_CACHE_CAPACITY: usize = 18;
const DIFF_CACHE_MAX_BYTES: usize = 4 * 1024 * 1024;
const DIFF_PREFETCH_MAX_BYTES: usize = 512 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplitLayout {
    Aligned,
    TwoPane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum DiffViewMode {
    Split,
    Inline,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum CompareTarget {
    HeadToWorktree,
    IndexToWorktree,
    HeadToIndex,
    Refs { left: String, right: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DiffCacheKey {
    path: String,
    compare_target: CompareTarget,
    options: DiffViewOptions,
}

impl DiffCacheKey {
    fn new(path: String, compare_target: CompareTarget, options: DiffViewOptions) -> Self {
        Self {
            path,
            compare_target,
            options,
        }
    }
}

struct DiffCache {
    capacity: usize,
    entries: HashMap<DiffCacheKey, DiffViewState>,
    order: VecDeque<DiffCacheKey>,
}

impl DiffCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn contains(&self, key: &DiffCacheKey) -> bool {
        self.entries.contains_key(key)
    }

    fn take(&mut self, key: &DiffCacheKey) -> Option<DiffViewState> {
        let value = self.entries.remove(key)?;
        self.order.retain(|item| item != key);
        Some(value)
    }

    fn insert(&mut self, key: DiffCacheKey, value: DiffViewState) {
        if self.entries.contains_key(&key) {
            self.entries.insert(key.clone(), value);
            self.touch(&key);
            return;
        }

        self.entries.insert(key.clone(), value);
        self.order.retain(|item| item != &key);
        self.order.push_back(key);
        self.evict_if_needed();
    }

    fn touch(&mut self, key: &DiffCacheKey) {
        self.order.retain(|item| item != key);
        self.order.push_back(key.clone());
    }

    fn evict_if_needed(&mut self) {
        while self.entries.len() > self.capacity {
            let Some(evicted) = self.order.pop_front() else {
                break;
            };
            self.entries.remove(&evicted);
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HistoryCompareMode {
    ParentToCommit,
    CommitToWorktree,
}

#[derive(Clone)]
struct FileHistoryOverlayState {
    path: String,
    status: Option<String>,
    commits: Vec<CommitEntry>,
    loading: bool,
    selected: usize,
    mode: HistoryCompareMode,
    filter_input: Entity<InputState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RefPickerSide {
    Left,
    Right,
}

#[derive(Clone)]
struct RefPickerOverlayState {
    side: RefPickerSide,
    refs: Vec<GitRefEntry>,
    loading: bool,
    selected: usize,
    filter_input: Entity<InputState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommandPaletteCommand {
    Back,
    Next,
    Prev,
    ToggleDiffFind,
    ToggleViewMode,
    ToggleSplitLayout,
    ToggleWhitespace,
    ToggleWorktreeEditor,
    ExpandAll,
    RefreshGitStatus,
    OpenFileHistory,
    StageSelection,
    UnstageSelection,
    RevertSelection,
    ClearDiffSelection,
    ApplyEditor,
    SaveConflict,
    SaveConflictAndAdd,
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
        title: "下一处（hunk/冲突）",
        keywords: "next 下一个 hunk 冲突 navigate",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::Prev,
        title: "上一处（hunk/冲突）",
        keywords: "prev 上一个 hunk 冲突 navigate",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::ToggleDiffFind,
        title: "查找（Diff）",
        keywords: "find search diff 查找 搜索",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::ToggleViewMode,
        title: "切换视图 Split/Inline",
        keywords: "toggle view mode split inline 视图",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::ToggleSplitLayout,
        title: "切换布局 对齐/分栏",
        keywords: "toggle layout aligned two-pane 对齐 分栏",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::ToggleWhitespace,
        title: "切换忽略空白",
        keywords: "toggle whitespace ignore 空白 忽略",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::ToggleWorktreeEditor,
        title: "切换工作区编辑器面板",
        keywords: "toggle worktree editor panel 编辑器 工作区 面板",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::ExpandAll,
        title: "展开全部折叠",
        keywords: "expand all folds 展开 折叠",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::RefreshGitStatus,
        title: "刷新 Git 状态",
        keywords: "refresh reload git status 刷新 重新加载",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::OpenFileHistory,
        title: "打开文件历史对比",
        keywords: "history log commit 历史 对比",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::StageSelection,
        title: "Stage 选中行",
        keywords: "stage selection 暂存 选中 行 部分",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::UnstageSelection,
        title: "Unstage 选中行",
        keywords: "unstage selection 取消暂存 选中 行 部分",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::RevertSelection,
        title: "Revert 选中行",
        keywords: "revert selection discard 丢弃 选中 行 部分",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::ClearDiffSelection,
        title: "清除 Diff 选中行",
        keywords: "clear selection 取消 选中 行",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::ApplyEditor,
        title: "应用编辑内容（冲突/工作区）",
        keywords: "apply editor 应用 合并 结果 worktree 工作区",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::SaveConflict,
        title: "保存冲突结果到文件",
        keywords: "save conflict 保存 写入",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::SaveConflictAndAdd,
        title: "保存并 git add",
        keywords: "save add stage resolved 解决",
    },
];

#[derive(Clone, Copy, Debug)]
enum ConflictResolution {
    Ours,
    Theirs,
    Base,
    Both,
}

#[derive(Clone, Copy, Debug)]
enum HunkApplyAction {
    Stage,
    Unstage,
    Revert,
}

impl HunkApplyAction {
    fn label(self) -> &'static str {
        match self {
            Self::Stage => "Stage",
            Self::Unstage => "Unstage",
            Self::Revert => "Revert",
        }
    }

    fn required_compare_target(self) -> CompareTarget {
        match self {
            Self::Stage | Self::Revert => CompareTarget::IndexToWorktree,
            Self::Unstage => CompareTarget::HeadToIndex,
        }
    }

    fn git_args(self) -> Vec<&'static str> {
        match self {
            Self::Stage => vec!["apply", "--cached"],
            Self::Unstage => vec!["apply", "-R", "--cached"],
            Self::Revert => vec!["apply", "-R"],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct DiffRowRef {
    hunk_index: usize,
    row_index: usize,
}

#[derive(Clone, Debug)]
enum DisplayRow {
    HunkHeader {
        text: SharedString,
    },
    Fold {
        old_start: usize,
        new_start: usize,
        len: usize,
    },
    Code {
        source: Option<DiffRowRef>,
        kind: diffview::DiffRowKind,
        old_line: Option<usize>,
        new_line: Option<usize>,
        old_segments: Vec<diffview::DiffSegment>,
        new_segments: Vec<diffview::DiffSegment>,
    },
}

#[derive(Clone)]
struct DiffViewState {
    title: SharedString,
    path: Option<String>,
    orig_path: Option<String>,
    status: Option<String>,
    compare_target: CompareTarget,
    rows_view_mode: DiffViewMode,
    old_text: String,
    new_text: String,
    old_lines: Vec<String>,
    new_lines: Vec<String>,
    diff_model: diffview::DiffModel,
    rows: Vec<DisplayRow>,
    selected_rows: HashSet<DiffRowRef>,
    selection_anchor: Option<DiffRowRef>,
    hunk_rows: Vec<usize>,
    current_hunk: usize,
    scroll_handle: VirtualListScrollHandle,
    scroll_state: ScrollbarState,
    list_item_sizes: Rc<Vec<Size<Pixels>>>,
    list_item_height: Pixels,
}

#[derive(Clone, Debug)]
enum ConflictRow {
    EmptyState {
        text: SharedString,
    },
    BlockHeader {
        conflict_index: usize,
        ours_branch_name: SharedString,
        theirs_branch_name: SharedString,
        has_base: bool,
    },
    Code {
        kind: diffview::DiffRowKind,
        ours_segments: Vec<diffview::DiffSegment>,
        base_segments: Vec<diffview::DiffSegment>,
        theirs_segments: Vec<diffview::DiffSegment>,
    },
}

#[derive(Clone)]
struct ConflictViewState {
    title: SharedString,
    path: Option<String>,
    text: String,
    result_input: Entity<InputState>,
    show_result_editor: bool,
    conflicts: Vec<diffview::ConflictRegion>,
    rows: Vec<ConflictRow>,
    conflict_rows: Vec<usize>,
    current_conflict: usize,
    scroll_handle: VirtualListScrollHandle,
    scroll_state: ScrollbarState,
    list_item_sizes: Rc<Vec<Size<Pixels>>>,
    list_item_height: Pixels,
}

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
    diff_view: Option<DiffViewState>,
    conflict_view: Option<ConflictViewState>,
    diff_options: DiffViewOptions,
    timing_enabled: bool,
    last_diff_load_timing: Option<DiffLoadTiming>,
    diff_find_open: bool,
    diff_find_input: Entity<InputState>,
    diff_find_matches: Vec<usize>,
    diff_find_selected: usize,
    diff_find_last_query: String,
    diff_find_last_rows_len: usize,
    diff_find_last_content_revision: u64,
    diff_find_last_view_mode: DiffViewMode,
    compare_left_input: Entity<InputState>,
    compare_right_input: Entity<InputState>,
    compare_target_preference: CompareTarget,
    file_history_overlay: Option<FileHistoryOverlayState>,
    ref_picker_overlay: Option<RefPickerOverlayState>,
    command_palette_overlay: Option<CommandPaletteOverlayState>,
    diff_content_revision: u64,
    diff_rebuild_seq: u64,
    split_layout: SplitLayout,
    view_mode: DiffViewMode,
    status_filter: StatusFilter,
    changes_tree: Entity<TreeState>,
    changes_tree_item_cache: HashMap<SharedString, TreeItem>,
    changes_tree_root_items: Vec<TreeItem>,
    changes_tree_meta: HashMap<SharedString, ChangesTreeMeta>,
    changes_tree_revision: u64,
    changes_tree_built_revision: u64,
    selected_for_commit: HashSet<String>,
    show_diff_worktree_editor: bool,
    diff_worktree_editor_input: Entity<InputState>,
    diff_worktree_editor_loaded_path: Option<String>,
    diff_cache: DiffCache,
    diff_cache_revision: u64,
    current_diff_cache_key: Option<DiffCacheKey>,
    diff_prefetch_inflight: HashSet<DiffCacheKey>,
    open_file_seq: u64,
}

impl GitViewerApp {
    fn new(window: &mut Window, cx: &mut Context<Self>, start_dir: PathBuf) -> Self {
        let this = cx.entity();
        let repo_root = detect_repo_root(&start_dir);
        let repo_root_for_task = repo_root.clone();
        let git_available = Command::new("git").arg("--version").output().is_ok();
        let timing_enabled = std::env::var("GIT_VIEWER_TIMING").is_ok();
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
                Notification::new()
                    .message("未检测到 git 命令：已禁用仓库状态与 Git 操作（可打开 Demo）"),
                cx,
            );
        }

        let compare_left_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("左侧 ref（例如 HEAD~1 / a1b2c3）")
                .default_value("HEAD")
        });
        let compare_right_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("右侧 ref（留空=工作区，或 INDEX / :）")
                .default_value("")
        });

        let commit_message_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line()
                .rows(6)
                .placeholder("Commit message")
                .default_value("")
        });

        let diff_find_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("查找（Enter/↓ 下一处，Shift+Enter/↑ 上一处，Esc 关闭）")
                .default_value("")
        });

        let diff_worktree_editor_input = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("text")
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
            diff_view: None,
            conflict_view: None,
            diff_options: DiffViewOptions {
                ignore_whitespace: false,
                context_lines: 3,
            },
            timing_enabled,
            last_diff_load_timing: None,
            diff_find_open: false,
            diff_find_input,
            diff_find_matches: Vec::new(),
            diff_find_selected: 0,
            diff_find_last_query: String::new(),
            diff_find_last_rows_len: 0,
            diff_find_last_content_revision: 0,
            diff_find_last_view_mode: DiffViewMode::Split,
            compare_left_input,
            compare_right_input,
            compare_target_preference: CompareTarget::IndexToWorktree,
            file_history_overlay: None,
            ref_picker_overlay: None,
            command_palette_overlay: None,
            diff_content_revision: 0,
            diff_rebuild_seq: 0,
            split_layout: SplitLayout::TwoPane,
            view_mode: DiffViewMode::Split,
            status_filter: StatusFilter::All,
            changes_tree,
            changes_tree_item_cache: HashMap::new(),
            changes_tree_root_items: Vec::new(),
            changes_tree_meta: HashMap::new(),
            changes_tree_revision: 1,
            changes_tree_built_revision: 0,
            selected_for_commit: HashSet::new(),
            show_diff_worktree_editor: false,
            diff_worktree_editor_input,
            diff_worktree_editor_loaded_path: None,
            diff_cache: DiffCache::new(DIFF_CACHE_CAPACITY),
            diff_cache_revision: 0,
            current_diff_cache_key: None,
            diff_prefetch_inflight: HashSet::new(),
            open_file_seq: 0,
        }
    }

    fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    fn window_title(&self) -> String {
        format!("{} — {}", self.workspace_name, self.branch_name)
    }

    fn open_demo(&mut self) {
        let (old, new) = demo_texts();
        self.open_diff_view(
            "Split Diff Demo".into(),
            None,
            None,
            CompareTarget::HeadToWorktree,
            old,
            new,
        );
    }

    fn open_large_demo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let line_count = std::env::var("GIT_VIEWER_LARGE_DEMO_LINES")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(20_000);

        let this = cx.entity();
        window.push_notification(
            Notification::new().message(format!("正在生成 Large Diff Demo（{line_count} 行）…")),
            cx,
        );

        cx.spawn_in(window, async move |_, window| {
            let (old_text, new_text) = window
                .background_executor()
                .spawn(async move { large_demo_texts(line_count) })
                .await;

            let diff_options =
                window
                    .update(|_, cx| this.read(cx).diff_options)
                    .unwrap_or(DiffViewOptions {
                        ignore_whitespace: false,
                        context_lines: 3,
                    });
            let view_mode = window
                .update(|_, cx| this.read(cx).view_mode)
                .unwrap_or(DiffViewMode::Split);

            let (old_text, new_text, model, old_lines, new_lines, rows, hunk_rows) = window
                .background_executor()
                .spawn(async move {
                    let (model, old_lines, new_lines) = build_diff_model(
                        &old_text,
                        &new_text,
                        diff_options.ignore_whitespace,
                        diff_options.context_lines,
                    );
                    let rows =
                        build_display_rows_from_model(&model, &old_lines, &new_lines, view_mode);
                    let hunk_rows = rows
                        .iter()
                        .enumerate()
                        .filter_map(|(index, row)| {
                            matches!(row, DisplayRow::HunkHeader { .. }).then_some(index)
                        })
                        .collect::<Vec<_>>();
                    (
                        old_text, new_text, model, old_lines, new_lines, rows, hunk_rows,
                    )
                })
                .await;

            window
                .update(|window, cx| {
                    window.push_notification(
                        Notification::new().message("Large Diff Demo 已加载"),
                        cx,
                    );
                    this.update(cx, |this, _cx| {
                        this.diff_content_revision = this.diff_content_revision.wrapping_add(1);
                        this.conflict_view = None;
                        this.diff_view = Some(DiffViewState::from_precomputed_rows(
                            format!("Large Diff Demo ({line_count} lines)").into(),
                            None,
                            None,
                            CompareTarget::HeadToWorktree,
                            view_mode,
                            old_text,
                            new_text,
                            model,
                            old_lines,
                            new_lines,
                            rows,
                            hunk_rows,
                        ));
                        this.current_diff_cache_key = None;
                        this.screen = AppScreen::DiffView;
                    });
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn open_conflict_demo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = conflict_demo_text();
        let initial_text = text.clone();
        let result_input = cx.new(move |cx| {
            InputState::new(window, cx)
                .code_editor("diff")
                .default_value(initial_text)
        });
        self.open_conflict_view("Conflict Demo".into(), None, text, result_input);
    }

    fn open_diff_view(
        &mut self,
        title: SharedString,
        path: Option<String>,
        status: Option<String>,
        compare_target: CompareTarget,
        old_text: String,
        new_text: String,
    ) {
        self.stash_current_diff_view();
        self.last_diff_load_timing = None;
        self.diff_content_revision = self.diff_content_revision.wrapping_add(1);
        self.conflict_view = None;
        self.diff_view = Some(DiffViewState::new(
            title,
            path,
            status,
            compare_target,
            old_text,
            new_text,
            self.diff_options,
            self.view_mode,
        ));
        self.current_diff_cache_key = None;
        self.screen = AppScreen::DiffView;
    }

    fn open_conflict_view(
        &mut self,
        title: SharedString,
        path: Option<String>,
        text: String,
        result_input: Entity<InputState>,
    ) {
        self.stash_current_diff_view();
        self.diff_find_open = false;
        self.diff_view = None;
        self.current_diff_cache_key = None;
        self.conflict_view = Some(ConflictViewState::new(title, path, text, result_input));
        self.screen = AppScreen::ConflictView;
    }

    fn clear_diff_cache(&mut self) {
        self.diff_cache.clear();
        self.diff_prefetch_inflight.clear();
        self.diff_cache_revision = self.diff_cache_revision.wrapping_add(1);
    }

    fn stash_current_diff_view(&mut self) {
        let Some(key) = self.current_diff_cache_key.take() else {
            return;
        };
        let Some(view) = self.diff_view.take() else {
            return;
        };

        if view.path.is_none() {
            return;
        }

        let bytes = view.old_text.len().saturating_add(view.new_text.len());
        if bytes > DIFF_CACHE_MAX_BYTES {
            return;
        }

        self.diff_cache.insert(key, view);
    }

    fn prefetch_neighbor_diffs(
        &mut self,
        current_path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.git_available {
            return;
        }

        let entries = self
            .ordered_worktree_files()
            .into_iter()
            .map(|entry| (*entry).clone())
            .collect::<Vec<_>>();
        let Some(index) = entries.iter().position(|entry| entry.path == current_path) else {
            return;
        };

        let mut neighbors = Vec::new();
        if index > 0 {
            neighbors.push(entries[index - 1].clone());
        }
        if index + 1 < entries.len() {
            neighbors.push(entries[index + 1].clone());
        }

        for entry in neighbors {
            if is_conflict_status(&entry.status) {
                continue;
            }

            let target = sidebar::default_compare_target(&entry.status);
            let key = DiffCacheKey::new(entry.path.clone(), target.clone(), self.diff_options);
            if self.current_diff_cache_key.as_ref() == Some(&key)
                || self.diff_cache.contains(&key)
                || self.diff_prefetch_inflight.contains(&key)
            {
                continue;
            }

            self.spawn_prefetch_diff(key, entry, target, self.view_mode, window, cx);
        }
    }

    fn spawn_prefetch_diff(
        &mut self,
        key: DiffCacheKey,
        entry: FileEntry,
        target: CompareTarget,
        view_mode: DiffViewMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.diff_prefetch_inflight.insert(key.clone());
        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let revision = self.diff_cache_revision;
        let diff_options = key.options;
        let path_for_task = entry.path.clone();
        let status_for_task = entry.status.clone();
        let orig_path_for_task = entry.orig_path.clone();

        cx.spawn_in(window, async move |_, window| {
            let (old_path, new_path) = compare_paths_for_file(
                &path_for_task,
                orig_path_for_task.as_deref(),
                &status_for_task,
                &target,
            );
            let (old_text, _old_err, new_text, _new_err) = read_compare_texts_for_target(
                window,
                repo_root,
                old_path,
                new_path,
                status_for_task.clone(),
                target.clone(),
            )
            .await;

            if old_text.len().saturating_add(new_text.len()) > DIFF_PREFETCH_MAX_BYTES {
                window
                    .update(|_, cx| {
                        this.update(cx, |this, _cx| {
                            this.diff_prefetch_inflight.remove(&key);
                        });
                    })
                    .ok();
                return Some(());
            }

            let (old_text, new_text, model, old_lines, new_lines, rows, hunk_rows) = window
                .background_executor()
                .spawn(async move {
                    let (model, old_lines, new_lines) = build_diff_model(
                        &old_text,
                        &new_text,
                        diff_options.ignore_whitespace,
                        diff_options.context_lines,
                    );
                    let rows =
                        build_display_rows_from_model(&model, &old_lines, &new_lines, view_mode);
                    let hunk_rows = rows
                        .iter()
                        .enumerate()
                        .filter_map(|(index, row)| {
                            matches!(row, DisplayRow::HunkHeader { .. }).then_some(index)
                        })
                        .collect::<Vec<_>>();
                    (
                        old_text, new_text, model, old_lines, new_lines, rows, hunk_rows,
                    )
                })
                .await;

            window
                .update(|_, cx| {
                    this.update(cx, |this, cx| {
                        this.diff_prefetch_inflight.remove(&key);

                        if this.diff_cache_revision != revision {
                            return;
                        }

                        if this.current_diff_cache_key.as_ref() == Some(&key)
                            || this.diff_cache.contains(&key)
                        {
                            return;
                        }

                        let title: SharedString =
                            format!("{status_for_task} {path_for_task}").into();
                        let mut view = DiffViewState::from_precomputed_rows(
                            title,
                            Some(path_for_task),
                            Some(status_for_task),
                            target,
                            view_mode,
                            old_text,
                            new_text,
                            model,
                            old_lines,
                            new_lines,
                            rows,
                            hunk_rows,
                        );
                        view.orig_path = orig_path_for_task.clone();
                        this.diff_cache.insert(key, view);
                        cx.notify();
                    });
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn close_diff_view(&mut self) {
        self.diff_find_open = false;
        self.screen = AppScreen::StatusList;
    }

    fn close_conflict_view(&mut self) {
        self.diff_find_open = false;
        self.screen = AppScreen::StatusList;
    }

    fn open_file_diff(&mut self, entry: FileEntry, window: &mut Window, cx: &mut Context<Self>) {
        let target = match &self.compare_target_preference {
            CompareTarget::HeadToWorktree
            | CompareTarget::IndexToWorktree
            | CompareTarget::HeadToIndex => self.compare_target_preference.clone(),
            CompareTarget::Refs { .. } => sidebar::default_compare_target(&entry.status),
        };
        self.open_file_diff_with_target(entry, target, window, cx);
    }

    fn open_file_diff_with_target(
        &mut self,
        entry: FileEntry,
        target: CompareTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_file_seq = self.open_file_seq.wrapping_add(1);
        let open_seq = self.open_file_seq;
        self.last_diff_load_timing = None;

        self.stash_current_diff_view();
        let cache_key = DiffCacheKey::new(entry.path.clone(), target.clone(), self.diff_options);
        if let Some(mut cached) = self.diff_cache.take(&cache_key) {
            if cached.rows_view_mode != self.view_mode {
                cached.rebuild_rows(self.view_mode);
            }
            cached.title = format!("{} {}", entry.status, entry.path).into();
            cached.path = Some(entry.path.clone());
            cached.orig_path = entry.orig_path.clone();
            cached.status = Some(entry.status.clone());
            self.diff_content_revision = self.diff_content_revision.wrapping_add(1);
            self.conflict_view = None;
            self.diff_view = Some(cached);
            self.current_diff_cache_key = Some(cache_key);
            self.screen = AppScreen::DiffView;
            self.sync_diff_worktree_editor_for_current_file(window, cx);
            cx.notify();
            self.prefetch_neighbor_diffs(&entry.path, window, cx);
            return;
        }

        self.diff_content_revision = self.diff_content_revision.wrapping_add(1);
        self.conflict_view = None;
        self.diff_view = Some(DiffViewState::loading(
            format!("{} {}", entry.status, entry.path).into(),
            Some(entry.path.clone()),
            Some(entry.status.clone()),
            target.clone(),
        ));
        if let Some(view) = self.diff_view.as_mut() {
            view.orig_path = entry.orig_path.clone();
        }
        self.current_diff_cache_key = None;
        self.screen = AppScreen::DiffView;
        if self.show_diff_worktree_editor && compare_target_new_is_worktree(&target) {
            self.diff_worktree_editor_loaded_path = None;
            let language = code_language_for_path(&entry.path);
            self.diff_worktree_editor_input.update(cx, |state, cx| {
                state.set_highlighter(language, cx);
                state.set_value(String::new(), window, cx);
            });
        }
        cx.notify();

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let path_for_task = entry.path.clone();
        let orig_path_for_task = entry.orig_path.clone();
        let status_for_task = entry.status.clone();

        cx.spawn_in(window, async move |_, window| {
            let total_start = Instant::now();
            let io_start = Instant::now();
            let (old_path, new_path) = compare_paths_for_file(
                &path_for_task,
                orig_path_for_task.as_deref(),
                &status_for_task,
                &target,
            );
            let (old_text, old_err, new_text, new_err) = read_compare_texts_for_target(
                window,
                repo_root,
                old_path,
                new_path,
                status_for_task.clone(),
                target.clone(),
            )
            .await;
            let io_ms = io_start.elapsed().as_millis();

            let diff_options =
                window
                    .update(|_, cx| this.read(cx).diff_options)
                    .unwrap_or(DiffViewOptions {
                        ignore_whitespace: false,
                        context_lines: 3,
                    });
            let view_mode = window
                .update(|_, cx| this.read(cx).view_mode)
                .unwrap_or(DiffViewMode::Split);
            let cache_key = DiffCacheKey::new(path_for_task.clone(), target.clone(), diff_options);

            let diff_start = Instant::now();
            let (old_text, new_text, model, old_lines, new_lines, rows, hunk_rows) = window
                .background_executor()
                .spawn(async move {
                    let (model, old_lines, new_lines) = build_diff_model(
                        &old_text,
                        &new_text,
                        diff_options.ignore_whitespace,
                        diff_options.context_lines,
                    );
                    let rows =
                        build_display_rows_from_model(&model, &old_lines, &new_lines, view_mode);
                    let hunk_rows = rows
                        .iter()
                        .enumerate()
                        .filter_map(|(index, row)| {
                            matches!(row, DisplayRow::HunkHeader { .. }).then_some(index)
                        })
                        .collect::<Vec<_>>();
                    (
                        old_text, new_text, model, old_lines, new_lines, rows, hunk_rows,
                    )
                })
                .await;
            let diff_ms = diff_start.elapsed().as_millis();
            let total_ms = total_start.elapsed().as_millis();
            let timing = DiffLoadTiming {
                io_ms,
                diff_ms,
                total_ms,
            };

            window
                .update(|window, cx| {
                    if let Some(err) = old_err {
                        window.push_notification(
                            Notification::new().message(format!(
                                "读取 {} 版本失败，按空内容处理：{err}",
                                compare_target_side_label(&target, Side::Old)
                            )),
                            cx,
                        );
                    }
                    if let Some(err) = new_err {
                        window.push_notification(
                            Notification::new().message(format!(
                                "读取 {} 版本失败，按空内容处理：{err}",
                                compare_target_side_label(&target, Side::New)
                            )),
                            cx,
                        );
                    }

                    this.update(cx, |this, cx| {
                        if this.open_file_seq != open_seq {
                            return;
                        }
                        if this.timing_enabled {
                            this.last_diff_load_timing = Some(timing);
                        } else {
                            this.last_diff_load_timing = None;
                        }
                        this.diff_content_revision = this.diff_content_revision.wrapping_add(1);
                        this.conflict_view = None;
                        let mut view = DiffViewState::from_precomputed_rows(
                            format!("{status_for_task} {path_for_task}").into(),
                            Some(path_for_task.clone()),
                            Some(status_for_task.clone()),
                            target,
                            view_mode,
                            old_text,
                            new_text,
                            model,
                            old_lines,
                            new_lines,
                            rows,
                            hunk_rows,
                        );
                        view.orig_path = orig_path_for_task.clone();
                        this.diff_view = Some(view);
                        this.current_diff_cache_key = Some(cache_key);
                        this.screen = AppScreen::DiffView;
                        this.sync_diff_worktree_editor_for_current_file(window, cx);
                        cx.notify();
                        this.prefetch_neighbor_diffs(&path_for_task, window, cx);
                    });
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn open_file_diff_with_refs(
        &mut self,
        path: String,
        status: Option<String>,
        left_ref: String,
        right_ref: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.git_available {
            window.push_notification(
                Notification::new().message("未检测到 git 命令，无法进行历史/任意版本对比"),
                cx,
            );
            return;
        }

        let left_ref = left_ref.trim().to_string();
        let right_ref = right_ref.trim().to_string();
        if left_ref.is_empty() {
            window.push_notification(
                Notification::new().message("左侧 ref 不能为空（例如 HEAD / HEAD~1 / a1b2c3）"),
                cx,
            );
            return;
        }

        let compare_target = CompareTarget::Refs {
            left: left_ref.clone(),
            right: right_ref.clone(),
        };

        let title: SharedString = match &status {
            Some(status) if !status.trim().is_empty() => format!("{status} {path}").into(),
            _ => path.clone().into(),
        };

        self.open_file_seq = self.open_file_seq.wrapping_add(1);
        let open_seq = self.open_file_seq;
        self.last_diff_load_timing = None;

        self.stash_current_diff_view();
        let cache_key = DiffCacheKey::new(path.clone(), compare_target.clone(), self.diff_options);
        if let Some(mut cached) = self.diff_cache.take(&cache_key) {
            if cached.rows_view_mode != self.view_mode {
                cached.rebuild_rows(self.view_mode);
            }
            cached.title = title.clone();
            cached.path = Some(path.clone());
            cached.status = status.clone();
            self.diff_content_revision = self.diff_content_revision.wrapping_add(1);
            self.conflict_view = None;
            self.diff_view = Some(cached);
            self.current_diff_cache_key = Some(cache_key);
            self.screen = AppScreen::DiffView;
            self.sync_diff_worktree_editor_for_current_file(window, cx);
            cx.notify();
            self.prefetch_neighbor_diffs(&path, window, cx);
            return;
        }

        self.diff_content_revision = self.diff_content_revision.wrapping_add(1);
        self.conflict_view = None;
        self.diff_view = Some(DiffViewState::loading(
            title.clone(),
            Some(path.clone()),
            status.clone(),
            compare_target.clone(),
        ));
        self.current_diff_cache_key = None;
        self.screen = AppScreen::DiffView;
        cx.notify();

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let path_for_task = path.clone();
        let status_for_task = status.clone();

        cx.spawn_in(window, async move |_, window| {
            let total_start = Instant::now();
            let io_start = Instant::now();
            let (old_text, old_err, new_text, new_err) = read_compare_texts_for_target(
                window,
                repo_root,
                path_for_task.clone(),
                path_for_task.clone(),
                status_for_task.clone().unwrap_or_default(),
                compare_target.clone(),
            )
            .await;
            let io_ms = io_start.elapsed().as_millis();

            let diff_options =
                window
                    .update(|_, cx| this.read(cx).diff_options)
                    .unwrap_or(DiffViewOptions {
                        ignore_whitespace: false,
                        context_lines: 3,
                    });
            let view_mode = window
                .update(|_, cx| this.read(cx).view_mode)
                .unwrap_or(DiffViewMode::Split);
            let cache_key =
                DiffCacheKey::new(path_for_task.clone(), compare_target.clone(), diff_options);

            let diff_start = Instant::now();
            let (old_text, new_text, model, old_lines, new_lines, rows, hunk_rows) = window
                .background_executor()
                .spawn(async move {
                    let (model, old_lines, new_lines) = build_diff_model(
                        &old_text,
                        &new_text,
                        diff_options.ignore_whitespace,
                        diff_options.context_lines,
                    );
                    let rows =
                        build_display_rows_from_model(&model, &old_lines, &new_lines, view_mode);
                    let hunk_rows = rows
                        .iter()
                        .enumerate()
                        .filter_map(|(index, row)| {
                            matches!(row, DisplayRow::HunkHeader { .. }).then_some(index)
                        })
                        .collect::<Vec<_>>();
                    (
                        old_text, new_text, model, old_lines, new_lines, rows, hunk_rows,
                    )
                })
                .await;
            let diff_ms = diff_start.elapsed().as_millis();
            let total_ms = total_start.elapsed().as_millis();
            let timing = DiffLoadTiming {
                io_ms,
                diff_ms,
                total_ms,
            };

            window
                .update(|window, cx| {
                    if let Some(err) = old_err {
                        window.push_notification(
                            Notification::new().message(format!(
                                "读取 {} 版本失败，按空内容处理：{err}",
                                compare_target_side_label(&compare_target, Side::Old)
                            )),
                            cx,
                        );
                    }
                    if let Some(err) = new_err {
                        window.push_notification(
                            Notification::new().message(format!(
                                "读取 {} 版本失败，按空内容处理：{err}",
                                compare_target_side_label(&compare_target, Side::New)
                            )),
                            cx,
                        );
                    }

                    this.update(cx, |this, cx| {
                        if this.open_file_seq != open_seq {
                            return;
                        }
                        if this.timing_enabled {
                            this.last_diff_load_timing = Some(timing);
                        } else {
                            this.last_diff_load_timing = None;
                        }
                        this.diff_content_revision = this.diff_content_revision.wrapping_add(1);
                        this.conflict_view = None;
                        this.diff_view = Some(DiffViewState::from_precomputed_rows(
                            title,
                            Some(path_for_task.clone()),
                            status_for_task.clone(),
                            compare_target.clone(),
                            view_mode,
                            old_text,
                            new_text,
                            model,
                            old_lines,
                            new_lines,
                            rows,
                            hunk_rows,
                        ));
                        this.current_diff_cache_key = Some(cache_key);
                        this.screen = AppScreen::DiffView;
                        this.sync_diff_worktree_editor_for_current_file(window, cx);
                        cx.notify();
                        this.prefetch_neighbor_diffs(&path_for_task, window, cx);
                    });
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn apply_compare_refs_from_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(diff_view) = self.diff_view.as_ref() else {
            return;
        };
        let Some(path) = diff_view.path.clone() else {
            window.push_notification(
                Notification::new().message("当前 diff 为 demo，暂不支持任意版本对比"),
                cx,
            );
            return;
        };

        let left_ref = self.compare_left_input.read(cx).value().to_string();
        let right_ref = self.compare_right_input.read(cx).value().to_string();
        let status = diff_view.status.clone();
        self.open_file_diff_with_refs(path, status, left_ref, right_ref, window, cx);
    }

    fn swap_compare_refs_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let left_ref = self.compare_left_input.read(cx).value().to_string();
        let right_ref = self.compare_right_input.read(cx).value().to_string();

        self.compare_left_input.update(cx, |state, cx| {
            state.set_value(right_ref.clone(), window, cx);
        });
        self.compare_right_input.update(cx, |state, cx| {
            state.set_value(left_ref.clone(), window, cx);
        });
        cx.notify();
    }

    fn ordered_worktree_files(&self) -> Vec<&FileEntry> {
        let mut entries: Vec<&FileEntry> = self
            .files
            .iter()
            .filter(|entry| !is_ignored_status(&entry.status))
            .collect();
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        entries
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
        let current_path = self
            .diff_view
            .as_ref()
            .and_then(|view| view.path.clone())
            .or_else(|| {
                self.conflict_view
                    .as_ref()
                    .and_then(|view| view.path.clone())
            });
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

    fn open_file_history_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.git_available {
            window.push_notification(
                Notification::new().message("未检测到 git 命令，无法打开历史对比"),
                cx,
            );
            return;
        }

        self.command_palette_overlay = None;

        let Some(diff_view) = self.diff_view.as_ref() else {
            return;
        };
        let Some(path) = diff_view.path.clone() else {
            window.push_notification(
                Notification::new().message("当前 diff 为 demo，暂不支持历史对比"),
                cx,
            );
            return;
        };

        let status = diff_view.status.clone();
        let filter_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("搜索 commit（hash / message）")
                .default_value("")
        });

        self.file_history_overlay = Some(FileHistoryOverlayState {
            path: path.clone(),
            status,
            commits: Vec::new(),
            loading: true,
            selected: 0,
            mode: HistoryCompareMode::ParentToCommit,
            filter_input: filter_input.clone(),
        });

        filter_input.update(cx, |state, cx| state.focus(window, cx));

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let path_for_task = path.clone();
        cx.spawn_in(window, async move |_, window| {
            let path_for_task_bg = path_for_task.clone();
            let path_for_update = path_for_task.clone();
            let repo_root_bg = repo_root.clone();
            let commits = window
                .background_executor()
                .spawn(async move { fetch_file_history(&repo_root_bg, &path_for_task_bg, 200) })
                .await;

            window
                .update(|window, cx| match commits {
                    Ok(commits) => this.update(cx, |this, cx| {
                        if let Some(overlay) = this.file_history_overlay.as_mut() {
                            if overlay.path == path_for_update {
                                overlay.loading = false;
                                overlay.commits = commits;
                                overlay.selected = 0;
                                cx.notify();
                            }
                        }
                    }),
                    Err(err) => {
                        window.push_notification(
                            Notification::new().message(format!("获取历史失败：{err:#}")),
                            cx,
                        );
                        this.update(cx, |this, cx| {
                            if let Some(overlay) = this.file_history_overlay.as_mut() {
                                if overlay.path == path_for_update {
                                    overlay.loading = false;
                                    cx.notify();
                                }
                            }
                        });
                    }
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn close_file_history_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.file_history_overlay.take().is_some() {
            window.focus(&self.focus_handle);
            cx.notify();
        }
    }

    fn apply_selected_file_history_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(overlay) = self.file_history_overlay.as_ref() else {
            return;
        };
        if overlay.loading || overlay.commits.is_empty() {
            return;
        }

        let query = overlay.filter_input.read(cx).value().to_string();
        let filtered = filter_commits(&overlay.commits, &query);
        let Some(entry) = filtered.get(overlay.selected.min(filtered.len().saturating_sub(1)))
        else {
            return;
        };

        let (left_ref, right_ref) = match overlay.mode {
            HistoryCompareMode::ParentToCommit => (format!("{}^", entry.hash), entry.hash.clone()),
            HistoryCompareMode::CommitToWorktree => (entry.hash.clone(), String::new()),
        };

        let path = overlay.path.clone();
        let status = overlay.status.clone();
        self.file_history_overlay = None;
        self.open_file_diff_with_refs(path, status, left_ref, right_ref, window, cx);
    }

    fn handle_file_history_overlay_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(overlay) = self.file_history_overlay.as_mut() else {
            return false;
        };

        match event.keystroke.key.as_str() {
            "escape" => {
                self.close_file_history_overlay(window, cx);
                true
            }
            "enter" => {
                self.apply_selected_file_history_commit(window, cx);
                true
            }
            "up" | "down" | "pageup" | "pagedown" => {
                let query = overlay.filter_input.read(cx).value().to_string();
                let filtered_len = filter_commits(&overlay.commits, &query).len();
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

    fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.file_history_overlay = None;

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
            CommandPaletteCommand::Next | CommandPaletteCommand::Prev => match self.screen {
                AppScreen::DiffView => self
                    .diff_view
                    .as_ref()
                    .is_some_and(|view| !view.hunk_rows.is_empty()),
                AppScreen::ConflictView => self
                    .conflict_view
                    .as_ref()
                    .is_some_and(|view| !view.conflict_rows.is_empty()),
                AppScreen::StatusList => false,
            },
            CommandPaletteCommand::ToggleDiffFind => self.screen == AppScreen::DiffView,
            CommandPaletteCommand::ToggleViewMode => matches!(self.screen, AppScreen::DiffView),
            CommandPaletteCommand::ToggleSplitLayout => {
                matches!(self.screen, AppScreen::DiffView) && self.view_mode == DiffViewMode::Split
            }
            CommandPaletteCommand::ToggleWhitespace => matches!(self.screen, AppScreen::DiffView),
            CommandPaletteCommand::ToggleWorktreeEditor => {
                self.screen == AppScreen::DiffView
                    && self.diff_view.as_ref().is_some_and(|view| {
                        view.path.is_some() && compare_target_new_is_worktree(&view.compare_target)
                    })
            }
            CommandPaletteCommand::ExpandAll => {
                self.screen == AppScreen::DiffView
                    && self.diff_view.as_ref().is_some_and(|view| {
                        view.rows
                            .iter()
                            .any(|row| matches!(row, DisplayRow::Fold { .. }))
                    })
            }
            CommandPaletteCommand::RefreshGitStatus => self.git_available && !self.loading,
            CommandPaletteCommand::OpenFileHistory => {
                self.git_available
                    && self.screen == AppScreen::DiffView
                    && self
                        .diff_view
                        .as_ref()
                        .is_some_and(|view| view.path.is_some())
            }
            CommandPaletteCommand::StageSelection => {
                self.git_available
                    && self.screen == AppScreen::DiffView
                    && self.diff_view.as_ref().is_some_and(|view| {
                        let status = view.status.as_deref().unwrap_or_default();
                        view.path.is_some()
                            && !is_untracked_status(status)
                            && view.compare_target == CompareTarget::IndexToWorktree
                            && !view.selected_rows.is_empty()
                    })
            }
            CommandPaletteCommand::UnstageSelection => {
                self.git_available
                    && self.screen == AppScreen::DiffView
                    && self.diff_view.as_ref().is_some_and(|view| {
                        let status = view.status.as_deref().unwrap_or_default();
                        view.path.is_some()
                            && !is_untracked_status(status)
                            && view.compare_target == CompareTarget::HeadToIndex
                            && !view.selected_rows.is_empty()
                    })
            }
            CommandPaletteCommand::RevertSelection => {
                self.git_available
                    && self.screen == AppScreen::DiffView
                    && self.diff_view.as_ref().is_some_and(|view| {
                        let status = view.status.as_deref().unwrap_or_default();
                        view.path.is_some()
                            && !is_untracked_status(status)
                            && view.compare_target == CompareTarget::IndexToWorktree
                            && !view.selected_rows.is_empty()
                    })
            }
            CommandPaletteCommand::ClearDiffSelection => {
                self.screen == AppScreen::DiffView
                    && self
                        .diff_view
                        .as_ref()
                        .is_some_and(|view| !view.selected_rows.is_empty())
            }
            CommandPaletteCommand::ApplyEditor => match self.screen {
                AppScreen::ConflictView => true,
                AppScreen::DiffView => {
                    self.show_diff_worktree_editor
                        && self.diff_view.as_ref().is_some_and(|view| {
                            view.path.is_some()
                                && compare_target_new_is_worktree(&view.compare_target)
                        })
                }
                AppScreen::StatusList => false,
            },
            CommandPaletteCommand::SaveConflict | CommandPaletteCommand::SaveConflictAndAdd => {
                let Some(view) = self.conflict_view.as_ref() else {
                    return false;
                };
                let can_save = view.path.is_some() && view.conflicts.is_empty();
                match command {
                    CommandPaletteCommand::SaveConflict => can_save,
                    CommandPaletteCommand::SaveConflictAndAdd => can_save && self.git_available,
                    _ => false,
                }
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
                if self.command_palette_overlay.is_some() {
                    self.close_command_palette(window, cx);
                    return;
                }
                if self.file_history_overlay.is_some() {
                    self.close_file_history_overlay(window, cx);
                    return;
                }
                if self.screen == AppScreen::DiffView
                    && self
                        .diff_view
                        .as_ref()
                        .is_some_and(|view| !view.selected_rows.is_empty())
                {
                    self.clear_diff_selection();
                    window.focus(&self.focus_handle);
                    return;
                }

                match self.screen {
                    AppScreen::DiffView => self.close_diff_view(),
                    AppScreen::ConflictView => self.close_conflict_view(),
                    AppScreen::StatusList => {}
                }
                window.focus(&self.focus_handle);
            }
            CommandPaletteCommand::Next => match self.screen {
                AppScreen::DiffView => self.jump_hunk(1),
                AppScreen::ConflictView => self.jump_conflict(1),
                AppScreen::StatusList => {}
            },
            CommandPaletteCommand::Prev => match self.screen {
                AppScreen::DiffView => self.jump_hunk(-1),
                AppScreen::ConflictView => self.jump_conflict(-1),
                AppScreen::StatusList => {}
            },
            CommandPaletteCommand::ToggleDiffFind => {
                if matches!(self.screen, AppScreen::DiffView) {
                    self.toggle_diff_find(window, cx);
                }
            }
            CommandPaletteCommand::ToggleViewMode => {
                if matches!(self.screen, AppScreen::DiffView) {
                    let next = match self.view_mode {
                        DiffViewMode::Split => DiffViewMode::Inline,
                        DiffViewMode::Inline => DiffViewMode::Split,
                    };
                    self.set_view_mode(next);
                }
            }
            CommandPaletteCommand::ToggleSplitLayout => {
                if self.screen == AppScreen::DiffView && self.view_mode == DiffViewMode::Split {
                    self.split_layout = match self.split_layout {
                        SplitLayout::Aligned => SplitLayout::TwoPane,
                        SplitLayout::TwoPane => SplitLayout::Aligned,
                    };
                }
            }
            CommandPaletteCommand::ToggleWhitespace => {
                if matches!(self.screen, AppScreen::DiffView) {
                    let next = !self.diff_options.ignore_whitespace;
                    self.set_ignore_whitespace(next, window, cx);
                }
            }
            CommandPaletteCommand::ToggleWorktreeEditor => {
                if matches!(self.screen, AppScreen::DiffView) {
                    self.toggle_diff_worktree_editor(window, cx);
                }
            }
            CommandPaletteCommand::ExpandAll => {
                if matches!(self.screen, AppScreen::DiffView) {
                    self.expand_all_folds();
                }
            }
            CommandPaletteCommand::RefreshGitStatus => {
                self.refresh_git_status(window, cx);
            }
            CommandPaletteCommand::OpenFileHistory => {
                if matches!(self.screen, AppScreen::DiffView) {
                    self.open_file_history_overlay(window, cx);
                }
            }
            CommandPaletteCommand::StageSelection => {
                if matches!(self.screen, AppScreen::DiffView) {
                    self.stage_selected_rows(window, cx);
                }
            }
            CommandPaletteCommand::UnstageSelection => {
                if matches!(self.screen, AppScreen::DiffView) {
                    self.unstage_selected_rows(window, cx);
                }
            }
            CommandPaletteCommand::RevertSelection => {
                if matches!(self.screen, AppScreen::DiffView) {
                    self.revert_selected_rows(window, cx);
                }
            }
            CommandPaletteCommand::ClearDiffSelection => {
                if matches!(self.screen, AppScreen::DiffView) {
                    self.clear_diff_selection();
                }
            }
            CommandPaletteCommand::ApplyEditor => match self.screen {
                AppScreen::ConflictView => self.apply_conflict_editor(window, cx),
                AppScreen::DiffView => {
                    if self.show_diff_worktree_editor {
                        self.apply_diff_worktree_editor_preview(window, cx);
                    }
                }
                AppScreen::StatusList => {}
            },
            CommandPaletteCommand::SaveConflict => {
                if matches!(self.screen, AppScreen::ConflictView) {
                    self.save_conflict_to_working_tree(false, window, cx);
                }
            }
            CommandPaletteCommand::SaveConflictAndAdd => {
                if matches!(self.screen, AppScreen::ConflictView) {
                    self.save_conflict_to_working_tree(true, window, cx);
                }
            }
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

    fn set_compare_target(
        &mut self,
        target: CompareTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(
            target,
            CompareTarget::HeadToWorktree
                | CompareTarget::IndexToWorktree
                | CompareTarget::HeadToIndex
        ) {
            self.compare_target_preference = target.clone();
        }

        let Some(diff_view) = self.diff_view.as_ref() else {
            return;
        };
        if diff_view.compare_target == target {
            return;
        }

        let Some(path) = diff_view.path.clone() else {
            return;
        };
        let status = diff_view.status.clone().unwrap_or_default();
        self.open_file_diff_with_target(
            FileEntry {
                path,
                status,
                orig_path: diff_view.orig_path.clone(),
            },
            target,
            window,
            cx,
        );
    }

    fn open_file(&mut self, entry: FileEntry, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_changes_tree_selection_for_entry(&entry, cx);

        if entry.status.contains('U') {
            self.open_conflict_file(entry, window, cx);
        } else {
            self.open_file_diff(entry, window, cx);
        }
    }

    fn open_conflict_file(
        &mut self,
        entry: FileEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_file_seq = self.open_file_seq.wrapping_add(1);
        let open_seq = self.open_file_seq;

        self.stash_current_diff_view();
        self.diff_find_open = false;
        self.diff_view = None;
        self.current_diff_cache_key = None;

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let path_for_task = entry.path.clone();
        let status_for_task = entry.status.clone();
        let language = code_language_for_path(&path_for_task);
        let language_for_update = language.clone();
        let result_input = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor(language.clone())
                .default_value(String::new())
        });

        self.conflict_view = Some(ConflictViewState::loading(
            format!("{status_for_task} {path_for_task}").into(),
            Some(path_for_task.clone()),
            result_input.clone(),
        ));
        self.screen = AppScreen::ConflictView;
        cx.notify();

        cx.spawn_in(window, async move |_, window| {
            let path_for_task_bg = path_for_task.clone();
            let (text, err) = window
                .background_executor()
                .spawn(async move {
                    match read_working_file(&repo_root, &path_for_task_bg) {
                        Ok(text) => (text, None),
                        Err(err) => (String::new(), Some(err.to_string())),
                    }
                })
                .await;

            window
                .update(|window, cx| {
                    if let Some(err) = err {
                        window.push_notification(
                            Notification::new()
                                .message(format!("读取工作区版本失败，按空内容处理：{err}")),
                            cx,
                        );
                    }

                    let initial_text = text.clone();
                    result_input.update(cx, |state, cx| {
                        state.set_highlighter(language_for_update.clone(), cx);
                        state.set_value(initial_text, window, cx);
                    });
                    this.update(cx, |this, cx| {
                        if this.open_file_seq != open_seq {
                            return;
                        }
                        this.open_conflict_view(
                            format!("{status_for_task} {path_for_task}").into(),
                            Some(path_for_task),
                            text,
                            result_input,
                        );
                        cx.notify();
                    });
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn set_ignore_whitespace(&mut self, value: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.diff_options.ignore_whitespace == value {
            return;
        }
        self.diff_options.ignore_whitespace = value;
        self.request_diff_rebuild(window, cx);
    }

    fn set_context_lines(&mut self, value: usize, window: &mut Window, cx: &mut Context<Self>) {
        let value = value.min(MAX_CONTEXT_LINES);
        if self.diff_options.context_lines == value {
            return;
        }
        self.diff_options.context_lines = value;
        self.request_diff_rebuild(window, cx);
    }

    fn set_view_mode(&mut self, mode: DiffViewMode) {
        if self.view_mode == mode {
            return;
        }
        self.view_mode = mode;
        if let Some(diff_view) = self.diff_view.as_mut() {
            diff_view.rebuild_rows(mode);
        }
    }

    fn toggle_diff_worktree_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_diff_worktree_editor = !self.show_diff_worktree_editor;
        if self.show_diff_worktree_editor {
            self.sync_diff_worktree_editor_for_current_file(window, cx);
        }
        cx.notify();
    }

    fn sync_diff_worktree_editor_for_current_file(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.show_diff_worktree_editor {
            return;
        }

        let Some(diff_view) = self.diff_view.as_ref() else {
            return;
        };
        let Some(path) = diff_view.path.as_deref() else {
            return;
        };
        if !compare_target_new_is_worktree(&diff_view.compare_target) {
            return;
        }

        if self.diff_worktree_editor_loaded_path.as_deref() == Some(path) {
            return;
        }

        let language = code_language_for_path(path);
        let initial_text = diff_view.new_text.clone();
        self.diff_worktree_editor_input.update(cx, |state, cx| {
            state.set_highlighter(language, cx);
            state.set_value(initial_text, window, cx);
        });
        self.diff_worktree_editor_loaded_path = Some(path.to_string());
    }

    fn apply_diff_worktree_editor_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(diff_view) = self.diff_view.as_mut() else {
            return;
        };
        let Some(path) = diff_view.path.clone() else {
            window.push_notification(Notification::new().message("demo 不支持编辑"), cx);
            return;
        };
        if !compare_target_new_is_worktree(&diff_view.compare_target) {
            window.push_notification(
                Notification::new().message("当前对比目标不包含工作区版本，无法应用编辑内容"),
                cx,
            );
            return;
        }

        let text = self.diff_worktree_editor_input.read(cx).value().to_string();
        diff_view.new_text = text;
        self.diff_content_revision = self.diff_content_revision.wrapping_add(1);
        self.request_diff_rebuild(window, cx);
        self.diff_worktree_editor_loaded_path = Some(path);
        cx.notify();
    }

    fn reload_diff_worktree_editor_from_disk(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(diff_view) = self.diff_view.as_ref() else {
            return;
        };
        let Some(path) = diff_view.path.clone() else {
            window.push_notification(Notification::new().message("demo 不支持编辑"), cx);
            return;
        };
        if !compare_target_new_is_worktree(&diff_view.compare_target) {
            window.push_notification(
                Notification::new().message("当前对比目标不包含工作区版本，无法重新加载"),
                cx,
            );
            return;
        }

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let open_seq = self.open_file_seq;
        window.push_notification(
            Notification::new().message(format!("从磁盘重新加载：{path}")),
            cx,
        );

        cx.spawn_in(window, async move |_, window| {
            let path_bg = path.clone();
            let text = window
                .background_executor()
                .spawn(async move { read_working_file(&repo_root, &path_bg).unwrap_or_default() })
                .await;

            window
                .update(|window, cx| {
                    this.update(cx, |this, cx| {
                        if this.open_file_seq != open_seq {
                            return;
                        }
                        let language = code_language_for_path(&path);
                        this.diff_worktree_editor_input.update(cx, |state, cx| {
                            state.set_highlighter(language, cx);
                            state.set_value(text.clone(), window, cx);
                        });
                        this.diff_worktree_editor_loaded_path = Some(path.clone());

                        if let Some(view) = this.diff_view.as_mut() {
                            if view.path.as_deref() == Some(path.as_str())
                                && compare_target_new_is_worktree(&view.compare_target)
                            {
                                view.new_text = text;
                                this.diff_content_revision =
                                    this.diff_content_revision.wrapping_add(1);
                                this.request_diff_rebuild(window, cx);
                            }
                        }

                        cx.notify();
                    });
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn save_diff_worktree_editor_to_disk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.git_available {
            window.push_notification(
                Notification::new().message("未检测到 git 命令，无法保存后刷新状态"),
                cx,
            );
        }

        let Some(diff_view) = self.diff_view.as_ref() else {
            return;
        };
        let Some(path) = diff_view.path.clone() else {
            window.push_notification(Notification::new().message("demo 不支持编辑"), cx);
            return;
        };
        if !compare_target_new_is_worktree(&diff_view.compare_target) {
            window.push_notification(
                Notification::new().message("当前对比目标不包含工作区版本，无法保存"),
                cx,
            );
            return;
        }

        let text = self.diff_worktree_editor_input.read(cx).value().to_string();
        let repo_root = self.repo_root.clone();
        let this = cx.entity();
        let open_seq = self.open_file_seq;

        window.push_notification(
            Notification::new().message(format!("保存到工作区文件：{path}")),
            cx,
        );

        cx.spawn_in(window, async move |_, window| {
            let path_bg = path.clone();
            let repo_root_bg = repo_root.clone();
            let text_for_write = text.clone();
            let (write_result, status_result) = window
                .background_executor()
                .spawn(async move {
                    let full_path = repo_root_bg.join(&path_bg);
                    let write_result = std::fs::write(&full_path, text_for_write.as_bytes())
                        .with_context(|| format!("写入文件失败：{path_bg}"));

                    let status_result = fetch_git_status(&repo_root_bg);
                    (write_result, status_result)
                })
                .await;

            window
                .update(|window, cx| {
                    if let Err(err) = write_result {
                        window.push_notification(
                            Notification::new().message(format!("保存失败：{err:#}")),
                            cx,
                        );
                        return;
                    }

                    window.push_notification(Notification::new().message("已保存到工作区文件"), cx);

                    this.update(cx, |this, cx| {
                        this.clear_diff_cache();

                        if let Ok(entries) = status_result {
                            this.files = entries;
                            let mut valid_paths = HashSet::new();
                            for entry in &this.files {
                                if !is_ignored_status(&entry.status) {
                                    valid_paths.insert(entry.path.clone());
                                }
                            }
                            this.selected_for_commit
                                .retain(|path| valid_paths.contains(path));
                            this.invalidate_changes_tree();
                        }

                        if this.open_file_seq == open_seq {
                            if let Some(view) = this.diff_view.as_mut() {
                                if view.path.as_deref() == Some(path.as_str())
                                    && compare_target_new_is_worktree(&view.compare_target)
                                {
                                    view.new_text = text.clone();

                                    if let Some(entry) =
                                        this.files.iter().find(|entry| entry.path == path)
                                    {
                                        view.status = Some(entry.status.clone());
                                        view.title =
                                            format!("{} {}", entry.status, entry.path).into();
                                    }

                                    this.diff_content_revision =
                                        this.diff_content_revision.wrapping_add(1);
                                    this.request_diff_rebuild(window, cx);
                                }
                            }
                        }

                        cx.notify();
                    });
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn toggle_diff_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.screen, AppScreen::DiffView) {
            return;
        }

        if self.diff_find_open {
            self.close_diff_find(window, cx);
            return;
        }

        self.diff_find_open = true;
        self.diff_find_input
            .update(cx, |state, cx| state.focus(window, cx));
        cx.notify();
    }

    fn close_diff_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.diff_find_open = false;
        self.diff_find_matches.clear();
        self.diff_find_selected = 0;
        self.diff_find_last_query.clear();
        self.diff_find_last_rows_len = 0;
        self.diff_find_last_content_revision = 0;
        self.diff_find_last_view_mode = self.view_mode;
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn update_diff_find_matches_if_needed(&mut self, cx: &mut Context<Self>) {
        if !self.diff_find_open {
            return;
        }
        let Some(diff_view) = self.diff_view.as_ref() else {
            self.diff_find_matches.clear();
            self.diff_find_selected = 0;
            return;
        };

        let query = self.diff_find_input.read(cx).value().trim().to_string();
        let rows_len = diff_view.rows.len();
        let content_revision = self.diff_content_revision;
        let view_mode = self.view_mode;

        if query == self.diff_find_last_query
            && rows_len == self.diff_find_last_rows_len
            && content_revision == self.diff_find_last_content_revision
            && view_mode == self.diff_find_last_view_mode
        {
            return;
        }

        let previous_row = self.diff_find_matches.get(self.diff_find_selected).copied();

        self.diff_find_last_query = query.clone();
        self.diff_find_last_rows_len = rows_len;
        self.diff_find_last_content_revision = content_revision;
        self.diff_find_last_view_mode = view_mode;
        self.diff_find_matches.clear();

        if query.is_empty() {
            self.diff_find_selected = 0;
            return;
        }

        for (index, row) in diff_view.rows.iter().enumerate() {
            let mut matched = false;

            match row {
                DisplayRow::HunkHeader { text } => {
                    matched = text.as_ref().contains(&query);
                }
                DisplayRow::Code {
                    old_line, new_line, ..
                } => {
                    if let Some(line_no) = new_line {
                        if diff_view
                            .new_lines
                            .get(line_no.saturating_sub(1))
                            .is_some_and(|line| line.contains(&query))
                        {
                            matched = true;
                        }
                    }

                    if !matched {
                        if let Some(line_no) = old_line {
                            if diff_view
                                .old_lines
                                .get(line_no.saturating_sub(1))
                                .is_some_and(|line| line.contains(&query))
                            {
                                matched = true;
                            }
                        }
                    }
                }
                DisplayRow::Fold { .. } => {}
            }

            if matched {
                self.diff_find_matches.push(index);
            }
        }

        if self.diff_find_matches.is_empty() {
            self.diff_find_selected = 0;
            return;
        }

        if let Some(previous_row) = previous_row {
            if let Ok(pos) = self.diff_find_matches.binary_search(&previous_row) {
                self.diff_find_selected = pos;
                return;
            }
        }

        self.diff_find_selected = self
            .diff_find_selected
            .min(self.diff_find_matches.len().saturating_sub(1));
    }

    fn diff_find_highlight_color(
        &self,
        row_index: usize,
        theme: &gpui_component::Theme,
    ) -> Option<Hsla> {
        if !self.diff_find_open {
            return None;
        }
        if self.diff_find_last_query.is_empty() {
            return None;
        }

        if self.diff_find_matches.binary_search(&row_index).is_err() {
            return None;
        }

        let is_current = self
            .diff_find_matches
            .get(self.diff_find_selected)
            .is_some_and(|current| *current == row_index);

        Some(if is_current {
            theme.blue
        } else {
            theme.yellow.alpha(0.9)
        })
    }

    fn jump_diff_find_match(&mut self, delta: i32, _window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.screen, AppScreen::DiffView) || !self.diff_find_open {
            return;
        }

        self.update_diff_find_matches_if_needed(cx);
        let total = self.diff_find_matches.len();
        if total == 0 {
            return;
        }

        let current = self.diff_find_selected.min(total.saturating_sub(1));
        let next = if delta < 0 {
            if current == 0 { total - 1 } else { current - 1 }
        } else if current + 1 >= total {
            0
        } else {
            current + 1
        };

        self.diff_find_selected = next;
        if let Some(row_index) = self.diff_find_matches.get(next).copied() {
            if let Some(diff_view) = self.diff_view.as_ref() {
                diff_view
                    .scroll_handle
                    .scroll_to_item(row_index, ScrollStrategy::Top);
            }
        }
        cx.notify();
    }

    fn clear_diff_selection(&mut self) {
        let Some(view) = self.diff_view.as_mut() else {
            return;
        };
        view.selected_rows.clear();
        view.selection_anchor = None;
    }

    fn diff_row_ref_at_display_index(&self, display_index: usize) -> Option<DiffRowRef> {
        let view = self.diff_view.as_ref()?;
        match view.rows.get(display_index)? {
            DisplayRow::Code { source, .. } => *source,
            _ => None,
        }
    }

    fn diff_model_kind(&self, row_ref: DiffRowRef) -> Option<diffview::DiffRowKind> {
        let view = self.diff_view.as_ref()?;
        view.diff_model
            .hunks
            .get(row_ref.hunk_index)?
            .rows
            .get(row_ref.row_index)
            .map(|row| row.kind())
    }

    fn handle_diff_row_click(
        &mut self,
        display_index: usize,
        modifiers: Modifiers,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(row_ref) = self.diff_row_ref_at_display_index(display_index) else {
            return;
        };
        if self
            .diff_model_kind(row_ref)
            .is_some_and(|kind| kind == diffview::DiffRowKind::Unchanged)
        {
            return;
        }

        let Some(view) = self.diff_view.as_mut() else {
            return;
        };

        let extend = modifiers.shift;
        let toggle = modifiers.secondary();

        if extend {
            let anchor = view.selection_anchor.unwrap_or(row_ref);

            let (start, end) = if (anchor.hunk_index, anchor.row_index)
                <= (row_ref.hunk_index, row_ref.row_index)
            {
                (anchor, row_ref)
            } else {
                (row_ref, anchor)
            };

            let mut range = HashSet::new();
            for hunk_index in start.hunk_index..=end.hunk_index {
                let Some(hunk) = view.diff_model.hunks.get(hunk_index) else {
                    continue;
                };
                if hunk.rows.is_empty() {
                    continue;
                }
                let row_start = if hunk_index == start.hunk_index {
                    start.row_index
                } else {
                    0
                };
                let row_end = if hunk_index == end.hunk_index {
                    end.row_index.min(hunk.rows.len().saturating_sub(1))
                } else {
                    hunk.rows.len().saturating_sub(1)
                };
                if row_start > row_end {
                    continue;
                }

                for row_index in row_start..=row_end {
                    let Some(row) = hunk.rows.get(row_index) else {
                        continue;
                    };
                    if row.kind() == diffview::DiffRowKind::Unchanged {
                        continue;
                    }
                    range.insert(DiffRowRef {
                        hunk_index,
                        row_index,
                    });
                }
            }

            if toggle {
                view.selected_rows.extend(range);
            } else {
                view.selected_rows = range;
            }
            view.selection_anchor = Some(row_ref);
            cx.notify();
            return;
        }

        if toggle {
            if view.selected_rows.contains(&row_ref) {
                view.selected_rows.remove(&row_ref);
            } else {
                view.selected_rows.insert(row_ref);
            }
            view.selection_anchor = Some(row_ref);
            cx.notify();
            return;
        }

        view.selected_rows.clear();
        view.selected_rows.insert(row_ref);
        view.selection_anchor = Some(row_ref);
        cx.notify();
    }

    fn diff_context_menu_builder(
        app: Entity<Self>,
        row_ref: Option<DiffRowRef>,
        hunk_index: Option<usize>,
        kind: diffview::DiffRowKind,
        compare_target: CompareTarget,
        file_status: String,
        has_file_path: bool,
        selection_count: usize,
        git_available: bool,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
        move |menu: PopupMenu, _window: &mut Window, _cx: &mut Context<PopupMenu>| {
            let has_selection = selection_count > 0;
            let is_untracked = is_untracked_status(&file_status);
            let is_rename_or_copy = status_xy(&file_status)
                .is_some_and(|(x, y)| matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C'));

            let can_stage_selection = git_available
                && has_file_path
                && has_selection
                && !is_untracked
                && !is_rename_or_copy
                && compare_target == CompareTarget::IndexToWorktree;
            let can_unstage_selection = git_available
                && has_file_path
                && has_selection
                && !is_untracked
                && !is_rename_or_copy
                && compare_target == CompareTarget::HeadToIndex;
            let can_revert_selection = git_available
                && has_file_path
                && has_selection
                && !is_untracked
                && !is_rename_or_copy
                && compare_target == CompareTarget::IndexToWorktree;

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

            let can_line_ops = git_available
                && has_file_path
                && row_ref.is_some()
                && !is_untracked
                && !is_rename_or_copy
                && kind != diffview::DiffRowKind::Unchanged;

            let can_stage_line = can_line_ops && compare_target == CompareTarget::IndexToWorktree;
            let can_unstage_line = can_line_ops && compare_target == CompareTarget::HeadToIndex;
            let can_revert_line = can_line_ops && compare_target == CompareTarget::IndexToWorktree;

            let stage_selection_label: SharedString = if is_rename_or_copy {
                "Stage 选中行（rename/copy 暂不支持）".into()
            } else {
                format!("Stage 选中行（{selection_count}）").into()
            };
            let unstage_selection_label: SharedString = if is_rename_or_copy {
                "Unstage 选中行（rename/copy 暂不支持）".into()
            } else {
                format!("Unstage 选中行（{selection_count}）").into()
            };
            let revert_selection_label: SharedString = if is_rename_or_copy {
                "Revert 选中行（rename/copy 暂不支持）".into()
            } else {
                format!("Revert 选中行（{selection_count}）").into()
            };

            let mut menu = menu;

            if row_ref.is_some() && kind != diffview::DiffRowKind::Unchanged {
                let row_ref_for_click = row_ref;

                let app_for_stage_line = app.clone();
                menu = menu.item(
                    PopupMenuItem::new("Stage 这一行")
                        .icon(Icon::new(IconName::Plus).xsmall())
                        .disabled(!can_stage_line)
                        .on_click(move |_, window, cx| {
                            let Some(row_ref) = row_ref_for_click else {
                                return;
                            };
                            app_for_stage_line.update(cx, |this, cx| {
                                if let Some(view) = this.diff_view.as_mut() {
                                    view.selected_rows.clear();
                                    view.selected_rows.insert(row_ref);
                                    view.selection_anchor = Some(row_ref);
                                }
                                this.stage_selected_rows(window, cx);
                                cx.notify();
                            });
                        }),
                );

                let row_ref_for_click = row_ref;
                let app_for_unstage_line = app.clone();
                menu = menu.item(
                    PopupMenuItem::new("Unstage 这一行")
                        .icon(Icon::new(IconName::Minus).xsmall())
                        .disabled(!can_unstage_line)
                        .on_click(move |_, window, cx| {
                            let Some(row_ref) = row_ref_for_click else {
                                return;
                            };
                            app_for_unstage_line.update(cx, |this, cx| {
                                if let Some(view) = this.diff_view.as_mut() {
                                    view.selected_rows.clear();
                                    view.selected_rows.insert(row_ref);
                                    view.selection_anchor = Some(row_ref);
                                }
                                this.unstage_selected_rows(window, cx);
                                cx.notify();
                            });
                        }),
                );

                let row_ref_for_click = row_ref;
                let app_for_revert_line = app.clone();
                menu = menu.item(
                    PopupMenuItem::new("Revert 这一行")
                        .icon(Icon::new(PlateIconName::Undo2).xsmall())
                        .disabled(!can_revert_line)
                        .on_click(move |_, window, cx| {
                            let Some(row_ref) = row_ref_for_click else {
                                return;
                            };
                            app_for_revert_line.update(cx, |this, cx| {
                                if let Some(view) = this.diff_view.as_mut() {
                                    view.selected_rows.clear();
                                    view.selected_rows.insert(row_ref);
                                    view.selection_anchor = Some(row_ref);
                                }
                                this.revert_selected_rows(window, cx);
                                cx.notify();
                            });
                        }),
                );

                menu = menu.item(PopupMenuItem::separator());
            }

            let app_for_stage_selection = app.clone();
            menu = menu.item(
                PopupMenuItem::new(stage_selection_label)
                    .icon(Icon::new(IconName::Plus).xsmall())
                    .disabled(!can_stage_selection)
                    .on_click(move |_, window, cx| {
                        app_for_stage_selection.update(cx, |this, cx| {
                            this.stage_selected_rows(window, cx);
                            cx.notify();
                        });
                    }),
            );

            let app_for_unstage_selection = app.clone();
            menu = menu.item(
                PopupMenuItem::new(unstage_selection_label)
                    .icon(Icon::new(IconName::Minus).xsmall())
                    .disabled(!can_unstage_selection)
                    .on_click(move |_, window, cx| {
                        app_for_unstage_selection.update(cx, |this, cx| {
                            this.unstage_selected_rows(window, cx);
                            cx.notify();
                        });
                    }),
            );

            let app_for_revert_selection = app.clone();
            menu = menu.item(
                PopupMenuItem::new(revert_selection_label)
                    .icon(Icon::new(PlateIconName::Undo2).xsmall())
                    .disabled(!can_revert_selection)
                    .on_click(move |_, window, cx| {
                        app_for_revert_selection.update(cx, |this, cx| {
                            this.revert_selected_rows(window, cx);
                            cx.notify();
                        });
                    }),
            );

            let app_for_clear_selection = app.clone();
            menu = menu.item(
                PopupMenuItem::new("清除选中")
                    .icon(Icon::new(IconName::Close).xsmall())
                    .disabled(!has_selection)
                    .on_click(move |_, _window, cx| {
                        app_for_clear_selection.update(cx, |this, cx| {
                            this.clear_diff_selection();
                            cx.notify();
                        });
                    }),
            );

            if hunk_index.is_some() {
                menu = menu.item(PopupMenuItem::separator());

                let app_for_stage_hunk = app.clone();
                let hunk_index_for_click = hunk_index;
                menu = menu.item(
                    PopupMenuItem::new("Stage 该 hunk")
                        .icon(Icon::new(IconName::Plus).xsmall())
                        .disabled(!can_stage_hunk)
                        .on_click(move |_, window, cx| {
                            let Some(hunk_index) = hunk_index_for_click else {
                                return;
                            };
                            app_for_stage_hunk.update(cx, |this, cx| {
                                if let Some(view) = this.diff_view.as_mut() {
                                    view.current_hunk = hunk_index
                                        .min(view.diff_model.hunks.len().saturating_sub(1));
                                }
                                this.stage_current_hunk(window, cx);
                                cx.notify();
                            });
                        }),
                );

                let app_for_unstage_hunk = app.clone();
                let hunk_index_for_click = hunk_index;
                menu = menu.item(
                    PopupMenuItem::new("Unstage 该 hunk")
                        .icon(Icon::new(IconName::Minus).xsmall())
                        .disabled(!can_unstage_hunk)
                        .on_click(move |_, window, cx| {
                            let Some(hunk_index) = hunk_index_for_click else {
                                return;
                            };
                            app_for_unstage_hunk.update(cx, |this, cx| {
                                if let Some(view) = this.diff_view.as_mut() {
                                    view.current_hunk = hunk_index
                                        .min(view.diff_model.hunks.len().saturating_sub(1));
                                }
                                this.unstage_current_hunk(window, cx);
                                cx.notify();
                            });
                        }),
                );

                let app_for_revert_hunk = app.clone();
                let hunk_index_for_click = hunk_index;
                menu = menu.item(
                    PopupMenuItem::new("Revert 该 hunk")
                        .icon(Icon::new(PlateIconName::Undo2).xsmall())
                        .disabled(!can_revert_hunk)
                        .on_click(move |_, window, cx| {
                            let Some(hunk_index) = hunk_index_for_click else {
                                return;
                            };
                            app_for_revert_hunk.update(cx, |this, cx| {
                                if let Some(view) = this.diff_view.as_mut() {
                                    view.current_hunk = hunk_index
                                        .min(view.diff_model.hunks.len().saturating_sub(1));
                                }
                                this.revert_current_hunk(window, cx);
                                cx.notify();
                            });
                        }),
                );
            }

            menu
        }
    }

    fn request_diff_rebuild(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(diff_view) = self.diff_view.as_ref() else {
            return;
        };

        self.diff_rebuild_seq = self.diff_rebuild_seq.wrapping_add(1);
        let rebuild_seq = self.diff_rebuild_seq;
        let content_revision = self.diff_content_revision;
        let compare_target = diff_view.compare_target.clone();
        let this = cx.entity();

        cx.spawn_in(window, async move |_, window| {
            Timer::after(Duration::from_millis(DIFF_REBUILD_DEBOUNCE_MS)).await;

            let snapshot = window
                .update(|_, cx| {
                    let app = this.read(cx);
                    if app.diff_rebuild_seq != rebuild_seq
                        || app.diff_content_revision != content_revision
                    {
                        return None;
                    }

                    let diff_view = app.diff_view.as_ref()?;
                    Some((
                        diff_view.title.clone(),
                        diff_view.path.clone(),
                        diff_view.status.clone(),
                        diff_view.old_text.clone(),
                        diff_view.new_text.clone(),
                        diff_view.scroll_handle.clone(),
                        diff_view.scroll_state.clone(),
                        diff_view.current_hunk,
                        app.diff_options,
                        app.view_mode,
                    ))
                })
                .ok()
                .flatten();

            let Some((
                title,
                path,
                status,
                old_text,
                new_text,
                scroll_handle,
                scroll_state,
                current_hunk,
                diff_options,
                view_mode,
            )) = snapshot
            else {
                return Some(());
            };

            let (old_text, new_text, model, old_lines, new_lines, rows, hunk_rows) = window
                .background_executor()
                .spawn(async move {
                    let (model, old_lines, new_lines) = build_diff_model(
                        &old_text,
                        &new_text,
                        diff_options.ignore_whitespace,
                        diff_options.context_lines,
                    );
                    let rows =
                        build_display_rows_from_model(&model, &old_lines, &new_lines, view_mode);
                    let hunk_rows = rows
                        .iter()
                        .enumerate()
                        .filter_map(|(index, row)| {
                            matches!(row, DisplayRow::HunkHeader { .. }).then_some(index)
                        })
                        .collect::<Vec<_>>();
                    (
                        old_text, new_text, model, old_lines, new_lines, rows, hunk_rows,
                    )
                })
                .await;

            window
                .update(|_window, cx| {
                    this.update(cx, |this, cx| {
                        if this.diff_rebuild_seq != rebuild_seq
                            || this.diff_content_revision != content_revision
                        {
                            return;
                        }

                        let still_same = this.diff_view.as_ref().is_some_and(|view| {
                            view.path == path && view.compare_target == compare_target
                        });
                        if !still_same {
                            return;
                        }

                        let key_path = path.clone();
                        let mut next = DiffViewState::from_precomputed_rows(
                            title,
                            path,
                            status,
                            compare_target.clone(),
                            view_mode,
                            old_text,
                            new_text,
                            model,
                            old_lines,
                            new_lines,
                            rows,
                            hunk_rows,
                        );
                        next.scroll_handle = scroll_handle;
                        next.scroll_state = scroll_state;
                        next.current_hunk =
                            current_hunk.min(next.hunk_rows.len().saturating_sub(1));
                        this.diff_view = Some(next);
                        this.current_diff_cache_key = key_path.map(|path| {
                            DiffCacheKey::new(path, compare_target.clone(), diff_options)
                        });
                        cx.notify();
                    });
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn jump_hunk(&mut self, direction: i32) {
        let Some(diff_view) = self.diff_view.as_mut() else {
            return;
        };
        if diff_view.hunk_rows.is_empty() {
            return;
        }

        let mut next_index = diff_view.current_hunk;
        if direction < 0 {
            next_index = next_index.saturating_sub(1);
        } else if direction > 0 {
            next_index = (next_index + 1).min(diff_view.hunk_rows.len().saturating_sub(1));
        }

        diff_view.current_hunk = next_index;
        let row_index = diff_view.hunk_rows[next_index];
        diff_view
            .scroll_handle
            .scroll_to_item(row_index, ScrollStrategy::Top);
    }

    fn expand_fold(&mut self, row_index: usize) {
        let Some(diff_view) = self.diff_view.as_mut() else {
            return;
        };

        let DisplayRow::Fold {
            old_start,
            new_start,
            len,
        } = diff_view
            .rows
            .get(row_index)
            .cloned()
            .unwrap_or(DisplayRow::Fold {
                old_start: 0,
                new_start: 0,
                len: 0,
            })
        else {
            return;
        };

        if len == 0 {
            diff_view.rows.remove(row_index);
            diff_view.recalc_hunk_rows();
            return;
        }

        let mut expanded = Vec::with_capacity(len);
        for offset in 0..len {
            let old_index = old_start + offset;
            let new_index = new_start + offset;

            let old_text = diff_view
                .old_lines
                .get(old_index)
                .cloned()
                .unwrap_or_default();
            let new_text = diff_view
                .new_lines
                .get(new_index)
                .cloned()
                .unwrap_or_else(|| old_text.clone());

            let old_segments = vec![diffview::DiffSegment {
                kind: diffview::DiffSegmentKind::Unchanged,
                text: old_text.clone(),
            }];
            let new_segments = vec![diffview::DiffSegment {
                kind: diffview::DiffSegmentKind::Unchanged,
                text: new_text.clone(),
            }];

            expanded.push(DisplayRow::Code {
                source: None,
                kind: diffview::DiffRowKind::Unchanged,
                old_line: Some(old_index + 1),
                new_line: Some(new_index + 1),
                old_segments,
                new_segments,
            });
        }

        diff_view.rows.splice(row_index..=row_index, expanded);
        diff_view.recalc_hunk_rows();
    }

    fn expand_all_folds(&mut self) {
        while self.diff_view.as_ref().is_some_and(|diff_view| {
            diff_view
                .rows
                .iter()
                .any(|row| matches!(row, DisplayRow::Fold { .. }))
        }) {
            let next_index = self
                .diff_view
                .as_ref()
                .and_then(|diff_view| {
                    diff_view.rows.iter().enumerate().find_map(|(index, row)| {
                        matches!(row, DisplayRow::Fold { .. }).then_some(index)
                    })
                })
                .unwrap_or(0);
            self.expand_fold(next_index);
        }
    }

    fn resolve_conflict(
        &mut self,
        conflict_index: usize,
        resolution: ConflictResolution,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(conflict_view) = self.conflict_view.as_mut() else {
            return;
        };
        if conflict_index >= conflict_view.conflicts.len() {
            return;
        }

        let region = conflict_view.conflicts[conflict_index].clone();
        let replacement = match resolution {
            ConflictResolution::Ours => conflict_view.text[region.ours.clone()].to_string(),
            ConflictResolution::Theirs => conflict_view.text[region.theirs.clone()].to_string(),
            ConflictResolution::Base => region
                .base
                .as_ref()
                .map(|range| conflict_view.text[range.clone()].to_string())
                .unwrap_or_default(),
            ConflictResolution::Both => format!(
                "{}{}",
                &conflict_view.text[region.ours.clone()],
                &conflict_view.text[region.theirs.clone()]
            ),
        };

        let mut next = String::with_capacity(
            conflict_view.text.len().saturating_sub(region.range.len()) + replacement.len(),
        );
        next.push_str(&conflict_view.text[..region.range.start]);
        next.push_str(&replacement);
        next.push_str(&conflict_view.text[region.range.end..]);

        conflict_view.text = next;
        conflict_view.rebuild();
        let updated_text = conflict_view.text.clone();
        let result_input = conflict_view.result_input.clone();
        result_input.update(cx, move |state, cx| {
            state.set_value(updated_text, window, cx);
        });
    }

    fn jump_conflict(&mut self, direction: i32) {
        let Some(conflict_view) = self.conflict_view.as_mut() else {
            return;
        };
        if conflict_view.conflict_rows.is_empty() {
            return;
        }

        let mut next_index = conflict_view.current_conflict;
        if direction < 0 {
            next_index = next_index.saturating_sub(1);
        } else if direction > 0 {
            next_index = (next_index + 1).min(conflict_view.conflict_rows.len().saturating_sub(1));
        }

        conflict_view.current_conflict = next_index;
        let row_index = conflict_view.conflict_rows[next_index];
        conflict_view
            .scroll_handle
            .scroll_to_item(row_index, ScrollStrategy::Top);
    }

    fn apply_conflict_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conflict_view) = self.conflict_view.as_mut() else {
            return;
        };

        let edited_text = conflict_view.result_input.read(cx).value().to_string();
        if edited_text == conflict_view.text {
            window.push_notification(Notification::new().message("结果编辑器无改动"), cx);
            return;
        }

        conflict_view.text = edited_text;
        conflict_view.rebuild();

        let remaining = conflict_view.conflicts.len();
        window.push_notification(
            Notification::new().message(if remaining == 0 {
                "已应用编辑：冲突已清零".to_string()
            } else {
                format!("已应用编辑：仍有 {remaining} 处冲突")
            }),
            cx,
        );
        cx.notify();
    }

    fn save_conflict_to_working_tree(
        &mut self,
        add_to_index: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if add_to_index && !self.git_available {
            window.push_notification(
                Notification::new().message("未检测到 git 命令，无法执行 git add"),
                cx,
            );
            return;
        }

        let Some(conflict_view) = self.conflict_view.as_mut() else {
            return;
        };
        let Some(path) = conflict_view.path.clone() else {
            return;
        };

        let edited_text = conflict_view.result_input.read(cx).value().to_string();
        if edited_text != conflict_view.text {
            conflict_view.text = edited_text;
            conflict_view.rebuild();
        }

        if !conflict_view.conflicts.is_empty() {
            window.push_notification(
                Notification::new().message("仍有冲突未解决，无法保存/标记已解决"),
                cx,
            );
            return;
        }

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let text = conflict_view.text.clone();
        let path_for_task = path.clone();

        window.push_notification(
            Notification::new().message(if add_to_index {
                format!("保存并标记已解决：{path}")
            } else {
                format!("保存到文件：{path}")
            }),
            cx,
        );

        cx.spawn_in(window, async move |_, window| {
            let (write_result, add_result, status_result) = window
                .background_executor()
                .spawn(async move {
                    let full_path = repo_root.join(&path_for_task);
                    let write_result = std::fs::write(&full_path, text.as_bytes())
                        .with_context(|| format!("写入文件失败：{path_for_task}"));

                    let add_result = if add_to_index && write_result.is_ok() {
                        let output = Command::new("git")
                            .arg("-C")
                            .arg(&repo_root)
                            .args(["add", "--"])
                            .arg(&path_for_task)
                            .output()
                            .with_context(|| format!("执行 git add 失败：{path_for_task}"));

                        match output {
                            Ok(output) if output.status.success() => Ok(()),
                            Ok(output) => Err(anyhow!(
                                "git add 返回非零（{}）：{}",
                                output.status.code().unwrap_or(-1),
                                String::from_utf8_lossy(&output.stderr).trim()
                            )),
                            Err(err) => Err(err),
                        }
                    } else {
                        Ok(())
                    };

                    let status_result = fetch_git_status(&repo_root);

                    (write_result, add_result, status_result)
                })
                .await;

            window
                .update(|window, cx| {
                    if let Err(err) = write_result {
                        window.push_notification(
                            Notification::new().message(format!("保存失败：{err}")),
                            cx,
                        );
                        return;
                    }
                    if let Err(err) = add_result {
                        window.push_notification(
                            Notification::new().message(format!("git add 失败：{err}")),
                            cx,
                        );
                    } else if add_to_index {
                        window.push_notification(
                            Notification::new().message("已保存并标记为已解决（git add）"),
                            cx,
                        );
                    } else {
                        window.push_notification(
                            Notification::new().message("已保存到工作区文件"),
                            cx,
                        );
                    }

                    if let Ok(entries) = status_result {
                        this.update(cx, |this, cx| {
                            this.files = entries;
                            this.clear_diff_cache();
                            this.invalidate_changes_tree();
                            cx.notify();
                        });
                    } else {
                        this.update(cx, |this, cx| {
                            this.clear_diff_cache();
                            this.invalidate_changes_tree();
                            cx.notify();
                        });
                    }
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn stage_current_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(diff_view) = self.diff_view.as_ref() else {
            return;
        };
        let Some(path) = diff_view.path.clone() else {
            return;
        };

        let compare_target = diff_view.compare_target.clone();
        if matches!(compare_target, CompareTarget::Refs { .. }) {
            window.push_notification(
                Notification::new()
                    .message("自定义对比模式下不支持 Stage 文件，请先切换到内置对比目标"),
                cx,
            );
            return;
        }
        let scroll_handle = diff_view.scroll_handle.clone();
        let scroll_state = diff_view.scroll_state.clone();
        let current_hunk = diff_view.current_hunk;

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let path_for_task = path.clone();
        let orig_path_for_task = diff_view.orig_path.clone();

        window.push_notification(Notification::new().message(format!("git add {path}")), cx);

        cx.spawn_in(window, async move |_, window| {
            let diff_options =
                window
                    .update(|_, cx| this.read(cx).diff_options)
                    .unwrap_or(DiffViewOptions {
                        ignore_whitespace: false,
                        context_lines: 3,
                    });
            let view_mode = window
                .update(|_, cx| this.read(cx).view_mode)
                .unwrap_or(DiffViewMode::Split);

            let path_for_task_bg = path_for_task.clone();
            let orig_path_for_task_bg = orig_path_for_task.clone();
            let compare_target_for_io = compare_target.clone();
            let (
                add_ok,
                add_err,
                entries,
                status_err,
                updated_status,
                updated_orig_path,
                old_text,
                old_err,
                new_text,
                new_err,
                model,
                old_lines,
                new_lines,
            ) = window
                .background_executor()
                .spawn(async move {
                    let mut add_args: Vec<String> = vec!["add".into()];
                    if let Some(orig_path) = orig_path_for_task_bg.as_deref() {
                        let orig_path = orig_path.trim();
                        if !orig_path.is_empty() && orig_path != path_for_task_bg {
                            add_args.push("-A".into());
                            add_args.push("--".into());
                            add_args.push(path_for_task_bg.clone());
                            add_args.push(orig_path.to_string());
                        } else {
                            add_args.push("--".into());
                            add_args.push(path_for_task_bg.clone());
                        }
                    } else {
                        add_args.push("--".into());
                        add_args.push(path_for_task_bg.clone());
                    }
                    let add_result = run_git(repo_root.as_path(), add_args);
                    let add_ok = add_result.is_ok();
                    let add_err = add_result.err().map(|err| err.to_string());
                    let (entries, status_err, updated_entry) = match fetch_git_status(&repo_root) {
                        Ok(entries) => {
                            let updated_entry = entries
                                .iter()
                                .find(|entry| entry.path == path_for_task_bg)
                                .cloned()
                                .unwrap_or(FileEntry {
                                    path: path_for_task_bg.clone(),
                                    status: String::new(),
                                    orig_path: orig_path_for_task_bg.clone(),
                                });
                            (Some(entries), None, updated_entry)
                        }
                        Err(err) => (
                            None,
                            Some(err.to_string()),
                            FileEntry {
                                path: path_for_task_bg.clone(),
                                status: String::new(),
                                orig_path: orig_path_for_task_bg.clone(),
                            },
                        ),
                    };
                    let updated_status = updated_entry.status.clone();
                    let updated_orig_path = updated_entry
                        .orig_path
                        .clone()
                        .or(orig_path_for_task_bg.clone());
                    let (old_path, new_path) = compare_paths_for_file(
                        &path_for_task_bg,
                        updated_orig_path.as_deref(),
                        &updated_status,
                        &compare_target_for_io,
                    );

                    let (old_text, old_err) = match compare_target_for_io.clone() {
                        CompareTarget::HeadToWorktree | CompareTarget::HeadToIndex => {
                            match read_head_file(&repo_root, &old_path, &updated_status) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::IndexToWorktree => {
                            match read_index_file(&repo_root, &old_path, &updated_status) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::Refs { .. } => {
                            (String::new(), Some("不支持的对比目标".to_string()))
                        }
                    };

                    let (new_text, new_err) = match compare_target_for_io {
                        CompareTarget::HeadToWorktree | CompareTarget::IndexToWorktree => {
                            match read_working_file(&repo_root, &new_path) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::HeadToIndex => {
                            match read_index_file(&repo_root, &new_path, &updated_status) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::Refs { .. } => {
                            (String::new(), Some("不支持的对比目标".to_string()))
                        }
                    };

                    let (model, old_lines, new_lines) = build_diff_model(
                        &old_text,
                        &new_text,
                        diff_options.ignore_whitespace,
                        diff_options.context_lines,
                    );

                    (
                        add_ok,
                        add_err,
                        entries,
                        status_err,
                        updated_status,
                        updated_orig_path,
                        old_text,
                        old_err,
                        new_text,
                        new_err,
                        model,
                        old_lines,
                        new_lines,
                    )
                })
                .await;

            window
                .update(|window, cx| {
                    if let Some(err) = add_err {
                        window.push_notification(
                            Notification::new().message(format!("git add 失败：{err}")),
                            cx,
                        );
                    } else {
                        window.push_notification(Notification::new().message("git add 成功"), cx);
                    }

                    if let Some(err) = status_err {
                        window.push_notification(
                            Notification::new().message(format!("刷新 git 状态失败：{err}")),
                            cx,
                        );
                    }

                    if let Some(err) = old_err {
                        window.push_notification(
                            Notification::new().message(format!(
                                "读取 {} 版本失败，按空内容处理：{err}",
                                compare_target_side_label(&compare_target, Side::Old)
                            )),
                            cx,
                        );
                    }
                    if let Some(err) = new_err {
                        window.push_notification(
                            Notification::new().message(format!(
                                "读取 {} 版本失败，按空内容处理：{err}",
                                compare_target_side_label(&compare_target, Side::New)
                            )),
                            cx,
                        );
                    }

                    this.update(cx, |this, cx| {
                        this.clear_diff_cache();
                        if let Some(entries) = entries {
                            this.files = entries;
                            this.invalidate_changes_tree();
                        }

                        if add_ok {
                            let title: SharedString = if updated_status.is_empty() {
                                path_for_task.clone().into()
                            } else {
                                format!("{updated_status} {path_for_task}").into()
                            };

                            if let Some(diff_view) = this.diff_view.as_ref() {
                                if diff_view.path.as_deref() == Some(path_for_task.as_str()) {
                                    let mut next = DiffViewState::from_precomputed(
                                        title,
                                        Some(path_for_task.clone()),
                                        (!updated_status.is_empty())
                                            .then(|| updated_status.clone()),
                                        compare_target,
                                        old_text,
                                        new_text,
                                        view_mode,
                                        model,
                                        old_lines,
                                        new_lines,
                                    );
                                    next.orig_path = updated_orig_path.clone();
                                    next.scroll_handle = scroll_handle.clone();
                                    next.scroll_state = scroll_state.clone();
                                    next.current_hunk =
                                        current_hunk.min(next.hunk_rows.len().saturating_sub(1));
                                    this.diff_content_revision =
                                        this.diff_content_revision.wrapping_add(1);
                                    this.diff_view = Some(next);
                                }
                            }
                        }

                        cx.notify();
                    });
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn unstage_current_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(diff_view) = self.diff_view.as_ref() else {
            return;
        };
        let Some(path) = diff_view.path.clone() else {
            return;
        };

        let compare_target = diff_view.compare_target.clone();
        if matches!(compare_target, CompareTarget::Refs { .. }) {
            window.push_notification(
                Notification::new()
                    .message("自定义对比模式下不支持 Unstage 文件，请先切换到内置对比目标"),
                cx,
            );
            return;
        }
        let scroll_handle = diff_view.scroll_handle.clone();
        let scroll_state = diff_view.scroll_state.clone();
        let current_hunk = diff_view.current_hunk;

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let path_for_task = path.clone();
        let orig_path_for_task = diff_view.orig_path.clone();

        window.push_notification(
            Notification::new().message(format!("git reset HEAD -- {path}")),
            cx,
        );

        cx.spawn_in(window, async move |_, window| {
            let diff_options =
                window
                    .update(|_, cx| this.read(cx).diff_options)
                    .unwrap_or(DiffViewOptions {
                        ignore_whitespace: false,
                        context_lines: 3,
                    });
            let view_mode = window
                .update(|_, cx| this.read(cx).view_mode)
                .unwrap_or(DiffViewMode::Split);

            let path_for_task_bg = path_for_task.clone();
            let orig_path_for_task_bg = orig_path_for_task.clone();
            let compare_target_for_io = compare_target.clone();
            let (
                reset_ok,
                reset_err,
                entries,
                status_err,
                updated_status,
                updated_orig_path,
                old_text,
                old_err,
                new_text,
                new_err,
                model,
                old_lines,
                new_lines,
            ) = window
                .background_executor()
                .spawn(async move {
                    let mut reset_args: Vec<String> = vec![
                        "reset".into(),
                        "HEAD".into(),
                        "--".into(),
                        path_for_task_bg.clone(),
                    ];
                    if let Some(orig_path) = orig_path_for_task_bg.as_deref() {
                        let orig_path = orig_path.trim();
                        if !orig_path.is_empty() && orig_path != path_for_task_bg {
                            reset_args.push(orig_path.to_string());
                        }
                    }
                    let reset_result = run_git(repo_root.as_path(), reset_args);
                    let reset_ok = reset_result.is_ok();
                    let reset_err = reset_result.err().map(|err| err.to_string());
                    let (entries, status_err, updated_entry) = match fetch_git_status(&repo_root) {
                        Ok(entries) => {
                            let updated_entry = entries
                                .iter()
                                .find(|entry| entry.path == path_for_task_bg)
                                .cloned()
                                .unwrap_or(FileEntry {
                                    path: path_for_task_bg.clone(),
                                    status: String::new(),
                                    orig_path: orig_path_for_task_bg.clone(),
                                });
                            (Some(entries), None, updated_entry)
                        }
                        Err(err) => (
                            None,
                            Some(err.to_string()),
                            FileEntry {
                                path: path_for_task_bg.clone(),
                                status: String::new(),
                                orig_path: orig_path_for_task_bg.clone(),
                            },
                        ),
                    };
                    let updated_status = updated_entry.status.clone();
                    let updated_orig_path = updated_entry
                        .orig_path
                        .clone()
                        .or(orig_path_for_task_bg.clone());
                    let (old_path, new_path) = compare_paths_for_file(
                        &path_for_task_bg,
                        updated_orig_path.as_deref(),
                        &updated_status,
                        &compare_target_for_io,
                    );

                    let (old_text, old_err) = match compare_target_for_io.clone() {
                        CompareTarget::HeadToWorktree | CompareTarget::HeadToIndex => {
                            match read_head_file(&repo_root, &old_path, &updated_status) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::IndexToWorktree => {
                            match read_index_file(&repo_root, &old_path, &updated_status) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::Refs { .. } => {
                            (String::new(), Some("不支持的对比目标".to_string()))
                        }
                    };

                    let (new_text, new_err) = match compare_target_for_io {
                        CompareTarget::HeadToWorktree | CompareTarget::IndexToWorktree => {
                            match read_working_file(&repo_root, &new_path) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::HeadToIndex => {
                            match read_index_file(&repo_root, &new_path, &updated_status) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::Refs { .. } => {
                            (String::new(), Some("不支持的对比目标".to_string()))
                        }
                    };

                    let (model, old_lines, new_lines) = build_diff_model(
                        &old_text,
                        &new_text,
                        diff_options.ignore_whitespace,
                        diff_options.context_lines,
                    );

                    (
                        reset_ok,
                        reset_err,
                        entries,
                        status_err,
                        updated_status,
                        updated_orig_path,
                        old_text,
                        old_err,
                        new_text,
                        new_err,
                        model,
                        old_lines,
                        new_lines,
                    )
                })
                .await;

            window
                .update(|window, cx| {
                    if let Some(err) = reset_err {
                        window.push_notification(
                            Notification::new().message(format!("unstage 失败：{err}")),
                            cx,
                        );
                    } else {
                        window.push_notification(Notification::new().message("unstage 成功"), cx);
                    }

                    if let Some(err) = status_err {
                        window.push_notification(
                            Notification::new().message(format!("刷新 git 状态失败：{err}")),
                            cx,
                        );
                    }

                    if let Some(err) = old_err {
                        window.push_notification(
                            Notification::new().message(format!(
                                "读取 {} 版本失败，按空内容处理：{err}",
                                compare_target_side_label(&compare_target, Side::Old)
                            )),
                            cx,
                        );
                    }
                    if let Some(err) = new_err {
                        window.push_notification(
                            Notification::new().message(format!(
                                "读取 {} 版本失败，按空内容处理：{err}",
                                compare_target_side_label(&compare_target, Side::New)
                            )),
                            cx,
                        );
                    }

                    this.update(cx, |this, cx| {
                        if let Some(entries) = entries {
                            this.files = entries;
                            this.clear_diff_cache();
                            this.invalidate_changes_tree();
                        }

                        if reset_ok {
                            let title: SharedString = if updated_status.is_empty() {
                                path_for_task.clone().into()
                            } else {
                                format!("{updated_status} {path_for_task}").into()
                            };

                            if let Some(diff_view) = this.diff_view.as_ref() {
                                if diff_view.path.as_deref() == Some(path_for_task.as_str()) {
                                    let mut next = DiffViewState::from_precomputed(
                                        title,
                                        Some(path_for_task.clone()),
                                        (!updated_status.is_empty())
                                            .then(|| updated_status.clone()),
                                        compare_target,
                                        old_text,
                                        new_text,
                                        view_mode,
                                        model,
                                        old_lines,
                                        new_lines,
                                    );
                                    next.orig_path = updated_orig_path.clone();
                                    next.scroll_handle = scroll_handle.clone();
                                    next.scroll_state = scroll_state.clone();
                                    next.current_hunk =
                                        current_hunk.min(next.hunk_rows.len().saturating_sub(1));
                                    this.diff_content_revision =
                                        this.diff_content_revision.wrapping_add(1);
                                    this.diff_view = Some(next);
                                }
                            }
                        }

                        cx.notify();
                    });
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn stage_path(&mut self, entry: FileEntry, window: &mut Window, cx: &mut Context<Self>) {
        if !self.git_available {
            window.push_notification(
                Notification::new().message("未检测到 git 命令，无法执行 git add"),
                cx,
            );
            return;
        }
        if self.loading {
            return;
        }

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let path_for_task = entry.path.clone();
        let orig_for_task = entry.orig_path.clone();
        window.push_notification(
            Notification::new().message(format!("git add {path_for_task}")),
            cx,
        );

        cx.spawn_in(window, async move |_, window| {
            let result = window
                .background_executor()
                .spawn(async move {
                    let mut args: Vec<String> = vec!["add".into()];
                    if let Some(orig_path) = orig_for_task.as_deref() {
                        let orig_path = orig_path.trim();
                        if !orig_path.is_empty() && orig_path != path_for_task {
                            args.push("-A".into());
                            args.push("--".into());
                            args.push(path_for_task.clone());
                            args.push(orig_path.to_string());
                        } else {
                            args.push("--".into());
                            args.push(path_for_task.clone());
                        }
                    } else {
                        args.push("--".into());
                        args.push(path_for_task.clone());
                    }

                    run_git(repo_root.as_path(), args)
                        .with_context(|| format!("执行 git add 失败：{path_for_task}"))
                })
                .await;

            window
                .update(|window, cx| {
                    match result {
                        Ok(()) => window
                            .push_notification(Notification::new().message("git add 成功"), cx),
                        Err(err) => window.push_notification(
                            Notification::new().message(format!("git add 失败：{err:#}")),
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

    fn unstage_path(&mut self, entry: FileEntry, window: &mut Window, cx: &mut Context<Self>) {
        if !self.git_available {
            window.push_notification(
                Notification::new().message("未检测到 git 命令，无法执行 git reset"),
                cx,
            );
            return;
        }
        if self.loading {
            return;
        }

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let path_for_task = entry.path.clone();
        let orig_for_task = entry.orig_path.clone();
        window.push_notification(
            Notification::new().message(format!("git reset HEAD -- {path_for_task}")),
            cx,
        );

        cx.spawn_in(window, async move |_, window| {
            let result = window
                .background_executor()
                .spawn(async move {
                    let mut args: Vec<String> = vec![
                        "reset".into(),
                        "HEAD".into(),
                        "--".into(),
                        path_for_task.clone(),
                    ];
                    if let Some(orig_path) = orig_for_task.as_deref() {
                        let orig_path = orig_path.trim();
                        if !orig_path.is_empty() && orig_path != path_for_task {
                            args.push(orig_path.to_string());
                        }
                    }

                    run_git(repo_root.as_path(), args)
                        .with_context(|| format!("执行 git reset 失败：{path_for_task}"))
                })
                .await;

            window
                .update(|window, cx| {
                    match result {
                        Ok(()) => window
                            .push_notification(Notification::new().message("git reset 成功"), cx),
                        Err(err) => window.push_notification(
                            Notification::new().message(format!("git reset 失败：{err:#}")),
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

    fn rollback_path(
        &mut self,
        path: String,
        status: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        if is_ignored_status(&status) {
            return;
        }

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let path_for_task = path.clone();
        let status_for_task = status.clone();
        window.push_notification(Notification::new().message(format!("Rollback {path}")), cx);

        cx.spawn_in(window, async move |_, window| {
            let result = window
                .background_executor()
                .spawn(async move {
                    rollback_path_on_disk(repo_root.as_path(), &path_for_task, &status_for_task)
                })
                .await;

            window
                .update(|window, cx| {
                    match result {
                        Ok(message) => {
                            window.push_notification(Notification::new().message(message), cx)
                        }
                        Err(err) => window.push_notification(
                            Notification::new().message(format!("Rollback 失败：{err:#}")),
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

    fn rollback_paths(
        &mut self,
        entries: Vec<FileEntry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

        let mut entries = entries;
        entries.retain(|entry| {
            !is_ignored_status(&entry.status) && !is_conflict_status(&entry.status)
        });
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        entries.dedup_by(|a, b| a.path == b.path);

        if entries.is_empty() {
            return;
        }

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let count = entries.len();
        window.push_notification(
            Notification::new().message(format!("Rollback {count} file(s)…")),
            cx,
        );

        cx.spawn_in(window, async move |_, window| {
            let result = window
                .background_executor()
                .spawn(async move {
                    let mut ok = 0usize;
                    let mut err = 0usize;
                    let mut first_err: Option<String> = None;

                    for entry in entries {
                        match rollback_path_on_disk(repo_root.as_path(), &entry.path, &entry.status)
                        {
                            Ok(_) => ok += 1,
                            Err(e) => {
                                err += 1;
                                if first_err.is_none() {
                                    first_err = Some(format!("{e:#}"));
                                }
                            }
                        }
                    }

                    (ok, err, first_err)
                })
                .await;

            window
                .update(|window, cx| {
                    let (ok, err, first_err) = result;
                    if err == 0 {
                        window.push_notification(
                            Notification::new().message(format!("Rollback 完成：{ok} file(s)")),
                            cx,
                        );
                    } else {
                        let mut message = format!("Rollback 完成：成功 {ok}，失败 {err}");
                        if let Some(first_err) = first_err {
                            message.push_str(&format!("\n{first_err}"));
                        }
                        window.push_notification(Notification::new().message(message), cx);
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

    fn reveal_in_file_manager(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |_, window| {
            let result = window
                .background_executor()
                .spawn(async move { reveal_path_in_file_manager(&path) })
                .await;

            window
                .update(|window, cx| {
                    if let Err(err) = result {
                        window.push_notification(
                            Notification::new().message(format!("打开文件管理器失败：{err:#}")),
                            cx,
                        );
                    }
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn stage_current_hunk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_current_hunk(HunkApplyAction::Stage, window, cx);
    }

    fn unstage_current_hunk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_current_hunk(HunkApplyAction::Unstage, window, cx);
    }

    fn revert_current_hunk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_current_hunk(HunkApplyAction::Revert, window, cx);
    }

    fn stage_selected_rows(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_selected_rows(HunkApplyAction::Stage, window, cx);
    }

    fn unstage_selected_rows(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_selected_rows(HunkApplyAction::Unstage, window, cx);
    }

    fn revert_selected_rows(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_selected_rows(HunkApplyAction::Revert, window, cx);
    }

    fn apply_selected_rows(
        &mut self,
        action: HunkApplyAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(diff_view) = self.diff_view.as_ref() else {
            return;
        };

        let required_target = action.required_compare_target();
        if diff_view.compare_target != required_target {
            window.push_notification(
                Notification::new().message(format!(
                    "当前对比为 {}，请切换到 {} 再执行 {}",
                    compare_target_label(&diff_view.compare_target),
                    compare_target_label(&required_target),
                    action.label()
                )),
                cx,
            );
            return;
        }

        let Some(path) = diff_view.path.clone() else {
            return;
        };

        if diff_view.selected_rows.is_empty() {
            window.push_notification(
                Notification::new()
                    .message(format!("{}：请先在 Diff 中选中要操作的行", action.label())),
                cx,
            );
            return;
        }

        let status = diff_view.status.as_deref().unwrap_or_default();
        if is_untracked_status(status) {
            window.push_notification(
                Notification::new()
                    .message("Untracked 文件不支持行级 Stage/Unstage/Revert（请使用文件级操作）"),
                cx,
            );
            return;
        }
        let is_rename_or_copy = status_xy(status)
            .is_some_and(|(x, y)| matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C'));
        if is_rename_or_copy {
            window.push_notification(
                Notification::new().message(format!(
                    "{}：rename/copy 文件暂不支持 hunk/行级操作",
                    action.label()
                )),
                cx,
            );
            return;
        }

        let selection = diff_view.selected_rows.clone();
        let unselected_context = match action {
            HunkApplyAction::Stage => PatchUnselectedContext::Old,
            HunkApplyAction::Unstage | HunkApplyAction::Revert => PatchUnselectedContext::New,
        };

        let Some(patch) = unified_patch_for_selection(
            &path,
            &diff_view.diff_model,
            &selection,
            unselected_context,
        ) else {
            window.push_notification(
                Notification::new()
                    .message("未选择可应用的变更（提示：仅支持选中新增/删除/修改行）"),
                cx,
            );
            return;
        };

        let compare_target = diff_view.compare_target.clone();
        let scroll_handle = diff_view.scroll_handle.clone();
        let scroll_state = diff_view.scroll_state.clone();
        let current_hunk = diff_view.current_hunk;

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let path_for_task = path.clone();

        window.push_notification(
            Notification::new().message(format!(
                "{} 选中行（{}）: {path_for_task}",
                action.label(),
                selection.len()
            )),
            cx,
        );

        cx.spawn_in(window, async move |_, window| {
            let diff_options =
                window
                    .update(|_, cx| this.read(cx).diff_options)
                    .unwrap_or(DiffViewOptions {
                        ignore_whitespace: false,
                        context_lines: 3,
                    });
            let view_mode = window
                .update(|_, cx| this.read(cx).view_mode)
                .unwrap_or(DiffViewMode::Split);

            let path_for_task_bg = path_for_task.clone();
            let compare_target_for_io = compare_target.clone();
            let (
                apply_result,
                entries,
                status_err,
                updated_status,
                old_text,
                old_err,
                new_text,
                new_err,
                model,
                old_lines,
                new_lines,
            ) = window
                .background_executor()
                .spawn(async move {
                    let apply_result =
                        run_git_with_stdin(repo_root.as_path(), action.git_args(), &patch);
                    let (entries, status_err, updated_status) = match fetch_git_status(&repo_root) {
                        Ok(entries) => {
                            let updated_status = entries
                                .iter()
                                .find(|entry| entry.path == path_for_task_bg)
                                .map(|entry| entry.status.clone())
                                .unwrap_or_default();
                            (Some(entries), None, updated_status)
                        }
                        Err(err) => (None, Some(err.to_string()), String::new()),
                    };

                    let (old_text, old_err) = match compare_target_for_io.clone() {
                        CompareTarget::HeadToWorktree | CompareTarget::HeadToIndex => {
                            match read_head_file(&repo_root, &path_for_task_bg, &updated_status) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::IndexToWorktree => {
                            match read_index_file(&repo_root, &path_for_task_bg, &updated_status) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::Refs { .. } => {
                            (String::new(), Some("不支持的对比目标".to_string()))
                        }
                    };

                    let (new_text, new_err) = match compare_target_for_io {
                        CompareTarget::HeadToWorktree | CompareTarget::IndexToWorktree => {
                            match read_working_file(&repo_root, &path_for_task_bg) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::HeadToIndex => {
                            match read_index_file(&repo_root, &path_for_task_bg, &updated_status) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::Refs { .. } => {
                            (String::new(), Some("不支持的对比目标".to_string()))
                        }
                    };

                    let (model, old_lines, new_lines) = build_diff_model(
                        &old_text,
                        &new_text,
                        diff_options.ignore_whitespace,
                        diff_options.context_lines,
                    );

                    (
                        apply_result,
                        entries,
                        status_err,
                        updated_status,
                        old_text,
                        old_err,
                        new_text,
                        new_err,
                        model,
                        old_lines,
                        new_lines,
                    )
                })
                .await;

            window
                .update(|window, cx| {
                    if let Err(err) = apply_result {
                        window.push_notification(
                            Notification::new().message(format!("{} 失败：{err}", action.label())),
                            cx,
                        );
                        return;
                    }

                    window.push_notification(
                        Notification::new().message(format!("{} 成功", action.label())),
                        cx,
                    );

                    if let Some(err) = status_err {
                        window.push_notification(
                            Notification::new().message(format!("刷新 git 状态失败：{err}")),
                            cx,
                        );
                    }

                    if let Some(err) = old_err {
                        window.push_notification(
                            Notification::new().message(format!(
                                "读取 {} 版本失败，按空内容处理：{err}",
                                compare_target_side_label(&compare_target, Side::Old)
                            )),
                            cx,
                        );
                    }
                    if let Some(err) = new_err {
                        window.push_notification(
                            Notification::new().message(format!(
                                "读取 {} 版本失败，按空内容处理：{err}",
                                compare_target_side_label(&compare_target, Side::New)
                            )),
                            cx,
                        );
                    }

                    this.update(cx, |this, cx| {
                        if let Some(entries) = entries {
                            this.files = entries;
                        }

                        let title: SharedString = if updated_status.is_empty() {
                            path_for_task.clone().into()
                        } else {
                            format!("{updated_status} {path_for_task}").into()
                        };

                        if let Some(diff_view) = this.diff_view.as_ref() {
                            if diff_view.path.as_deref() == Some(path_for_task.as_str()) {
                                let mut next = DiffViewState::from_precomputed(
                                    title,
                                    Some(path_for_task.clone()),
                                    (!updated_status.is_empty()).then(|| updated_status.clone()),
                                    compare_target.clone(),
                                    old_text,
                                    new_text,
                                    view_mode,
                                    model,
                                    old_lines,
                                    new_lines,
                                );
                                next.scroll_handle = scroll_handle.clone();
                                next.scroll_state = scroll_state.clone();
                                next.current_hunk =
                                    current_hunk.min(next.hunk_rows.len().saturating_sub(1));
                                this.diff_content_revision =
                                    this.diff_content_revision.wrapping_add(1);
                                this.diff_view = Some(next);
                                this.current_diff_cache_key = Some(DiffCacheKey::new(
                                    path_for_task.clone(),
                                    compare_target.clone(),
                                    diff_options,
                                ));
                            } else {
                                this.current_diff_cache_key = None;
                            }
                        } else {
                            this.current_diff_cache_key = None;
                        }

                        cx.notify();
                    });
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn apply_current_hunk(
        &mut self,
        action: HunkApplyAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(diff_view) = self.diff_view.as_ref() else {
            return;
        };

        let required_target = action.required_compare_target();
        if diff_view.compare_target != required_target {
            window.push_notification(
                Notification::new().message(format!(
                    "当前对比为 {}，请切换到 {} 再执行 {}",
                    compare_target_label(&diff_view.compare_target),
                    compare_target_label(&required_target),
                    action.label()
                )),
                cx,
            );
            return;
        }

        let Some(path) = diff_view.path.clone() else {
            return;
        };

        let hunk_count = diff_view.diff_model.hunks.len();
        if hunk_count == 0 {
            return;
        }
        let hunk_index = diff_view.current_hunk.min(hunk_count.saturating_sub(1));
        let patch = unified_patch_for_hunk(&path, &diff_view.diff_model.hunks[hunk_index]);

        let compare_target = diff_view.compare_target.clone();
        let scroll_handle = diff_view.scroll_handle.clone();
        let scroll_state = diff_view.scroll_state.clone();
        let current_hunk = diff_view.current_hunk;

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let path_for_task = path.clone();

        window.push_notification(
            Notification::new().message(format!(
                "{} hunk {}/{}: {path_for_task}",
                action.label(),
                hunk_index + 1,
                hunk_count
            )),
            cx,
        );

        cx.spawn_in(window, async move |_, window| {
            let diff_options =
                window
                    .update(|_, cx| this.read(cx).diff_options)
                    .unwrap_or(DiffViewOptions {
                        ignore_whitespace: false,
                        context_lines: 3,
                    });
            let view_mode = window
                .update(|_, cx| this.read(cx).view_mode)
                .unwrap_or(DiffViewMode::Split);

            let path_for_task_bg = path_for_task.clone();
            let compare_target_for_io = compare_target.clone();
            let (
                apply_result,
                entries,
                status_err,
                updated_status,
                old_text,
                old_err,
                new_text,
                new_err,
                model,
                old_lines,
                new_lines,
            ) = window
                .background_executor()
                .spawn(async move {
                    let apply_result =
                        run_git_with_stdin(repo_root.as_path(), action.git_args(), &patch);
                    let (entries, status_err, updated_status) = match fetch_git_status(&repo_root) {
                        Ok(entries) => {
                            let updated_status = entries
                                .iter()
                                .find(|entry| entry.path == path_for_task_bg)
                                .map(|entry| entry.status.clone())
                                .unwrap_or_default();
                            (Some(entries), None, updated_status)
                        }
                        Err(err) => (None, Some(err.to_string()), String::new()),
                    };

                    let (old_text, old_err) = match compare_target_for_io.clone() {
                        CompareTarget::HeadToWorktree | CompareTarget::HeadToIndex => {
                            match read_head_file(&repo_root, &path_for_task_bg, &updated_status) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::IndexToWorktree => {
                            match read_index_file(&repo_root, &path_for_task_bg, &updated_status) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::Refs { .. } => {
                            (String::new(), Some("不支持的对比目标".to_string()))
                        }
                    };

                    let (new_text, new_err) = match compare_target_for_io {
                        CompareTarget::HeadToWorktree | CompareTarget::IndexToWorktree => {
                            match read_working_file(&repo_root, &path_for_task_bg) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::HeadToIndex => {
                            match read_index_file(&repo_root, &path_for_task_bg, &updated_status) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::Refs { .. } => {
                            (String::new(), Some("不支持的对比目标".to_string()))
                        }
                    };

                    let (model, old_lines, new_lines) = build_diff_model(
                        &old_text,
                        &new_text,
                        diff_options.ignore_whitespace,
                        diff_options.context_lines,
                    );

                    (
                        apply_result,
                        entries,
                        status_err,
                        updated_status,
                        old_text,
                        old_err,
                        new_text,
                        new_err,
                        model,
                        old_lines,
                        new_lines,
                    )
                })
                .await;

            window
                .update(|window, cx| {
                    if let Err(err) = apply_result {
                        window.push_notification(
                            Notification::new().message(format!("{} 失败：{err}", action.label())),
                            cx,
                        );
                        return;
                    }

                    window.push_notification(
                        Notification::new().message(format!("{} 成功", action.label())),
                        cx,
                    );

                    if let Some(err) = status_err {
                        window.push_notification(
                            Notification::new().message(format!("刷新 git 状态失败：{err}")),
                            cx,
                        );
                    }

                    if let Some(err) = old_err {
                        window.push_notification(
                            Notification::new().message(format!(
                                "读取 {} 版本失败，按空内容处理：{err}",
                                compare_target_side_label(&compare_target, Side::Old)
                            )),
                            cx,
                        );
                    }
                    if let Some(err) = new_err {
                        window.push_notification(
                            Notification::new().message(format!(
                                "读取 {} 版本失败，按空内容处理：{err}",
                                compare_target_side_label(&compare_target, Side::New)
                            )),
                            cx,
                        );
                    }

                    this.update(cx, |this, cx| {
                        if let Some(entries) = entries {
                            this.files = entries;
                        }

                        let title: SharedString = if updated_status.is_empty() {
                            path_for_task.clone().into()
                        } else {
                            format!("{updated_status} {path_for_task}").into()
                        };

                        if let Some(diff_view) = this.diff_view.as_ref() {
                            if diff_view.path.as_deref() == Some(path_for_task.as_str()) {
                                let mut next = DiffViewState::from_precomputed(
                                    title,
                                    Some(path_for_task.clone()),
                                    (!updated_status.is_empty()).then(|| updated_status.clone()),
                                    compare_target.clone(),
                                    old_text,
                                    new_text,
                                    view_mode,
                                    model,
                                    old_lines,
                                    new_lines,
                                );
                                next.scroll_handle = scroll_handle.clone();
                                next.scroll_state = scroll_state.clone();
                                next.current_hunk =
                                    current_hunk.min(next.hunk_rows.len().saturating_sub(1));
                                this.diff_content_revision =
                                    this.diff_content_revision.wrapping_add(1);
                                this.diff_view = Some(next);
                                this.current_diff_cache_key = Some(DiffCacheKey::new(
                                    path_for_task.clone(),
                                    compare_target.clone(),
                                    diff_options,
                                ));
                            } else {
                                this.current_diff_cache_key = None;
                            }
                        } else {
                            this.current_diff_cache_key = None;
                        }

                        cx.notify();
                    });
                })
                .ok();

            Some(())
        })
        .detach();
    }

    #[allow(dead_code)]
    fn render_status_list(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Div {
        let counts = StatusCounts::from_entries(&self.files);
        let filtered_count = self
            .files
            .iter()
            .filter(|entry| matches_filter(entry, self.status_filter))
            .count();

        let header: SharedString = if !self.git_available {
            "未检测到 git 命令：仅可运行 Diff/Conflict Demo（请安装 git 或配置 PATH）".into()
        } else if self.loading {
            "正在加载 git 状态…".into()
        } else {
            format!("显示 {} / {} 个文件", filtered_count, counts.all).into()
        };

        let demo_button = Button::new("open-demo")
            .label("打开 Diff Demo")
            .ghost()
            .on_click(cx.listener(|this, _, _window, cx| {
                this.open_demo();
                cx.notify();
            }));

        let large_demo_button = Button::new("open-large-demo")
            .label("打开 Large Diff Demo")
            .ghost()
            .on_click(cx.listener(|this, _, window, cx| {
                this.open_large_demo(window, cx);
                cx.notify();
            }));

        let conflict_demo_button = Button::new("open-conflict-demo")
            .label("打开 Conflict Demo")
            .ghost()
            .on_click(cx.listener(|this, _, window, cx| {
                this.open_conflict_demo(window, cx);
                cx.notify();
            }));

        let filter_button = |filter: StatusFilter, id: &'static str, label: String| {
            let mut button = Button::new(id).label(label);
            if self.status_filter == filter {
                button = button.primary();
            } else {
                button = button
                    .ghost()
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.status_filter = filter;
                        cx.notify();
                    }));
            }
            button
        };

        let filter_bar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .child(filter_button(
                StatusFilter::All,
                "filter-all",
                format!("All {}", counts.all),
            ))
            .child(filter_button(
                StatusFilter::Conflicts,
                "filter-conflicts",
                format!("Conflicts {}", counts.conflicts),
            ))
            .child(filter_button(
                StatusFilter::Staged,
                "filter-staged",
                format!("Staged {}", counts.staged),
            ))
            .child(filter_button(
                StatusFilter::Unstaged,
                "filter-unstaged",
                format!("Unstaged {}", counts.unstaged),
            ))
            .child(filter_button(
                StatusFilter::Untracked,
                "filter-untracked",
                format!("Untracked {}", counts.untracked),
            ));

        let list: Vec<AnyElement> = if self.loading {
            vec![div().child("加载中…").into_any_element()]
        } else if self.files.is_empty() {
            vec![div().child("没有检测到变更文件").into_any_element()]
        } else {
            self.files
                .iter()
                .filter(|entry| matches_filter(entry, self.status_filter))
                .enumerate()
                .map(|(index, entry)| {
                    let entry = entry.clone();
                    Button::new(("file", index))
                        .label(format!("{} {}", entry.status, entry.path))
                        .w_full()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            println!("[git-viewer] 打开文件: {} {}", entry.status, entry.path);
                            this.open_file(entry.clone(), window, cx);
                            cx.notify();
                        }))
                        .into_any_element()
                })
                .collect()
        };

        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(12.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .child(div().child(header))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.))
                            .child(demo_button)
                            .child(large_demo_button)
                            .child(conflict_demo_button),
                    ),
            )
            .child(filter_bar)
            .child(div().flex_col().gap(px(6.)).children(list))
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
            .child(
                div()
                    .text_lg()
                    .child("Git Viewer 希望提供更简单易用的 git diff 视图"),
            )
            .child(div().text_base().child("而不是代替所有 git 命令行操作"))
    }

    fn render_file_history_overlay(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let overlay = self.file_history_overlay.as_mut()?;
        let theme = cx.theme();
        let app = cx.entity();

        let loading = overlay.loading;
        let mode = overlay.mode;
        let filter_input = overlay.filter_input.clone();
        let title = overlay.path.clone();

        let query = overlay.filter_input.read(cx).value().to_string();
        let filtered = filter_commits(&overlay.commits, &query);

        let selected = overlay.selected.min(filtered.len().saturating_sub(1));

        let can_apply = !loading && !filtered.is_empty();

        let list: Vec<AnyElement> = if loading {
            vec![
                div()
                    .px(px(12.))
                    .py(px(10.))
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("加载历史中…")
                    .into_any_element(),
            ]
        } else if filtered.is_empty() {
            vec![
                div()
                    .px(px(12.))
                    .py(px(10.))
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("没有匹配的 commit")
                    .into_any_element(),
            ]
        } else {
            filtered
                .iter()
                .take(120)
                .enumerate()
                .map(|(index, entry)| {
                    let is_selected = index == selected;
                    let label = format!("{}  {}", entry.short_hash, entry.subject);
                    let app = app.clone();
                    div()
                        .id(("history-commit", index))
                        .flex()
                        .flex_row()
                        .items_center()
                        .h(px(32.))
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
                                        if let Some(overlay) = this.file_history_overlay.as_mut() {
                                            overlay.selected = index;
                                        }
                                        cx.notify();
                                    });
                                })
                        })
                        .child(div().truncate().child(label))
                        .into_any_element()
                })
                .collect()
        };

        let mode_parent_active = mode == HistoryCompareMode::ParentToCommit;
        let mode_worktree_active = mode == HistoryCompareMode::CommitToWorktree;

        let overlay_container = div()
            .id("file-history-overlay")
            .w(px(720.))
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
                        this.handle_file_history_overlay_key(event, window, cx)
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
                        div().flex().flex_col().gap(px(2.)).child("历史对比").child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .truncate()
                                .child(title),
                        ),
                    )
                    .child(
                        Button::new("history-overlay-close")
                            .label("关闭 (Esc)")
                            .ghost()
                            .on_click({
                                let app = app.clone();
                                move |_, window, cx| {
                                    app.update(cx, |this, cx| {
                                        this.close_file_history_overlay(window, cx);
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
                    .child(Input::new(&filter_input).w_full())
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                Button::new("history-mode-parent")
                                    .label("变更（parent→commit）")
                                    .when(mode_parent_active, |this| this.primary())
                                    .when(!mode_parent_active, |this| this.ghost())
                                    .on_click({
                                        let app = app.clone();
                                        move |_, _window, cx| {
                                            app.update(cx, |this, cx| {
                                                if let Some(overlay) =
                                                    this.file_history_overlay.as_mut()
                                                {
                                                    overlay.mode =
                                                        HistoryCompareMode::ParentToCommit;
                                                }
                                                cx.notify();
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Button::new("history-mode-worktree")
                                    .label("对比工作区")
                                    .when(mode_worktree_active, |this| this.primary())
                                    .when(!mode_worktree_active, |this| this.ghost())
                                    .on_click({
                                        let app = app.clone();
                                        move |_, _window, cx| {
                                            app.update(cx, |this, cx| {
                                                if let Some(overlay) =
                                                    this.file_history_overlay.as_mut()
                                                {
                                                    overlay.mode =
                                                        HistoryCompareMode::CommitToWorktree;
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
                    .id("file-history-overlay-list")
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
                            .child("↑↓ 选择 · Enter 打开 · Esc 关闭"),
                    )
                    .child(
                        Button::new("history-overlay-apply")
                            .label("打开对比")
                            .primary()
                            .disabled(!can_apply)
                            .on_click({
                                let app = app.clone();
                                move |_, window, cx| {
                                    app.update(cx, |this, cx| {
                                        this.apply_selected_file_history_commit(window, cx);
                                    });
                                }
                            }),
                    ),
            );

        Some(
            div()
                .id("file-history-overlay-backdrop")
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
                            this.close_file_history_overlay(window, cx);
                        });
                    }
                })
                .child(overlay_container)
                .into_any_element(),
        )
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

        let mut list: Vec<AnyElement> = Vec::new();
        if filtered.is_empty() {
            list.push(
                div()
                    .px(px(12.))
                    .py(px(10.))
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("没有匹配命令")
                    .into_any_element(),
            );
        } else {
            for (index, item) in filtered.iter().take(80).enumerate() {
                let is_selected = index == selected;
                let enabled = self.command_palette_command_enabled(item.command);
                let app_for_click = app.clone();
                let command = item.command;

                let mut row = div()
                    .id(("command-palette-item", index))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .h(px(32.))
                    .px(px(10.))
                    .rounded(px(6.))
                    .text_sm()
                    .child(div().truncate().child(item.title));

                if enabled {
                    row = row
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
                                    app_for_click.update(cx, |this, cx| {
                                        if let Some(overlay) = this.command_palette_overlay.as_mut()
                                        {
                                            overlay.selected = index;
                                        }
                                        this.run_command_palette_command(command, window, cx);
                                        this.close_command_palette(window, cx);
                                    });
                                })
                        });
                } else {
                    row = row
                        .bg(theme.transparent)
                        .text_color(theme.muted_foreground)
                        .cursor_default();
                }

                list.push(row.into_any_element());
            }
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

fn filter_commits<'a>(commits: &'a [CommitEntry], query: &str) -> Vec<&'a CommitEntry> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return commits.iter().collect();
    }

    let tokens: Vec<&str> = query.split_whitespace().filter(|t| !t.is_empty()).collect();
    if tokens.is_empty() {
        return commits.iter().collect();
    }

    commits
        .iter()
        .filter(|entry| {
            let hash = entry.hash.to_lowercase();
            let short_hash = entry.short_hash.to_lowercase();
            let subject = entry.subject.to_lowercase();
            tokens.iter().all(|token| {
                hash.contains(token) || short_hash.contains(token) || subject.contains(token)
            })
        })
        .collect()
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
