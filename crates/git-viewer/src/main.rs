use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use anyhow::Context as _;
use anyhow::{Result, anyhow};
use gpui::StatefulInteractiveElement as _;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui::{KeyBinding, actions};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Root, TitleBar, VirtualListScrollHandle, WindowExt as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
    notification::Notification,
    popover::Popover,
    scroll::{Scrollbar, ScrollbarState},
    v_virtual_list,
};

const CONTEXT: &str = "GitViewer";

actions!(
    git_viewer,
    [
        OpenCommandPalette,
        Back,
        Next,
        Prev,
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
        KeyBinding::new("cmd-s", SaveConflict, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-s", SaveConflict, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-s", SaveConflictAndAdd, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-s", SaveConflictAndAdd, Some(CONTEXT)),
    ]);
}

#[derive(Clone, Debug)]
struct FileEntry {
    path: String,
    status: String,
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

#[derive(Clone, Copy, Debug)]
struct DiffViewOptions {
    ignore_whitespace: bool,
    context_lines: usize,
}

const MAX_CONTEXT_LINES: usize = 20;
const DIFF_REBUILD_DEBOUNCE_MS: u64 = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplitLayout {
    Aligned,
    TwoPane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiffViewMode {
    Split,
    Inline,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CompareTarget {
    HeadToWorktree,
    IndexToWorktree,
    HeadToIndex,
    Refs { left: String, right: String },
}

#[derive(Clone, Debug)]
struct CommitEntry {
    hash: String,
    short_hash: String,
    subject: String,
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
enum CommandPaletteCommand {
    Back,
    Next,
    Prev,
    ToggleViewMode,
    ToggleSplitLayout,
    ToggleWhitespace,
    ExpandAll,
    OpenFileHistory,
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
        command: CommandPaletteCommand::ExpandAll,
        title: "展开全部折叠",
        keywords: "expand all folds 展开 折叠",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::OpenFileHistory,
        title: "打开文件历史对比",
        keywords: "history log commit 历史 对比",
    },
    CommandPaletteItem {
        command: CommandPaletteCommand::ApplyEditor,
        title: "应用合并结果编辑",
        keywords: "apply editor 应用 合并 结果",
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
    status: Option<String>,
    compare_target: CompareTarget,
    old_text: String,
    new_text: String,
    old_lines: Vec<String>,
    new_lines: Vec<String>,
    diff_model: diffview::DiffModel,
    rows: Vec<DisplayRow>,
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
    files: Vec<FileEntry>,
    loading: bool,
    git_available: bool,
    focus_handle: FocusHandle,
    screen: AppScreen,
    diff_view: Option<DiffViewState>,
    conflict_view: Option<ConflictViewState>,
    diff_options: DiffViewOptions,
    compare_left_input: Entity<InputState>,
    compare_right_input: Entity<InputState>,
    file_history_overlay: Option<FileHistoryOverlayState>,
    command_palette_overlay: Option<CommandPaletteOverlayState>,
    diff_content_revision: u64,
    diff_rebuild_seq: u64,
    split_layout: SplitLayout,
    view_mode: DiffViewMode,
    status_filter: StatusFilter,
}

impl GitViewerApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let this = cx.entity();
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let repo_root = detect_repo_root(&cwd);
        let repo_root_for_task = repo_root.clone();
        let git_available = Command::new("git").arg("--version").output().is_ok();
        let focus_handle = cx.focus_handle().tab_stop(true);

        if git_available {
            cx.spawn_in(window, async move |_, window| {
                let entries = window
                    .background_executor()
                    .spawn(async move {
                        fetch_git_status(&repo_root_for_task)
                            .map_err(|err| {
                                eprintln!("git status failed: {err:?}");
                                err
                            })
                            .unwrap_or_default()
                    })
                    .await;

                let _ = window.update(|_window, cx| {
                    this.update(cx, |this, _cx| {
                        this.loading = false;
                        this.files = entries;
                    })
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

        Self {
            repo_root,
            files: Vec::new(),
            loading: git_available,
            git_available,
            focus_handle,
            screen: AppScreen::StatusList,
            diff_view: None,
            conflict_view: None,
            diff_options: DiffViewOptions {
                ignore_whitespace: false,
                context_lines: 3,
            },
            compare_left_input,
            compare_right_input,
            file_history_overlay: None,
            command_palette_overlay: None,
            diff_content_revision: 0,
            diff_rebuild_seq: 0,
            split_layout: SplitLayout::TwoPane,
            view_mode: DiffViewMode::Split,
            status_filter: StatusFilter::All,
        }
    }

    fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
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

            let (old_text, new_text, model, old_lines, new_lines) = window
                .background_executor()
                .spawn(async move {
                    let (model, old_lines, new_lines) = build_diff_model(
                        &old_text,
                        &new_text,
                        diff_options.ignore_whitespace,
                        diff_options.context_lines,
                    );
                    (old_text, new_text, model, old_lines, new_lines)
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
                        this.diff_view = Some(DiffViewState::from_precomputed(
                            format!("Large Diff Demo ({line_count} lines)").into(),
                            None,
                            None,
                            CompareTarget::HeadToWorktree,
                            old_text,
                            new_text,
                            view_mode,
                            model,
                            old_lines,
                            new_lines,
                        ));
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
                .code_editor("text")
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
        self.screen = AppScreen::DiffView;
    }

    fn open_conflict_view(
        &mut self,
        title: SharedString,
        path: Option<String>,
        text: String,
        result_input: Entity<InputState>,
    ) {
        self.diff_view = None;
        self.conflict_view = Some(ConflictViewState::new(title, path, text, result_input));
        self.screen = AppScreen::ConflictView;
    }

    fn close_diff_view(&mut self) {
        self.screen = AppScreen::StatusList;
    }

    fn close_conflict_view(&mut self) {
        self.screen = AppScreen::StatusList;
    }

    fn open_file_diff(
        &mut self,
        path: String,
        status: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = default_compare_target(&status);
        self.open_file_diff_with_target(path, status, target, window, cx);
    }

    fn open_file_diff_with_target(
        &mut self,
        path: String,
        status: String,
        target: CompareTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let path_for_task = path.clone();
        let status_for_task = status.clone();

        window.push_notification(
            Notification::new().message(format!(
                "正在加载 diff：{status} {path}（{}）",
                compare_target_label(&target)
            )),
            cx,
        );

        cx.spawn_in(window, async move |_, window| {
            let path_for_task_bg = path_for_task.clone();
            let status_for_task_bg = status_for_task.clone();
            let target_for_io = target.clone();
            let (old_text, old_err, new_text, new_err) = window
                .background_executor()
                .spawn(async move {
                    let (old_text, old_err) = match target_for_io.clone() {
                        CompareTarget::HeadToWorktree | CompareTarget::HeadToIndex => {
                            match read_head_file(&repo_root, &path_for_task_bg, &status_for_task_bg)
                            {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::IndexToWorktree => {
                            match read_index_file(
                                &repo_root,
                                &path_for_task_bg,
                                &status_for_task_bg,
                            ) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::Refs { left, .. } => {
                            match read_specified_file(&repo_root, &path_for_task_bg, &left) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                    };

                    let (new_text, new_err) = match target_for_io {
                        CompareTarget::HeadToWorktree | CompareTarget::IndexToWorktree => {
                            match read_working_file(&repo_root, &path_for_task_bg) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::HeadToIndex => {
                            match read_index_file(
                                &repo_root,
                                &path_for_task_bg,
                                &status_for_task_bg,
                            ) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                        CompareTarget::Refs { right, .. } => {
                            match read_specified_file(&repo_root, &path_for_task_bg, &right) {
                                Ok(text) => (text, None),
                                Err(err) => (String::new(), Some(err.to_string())),
                            }
                        }
                    };

                    (old_text, old_err, new_text, new_err)
                })
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

            let (old_text, new_text, model, old_lines, new_lines) = window
                .background_executor()
                .spawn(async move {
                    let (model, old_lines, new_lines) = build_diff_model(
                        &old_text,
                        &new_text,
                        diff_options.ignore_whitespace,
                        diff_options.context_lines,
                    );
                    (old_text, new_text, model, old_lines, new_lines)
                })
                .await;

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

                    this.update(cx, |this, _cx| {
                        this.diff_content_revision = this.diff_content_revision.wrapping_add(1);
                        this.conflict_view = None;
                        this.diff_view = Some(DiffViewState::from_precomputed(
                            format!("{status_for_task} {path_for_task}").into(),
                            Some(path_for_task.clone()),
                            Some(status_for_task.clone()),
                            target,
                            old_text,
                            new_text,
                            view_mode,
                            model,
                            old_lines,
                            new_lines,
                        ));
                        this.screen = AppScreen::DiffView;
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

        window.push_notification(
            Notification::new().message(format!(
                "正在加载 diff：{path}（{}）",
                compare_target_label(&compare_target)
            )),
            cx,
        );

        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let path_for_task = path.clone();
        let status_for_task = status.clone();

        cx.spawn_in(window, async move |_, window| {
            let path_for_task_bg = path_for_task.clone();
            let left_ref_for_io = left_ref.clone();
            let right_ref_for_io = right_ref.clone();
            let (old_text, old_err, new_text, new_err) = window
                .background_executor()
                .spawn(async move {
                    let (old_text, old_err) = match read_specified_file(
                        &repo_root,
                        &path_for_task_bg,
                        &left_ref_for_io,
                    ) {
                        Ok(text) => (text, None),
                        Err(err) => (String::new(), Some(err.to_string())),
                    };

                    let (new_text, new_err) =
                        match read_specified_file(&repo_root, &path_for_task_bg, &right_ref_for_io)
                        {
                            Ok(text) => (text, None),
                            Err(err) => (String::new(), Some(err.to_string())),
                        };

                    (old_text, old_err, new_text, new_err)
                })
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

            let (old_text, new_text, model, old_lines, new_lines) = window
                .background_executor()
                .spawn(async move {
                    let (model, old_lines, new_lines) = build_diff_model(
                        &old_text,
                        &new_text,
                        diff_options.ignore_whitespace,
                        diff_options.context_lines,
                    );
                    (old_text, new_text, model, old_lines, new_lines)
                })
                .await;

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

                    this.update(cx, |this, _cx| {
                        this.diff_content_revision = this.diff_content_revision.wrapping_add(1);
                        this.conflict_view = None;
                        this.diff_view = Some(DiffViewState::from_precomputed(
                            title,
                            Some(path_for_task.clone()),
                            status_for_task.clone(),
                            compare_target.clone(),
                            old_text,
                            new_text,
                            view_mode,
                            model,
                            old_lines,
                            new_lines,
                        ));
                        this.screen = AppScreen::DiffView;
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
            CommandPaletteCommand::ToggleViewMode => matches!(self.screen, AppScreen::DiffView),
            CommandPaletteCommand::ToggleSplitLayout => {
                matches!(self.screen, AppScreen::DiffView) && self.view_mode == DiffViewMode::Split
            }
            CommandPaletteCommand::ToggleWhitespace => matches!(self.screen, AppScreen::DiffView),
            CommandPaletteCommand::ExpandAll => {
                self.screen == AppScreen::DiffView
                    && self.diff_view.as_ref().is_some_and(|view| {
                        view.rows
                            .iter()
                            .any(|row| matches!(row, DisplayRow::Fold { .. }))
                    })
            }
            CommandPaletteCommand::OpenFileHistory => {
                self.git_available
                    && self.screen == AppScreen::DiffView
                    && self
                        .diff_view
                        .as_ref()
                        .is_some_and(|view| view.path.is_some())
            }
            CommandPaletteCommand::ApplyEditor => matches!(self.screen, AppScreen::ConflictView),
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
            CommandPaletteCommand::ExpandAll => {
                if matches!(self.screen, AppScreen::DiffView) {
                    self.expand_all_folds();
                }
            }
            CommandPaletteCommand::OpenFileHistory => {
                if matches!(self.screen, AppScreen::DiffView) {
                    self.open_file_history_overlay(window, cx);
                }
            }
            CommandPaletteCommand::ApplyEditor => {
                if matches!(self.screen, AppScreen::ConflictView) {
                    self.apply_conflict_editor(window, cx);
                }
            }
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
        self.open_file_diff_with_target(path, status, target, window, cx);
    }

    fn open_file(
        &mut self,
        path: String,
        status: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if status.contains('U') {
            self.open_conflict_file(path, status, window, cx);
        } else {
            self.open_file_diff(path, status, window, cx);
        }
    }

    fn open_conflict_file(
        &mut self,
        path: String,
        status: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let this = cx.entity();
        let repo_root = self.repo_root.clone();
        let path_for_task = path.clone();
        let status_for_task = status.clone();

        window.push_notification(
            Notification::new().message(format!("正在加载冲突：{status} {path}")),
            cx,
        );

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
                    let result_input = cx.new(move |cx| {
                        InputState::new(window, cx)
                            .code_editor("text")
                            .default_value(initial_text)
                    });
                    this.update(cx, |this, _cx| {
                        this.open_conflict_view(
                            format!("{status_for_task} {path_for_task}").into(),
                            Some(path_for_task),
                            text,
                            result_input,
                        );
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

            let (old_text, new_text, model, old_lines, new_lines) = window
                .background_executor()
                .spawn(async move {
                    let (model, old_lines, new_lines) = build_diff_model(
                        &old_text,
                        &new_text,
                        diff_options.ignore_whitespace,
                        diff_options.context_lines,
                    );
                    (old_text, new_text, model, old_lines, new_lines)
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

                        let mut next = DiffViewState::from_precomputed(
                            title,
                            path,
                            status,
                            compare_target,
                            old_text,
                            new_text,
                            view_mode,
                            model,
                            old_lines,
                            new_lines,
                        );
                        next.scroll_handle = scroll_handle;
                        next.scroll_state = scroll_state;
                        next.current_hunk =
                            current_hunk.min(next.hunk_rows.len().saturating_sub(1));
                        this.diff_view = Some(next);
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
                        this.update(cx, |this, _cx| {
                            this.files = entries;
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
            let compare_target_for_io = compare_target.clone();
            let (
                add_ok,
                add_err,
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
                    let add_result = run_git(repo_root.as_path(), ["add", "--", &path_for_task_bg]);
                    let add_ok = add_result.is_ok();
                    let add_err = add_result.err().map(|err| err.to_string());
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
                        add_ok,
                        add_err,
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
                        if let Some(entries) = entries {
                            this.files = entries;
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
            let compare_target_for_io = compare_target.clone();
            let (
                reset_ok,
                reset_err,
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
                    let reset_result = run_git(
                        repo_root.as_path(),
                        ["reset", "HEAD", "--", &path_for_task_bg],
                    );
                    let reset_ok = reset_result.is_ok();
                    let reset_err = reset_result.err().map(|err| err.to_string());
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
                        reset_ok,
                        reset_err,
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

    fn stage_current_hunk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_current_hunk(HunkApplyAction::Stage, window, cx);
    }

    fn unstage_current_hunk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_current_hunk(HunkApplyAction::Unstage, window, cx);
    }

    fn revert_current_hunk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_current_hunk(HunkApplyAction::Revert, window, cx);
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
                                    compare_target,
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
                    let path = entry.path.clone();
                    let status = entry.status.clone();
                    Button::new(("file", index))
                        .label(format!("{status} {path}"))
                        .w_full()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            println!("[git-viewer] 打开文件: {status} {path}");
                            this.open_file(path.clone(), status.clone(), window, cx);
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

    fn render_diff_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let Some(diff_view) = self.diff_view.as_mut() else {
            return div().p(px(12.)).child("No diff view");
        };

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
        let file_status = diff_view.status.as_deref().unwrap_or_default();
        let has_file_path = diff_view.path.is_some();
        let git_available = self.git_available;
        let can_stage = git_available
            && has_file_path
            && (is_untracked_status(file_status)
                || status_xy(file_status).is_some_and(|(_, y)| y != ' ' && y != '?' && y != '!'));
        let can_unstage = git_available
            && has_file_path
            && status_xy(file_status).is_some_and(|(x, _)| x != ' ' && x != '?' && x != '!');
        let has_hunks = hunk_count > 0;
        let can_stage_hunk = git_available
            && has_file_path
            && has_hunks
            && !is_untracked_status(file_status)
            && compare_target == CompareTarget::IndexToWorktree;
        let can_revert_hunk = can_stage_hunk;
        let can_unstage_hunk = git_available
            && has_file_path
            && has_hunks
            && compare_target == CompareTarget::HeadToIndex;
        let rows_len = diff_view.rows.len();
        let scroll_handle = diff_view.scroll_handle.clone();
        let scroll_state = diff_view.scroll_state.clone();
        let row_height = window.line_height() + px(4.);
        let item_sizes = diff_view.item_sizes(row_height);

        let app = cx.entity();
        let compare_left_input = self.compare_left_input.clone();
        let compare_right_input = self.compare_right_input.clone();
        let compare_label: SharedString =
            format!("对比: {}", compare_target_label(&compare_target)).into();
        let compare_control: AnyElement = if git_available && has_file_path {
            let app_for_menu = app.clone();
            let active_target = compare_target.clone();
            Popover::new("diff-compare-menu")
                .appearance(false)
                .trigger(
                    Button::new("diff-compare-trigger")
                        .label(compare_label.clone())
                        .ghost()
                        .tooltip("选择对比目标")
                        .on_click(|_, _, _| {}),
                )
                .content(move |_, _window, cx| {
                    let theme = cx.theme();
                    let popover = cx.entity();

                    let app_for_items = app_for_menu.clone();
                    let popover_for_items = popover.clone();
                    let active_target = active_target.clone();
                    let make_item = |id: &'static str,
                                     label: SharedString,
                                     target: CompareTarget| {
                        let app = app_for_items.clone();
                        let popover = popover_for_items.clone();
                        let is_active = active_target == target;

                        div()
                            .id(id)
                            .flex()
                            .items_center()
                            .h(px(32.))
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

                    div()
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
                        ))
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
                                .child(Input::new(&compare_left_input).w_full())
                                .child(Input::new(&compare_right_input).w_full())
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .justify_end()
                                        .gap(px(6.))
                                        .child(
                                            Button::new("diff-compare-swap")
                                                .label("交换")
                                                .ghost()
                                                .on_click({
                                                    let app = app_for_menu.clone();
                                                    move |_, window, cx| {
                                                        app.update(cx, |this, cx| {
                                                            this.swap_compare_refs_inputs(
                                                                window, cx,
                                                            );
                                                        });
                                                    }
                                                }),
                                        )
                                        .child(
                                            Button::new("diff-compare-apply")
                                                .label("应用")
                                                .primary()
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
                })
                .into_any_element()
        } else {
            Button::new("diff-compare-disabled")
                .label(compare_label)
                .ghost()
                .disabled(true)
                .into_any_element()
        };

        let can_expand_all = diff_view
            .rows
            .iter()
            .any(|row| matches!(row, DisplayRow::Fold { .. }));

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

        let stage_hunk_label: SharedString = if can_stage_hunk {
            "Stage 当前 hunk".into()
        } else if !git_available {
            "Stage 当前 hunk（git 不可用）".into()
        } else if !has_file_path {
            "Stage 当前 hunk（demo 不支持）".into()
        } else if !has_hunks {
            "Stage 当前 hunk（无 hunk）".into()
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
        } else if compare_target != CompareTarget::IndexToWorktree {
            "Revert 当前 hunk（切到 暂存↔工作区）".into()
        } else {
            "Revert 当前 hunk（不可用）".into()
        };

        let shortcuts_message: SharedString = concat!(
            "快捷键：\n",
            "Esc 返回\n",
            "Alt+N / Alt+P 下一/上一 hunk\n",
            "Alt+V 切换 Split/Inline\n",
            "Alt+L 切换对齐/分栏\n",
            "Alt+W 忽略空白\n",
            "Alt+E 展开全部\n",
        )
        .into();

        let more_menu = {
            let app_for_menu = app.clone();
            Popover::new("diff-more-menu")
                .appearance(false)
                .trigger(
                    Button::new("diff-more-trigger")
                        .label("更多")
                        .ghost()
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

                    let can_toggle_split = !inline_mode;

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
                                .gap(px(8.))
                                .px(px(6.))
                                .child(div().text_sm().child(format!("上下文: {context_lines}")))
                                .child(
                                    Button::new("diff-more-context-dec")
                                        .label("－")
                                        .ghost()
                                        .disabled(context_lines == 0)
                                        .on_click({
                                            let app = app_for_menu.clone();
                                            move |_, window, cx| {
                                                app.update(cx, |this, cx| {
                                                    let next = this
                                                        .diff_options
                                                        .context_lines
                                                        .saturating_sub(1);
                                                    this.set_context_lines(next, window, cx);
                                                    cx.notify();
                                                });
                                            }
                                        }),
                                )
                                .child(
                                    Button::new("diff-more-context-inc")
                                        .label("＋")
                                        .ghost()
                                        .disabled(context_lines >= MAX_CONTEXT_LINES)
                                        .on_click({
                                            let app = app_for_menu.clone();
                                            move |_, window, cx| {
                                                app.update(cx, |this, cx| {
                                                    let next = this.diff_options.context_lines + 1;
                                                    this.set_context_lines(next, window, cx);
                                                    cx.notify();
                                                });
                                            }
                                        }),
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
                                .px(px(4.))
                                .py(px(2.))
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
                    .child(
                        Button::new("back")
                            .label("返回")
                            .ghost()
                            .tooltip_with_action("返回", &Back, Some(CONTEXT))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.close_diff_view();
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1().min_w(px(0.)).truncate().child(title)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
                    .flex_wrap()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.))
                            .child({
                                let button = Button::new("mode-split").label("Split");
                                if view_mode == DiffViewMode::Split {
                                    button.primary()
                                } else {
                                    button.ghost().on_click(cx.listener(|this, _, _window, cx| {
                                        this.set_view_mode(DiffViewMode::Split);
                                        cx.notify();
                                    }))
                                }
                            })
                            .child({
                                let button = Button::new("mode-inline").label("Inline");
                                if view_mode == DiffViewMode::Inline {
                                    button.primary()
                                } else {
                                    button.ghost().on_click(cx.listener(|this, _, _window, cx| {
                                        this.set_view_mode(DiffViewMode::Inline);
                                        cx.notify();
                                    }))
                                }
                            }),
                    )
                    .child(compare_control)
                    .child(
                        Button::new("prev-hunk")
                            .label("上一 hunk")
                            .ghost()
                            .tooltip_with_action("上一 hunk", &Prev, Some(CONTEXT))
                            .disabled(!can_prev_hunk)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.jump_hunk(-1);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("next-hunk")
                            .label("下一 hunk")
                            .ghost()
                            .tooltip_with_action("下一 hunk", &Next, Some(CONTEXT))
                            .disabled(!can_next_hunk)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.jump_hunk(1);
                                cx.notify();
                            })),
                    )
                    .child(more_menu),
            );

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
        let status_left = match view_mode {
            DiffViewMode::Split => div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .child("视图: Split")
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
                .child("视图: Inline")
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

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(toolbar)
            .child(viewport)
            .child(status_bar)
    }

    fn render_conflict_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
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
        let shortcuts_message: SharedString = format!(
            "快捷键：\n\
             Esc 返回\n\
             Alt+N / Alt+P 下一/上一冲突\n\
             Alt+L 切换对齐/分栏\n\
             Alt+A 应用编辑\n\
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
                        .label("更多")
                        .ghost()
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
                    .child(
                        Button::new("conflict-back")
                            .label("返回")
                            .ghost()
                            .tooltip_with_action("返回", &Back, Some(CONTEXT))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.close_conflict_view();
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1().min_w(px(0.)).truncate().child(title)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
                    .flex_wrap()
                    .child(div().child(format!("冲突: {conflict_position}")))
                    .child(
                        Button::new("conflict-prev")
                            .label("上一冲突")
                            .ghost()
                            .tooltip_with_action("上一冲突", &Prev, Some(CONTEXT))
                            .disabled(!can_prev)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.jump_conflict(-1);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("conflict-next")
                            .label("下一冲突")
                            .ghost()
                            .tooltip_with_action("下一冲突", &Next, Some(CONTEXT))
                            .disabled(!can_next)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.jump_conflict(1);
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

        let result_panel = show_result_editor.then(|| {
            div()
                .flex()
                .flex_col()
                .flex_none()
                .h(px(220.))
                .border_t_1()
                .border_color(cx.theme().border.alpha(0.6))
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
                )
        });

        let mut root = div()
            .flex()
            .flex_col()
            .size_full()
            .child(toolbar)
            .child(viewport);

        if let Some(panel) = result_panel {
            root = root.child(panel);
        }

        root.child(status_bar)
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

    fn render_demo_row(
        &mut self,
        index: usize,
        height: Pixels,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = cx.theme();
        let Some(row) = self
            .diff_view
            .as_ref()
            .and_then(|diff_view| diff_view.rows.get(index))
            .cloned()
        else {
            return div();
        };

        match row {
            DisplayRow::HunkHeader { text } => div()
                .h(height)
                .px(px(12.))
                .flex()
                .items_center()
                .bg(theme.muted.alpha(0.35))
                .font_family(theme.mono_font_family.clone())
                .text_sm()
                .child(text),
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
            }
            DisplayRow::Code {
                kind,
                old_line,
                new_line,
                old_segments,
                new_segments,
            } => {
                let border = theme.border;
                let mono = theme.mono_font_family.clone();

                div()
                    .h(height)
                    .flex()
                    .flex_row()
                    .items_center()
                    .border_b_1()
                    .border_color(border.alpha(0.35))
                    .child(
                        render_side(
                            Side::Old,
                            kind,
                            old_line,
                            &old_segments,
                            mono.clone(),
                            theme,
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _window, cx| {
                                if let Some(diff_view) = this.diff_view.as_ref() {
                                    diff_view
                                        .scroll_handle
                                        .scroll_to_item(index, ScrollStrategy::Top);
                                }
                                cx.notify();
                            }),
                        ),
                    )
                    .child(div().w(px(1.)).h_full().bg(border.alpha(0.6)))
                    .child(
                        render_side(Side::New, kind, new_line, &new_segments, mono, theme)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _window, cx| {
                                    if let Some(diff_view) = this.diff_view.as_ref() {
                                        diff_view
                                            .scroll_handle
                                            .scroll_to_item(index, ScrollStrategy::Top);
                                    }
                                    cx.notify();
                                }),
                            ),
                    )
            }
        }
    }

    fn render_inline_row(
        &mut self,
        index: usize,
        height: Pixels,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = cx.theme();
        let Some(row) = self
            .diff_view
            .as_ref()
            .and_then(|diff_view| diff_view.rows.get(index))
            .cloned()
        else {
            return div();
        };

        match row {
            DisplayRow::HunkHeader { text } => div()
                .h(height)
                .px(px(12.))
                .flex()
                .items_center()
                .bg(theme.muted.alpha(0.35))
                .font_family(theme.mono_font_family.clone())
                .text_sm()
                .child(text),
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
            }
            DisplayRow::Code {
                kind,
                old_line,
                new_line,
                old_segments,
                new_segments,
            } => {
                let border = theme.border;
                let mono = theme.mono_font_family.clone();
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

                div()
                    .h(height)
                    .flex()
                    .flex_row()
                    .items_center()
                    .border_b_1()
                    .border_color(border.alpha(0.35))
                    .bg(bg)
                    .child(div().w(px(3.)).h_full().bg(marker_color))
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
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, cx| {
                            if let Some(diff_view) = this.diff_view.as_ref() {
                                diff_view
                                    .scroll_handle
                                    .scroll_to_item(index, ScrollStrategy::Top);
                            }
                            cx.notify();
                        }),
                    )
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
    ) -> Div {
        let theme = cx.theme();
        let Some(row) = self
            .diff_view
            .as_ref()
            .and_then(|diff_view| diff_view.rows.get(index))
            .cloned()
        else {
            return div();
        };

        match row {
            DisplayRow::HunkHeader { text } => div()
                .h(height)
                .px(px(12.))
                .flex()
                .items_center()
                .bg(theme.muted.alpha(0.35))
                .font_family(theme.mono_font_family.clone())
                .text_sm()
                .child(text),
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
                ),
            DisplayRow::Code {
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

                div()
                    .h(height)
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(border.alpha(0.35))
                    .child(
                        render_side(side, kind, line_no, &segments, mono, theme).on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _window, cx| {
                                if let Some(diff_view) = this.diff_view.as_ref() {
                                    diff_view
                                        .scroll_handle
                                        .scroll_to_item(index, ScrollStrategy::Top);
                                }
                                cx.notify();
                            }),
                        ),
                    )
            }
        }
    }
}

impl Render for GitViewerApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.screen {
            AppScreen::StatusList => self.render_status_list(window, cx).into_any_element(),
            AppScreen::DiffView => self.render_diff_view(window, cx).into_any_element(),
            AppScreen::ConflictView => self.render_conflict_view(window, cx).into_any_element(),
        };

        let file_history_overlay = self.render_file_history_overlay(window, cx);
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
                .on_action(cx.listener(|this, _: &ExpandAll, _window, cx| {
                    if matches!(this.screen, AppScreen::DiffView) {
                        this.expand_all_folds();
                        cx.notify();
                    }
                }))
                .on_action(cx.listener(|this, _: &ApplyEditor, window, cx| {
                    if matches!(this.screen, AppScreen::ConflictView) {
                        this.apply_conflict_editor(window, cx);
                    }
                }))
                .on_action(cx.listener(|this, _: &SaveConflict, window, cx| {
                    if matches!(this.screen, AppScreen::ConflictView) {
                        this.save_conflict_to_working_tree(false, window, cx);
                        cx.notify();
                    }
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

        if let Some(overlay) = command_palette_overlay {
            root = root.child(overlay);
        }

        root
    }
}

#[derive(Clone, Copy, Debug)]
enum Side {
    Old,
    New,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConflictPane {
    Ours,
    Base,
    Theirs,
}

fn render_side(
    side: Side,
    kind: diffview::DiffRowKind,
    line_no: Option<usize>,
    segments: &[diffview::DiffSegment],
    font_family: SharedString,
    theme: &gpui_component::Theme,
) -> Div {
    let gutter_width = px(56.);
    let marker_color = match (side, kind) {
        (Side::Old, diffview::DiffRowKind::Removed) => theme.red,
        (Side::New, diffview::DiffRowKind::Added) => theme.green,
        (_, diffview::DiffRowKind::Modified) => theme.yellow,
        _ => theme.transparent,
    };
    let (bg, gutter_bg) = match (side, kind) {
        (Side::Old, diffview::DiffRowKind::Removed) => {
            (theme.red.alpha(0.12), theme.red.alpha(0.08))
        }
        (Side::New, diffview::DiffRowKind::Added) => {
            (theme.green.alpha(0.12), theme.green.alpha(0.08))
        }
        (_, diffview::DiffRowKind::Modified) => (theme.muted.alpha(0.25), theme.muted.alpha(0.2)),
        _ => (theme.transparent, theme.transparent),
    };

    let line_no = line_no.map(|n| n.to_string()).unwrap_or_default();

    div()
        .flex()
        .flex_row()
        .items_center()
        .flex_1()
        .min_w(px(0.))
        .bg(bg)
        .child(div().w(px(3.)).h_full().bg(marker_color))
        .child(
            div()
                .w(gutter_width)
                .h_full()
                .px(px(8.))
                .flex()
                .items_center()
                .justify_end()
                .bg(gutter_bg)
                .text_xs()
                .text_color(theme.muted_foreground)
                .font_family(font_family.clone())
                .child(line_no),
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
                .font_family(font_family)
                .text_sm()
                .children(render_segments(segments, theme)),
        )
}

fn render_segments(segments: &[diffview::DiffSegment], theme: &gpui_component::Theme) -> Vec<Div> {
    segments
        .iter()
        .map(|seg| {
            let (bg, fg) = match seg.kind {
                diffview::DiffSegmentKind::Unchanged => (theme.transparent, theme.foreground),
                diffview::DiffSegmentKind::Added => (theme.green.alpha(0.28), theme.foreground),
                diffview::DiffSegmentKind::Removed => (theme.red.alpha(0.28), theme.foreground),
            };

            div()
                .flex_none()
                .bg(bg)
                .text_color(fg)
                .child(preserve_spaces(&seg.text))
        })
        .collect()
}

fn preserve_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            ' ' => out.push('\u{00A0}'),
            '\t' => out.push_str("\u{00A0}\u{00A0}\u{00A0}\u{00A0}"),
            _ => out.push(ch),
        }
    }
    out
}

impl DiffViewState {
    fn new(
        title: SharedString,
        path: Option<String>,
        status: Option<String>,
        compare_target: CompareTarget,
        old_text: String,
        new_text: String,
        options: DiffViewOptions,
        view_mode: DiffViewMode,
    ) -> Self {
        let (diff_model, old_lines, new_lines) = build_diff_model(
            &old_text,
            &new_text,
            options.ignore_whitespace,
            options.context_lines,
        );
        let rows = build_display_rows_from_model(&diff_model, &old_lines, &new_lines, view_mode);
        let mut this = Self {
            title,
            path,
            status,
            compare_target,
            old_text,
            new_text,
            old_lines,
            new_lines,
            diff_model,
            rows,
            hunk_rows: Vec::new(),
            current_hunk: 0,
            scroll_handle: VirtualListScrollHandle::new(),
            scroll_state: ScrollbarState::default(),
            list_item_sizes: Rc::new(Vec::new()),
            list_item_height: px(0.),
        };
        this.recalc_hunk_rows();
        this
    }

    fn from_precomputed(
        title: SharedString,
        path: Option<String>,
        status: Option<String>,
        compare_target: CompareTarget,
        old_text: String,
        new_text: String,
        view_mode: DiffViewMode,
        diff_model: diffview::DiffModel,
        old_lines: Vec<String>,
        new_lines: Vec<String>,
    ) -> Self {
        let rows = build_display_rows_from_model(&diff_model, &old_lines, &new_lines, view_mode);
        let mut this = Self {
            title,
            path,
            status,
            compare_target,
            old_text,
            new_text,
            old_lines,
            new_lines,
            diff_model,
            rows,
            hunk_rows: Vec::new(),
            current_hunk: 0,
            scroll_handle: VirtualListScrollHandle::new(),
            scroll_state: ScrollbarState::default(),
            list_item_sizes: Rc::new(Vec::new()),
            list_item_height: px(0.),
        };
        this.recalc_hunk_rows();
        this
    }

    fn item_sizes(&mut self, row_height: Pixels) -> Rc<Vec<Size<Pixels>>> {
        let count = self.rows.len();
        if self.list_item_height != row_height || self.list_item_sizes.len() != count {
            self.list_item_height = row_height;
            self.list_item_sizes = Rc::new(vec![size(px(0.), row_height); count]);
        }
        self.list_item_sizes.clone()
    }

    fn rebuild_rows(&mut self, view_mode: DiffViewMode) {
        self.rows = build_display_rows_from_model(
            &self.diff_model,
            &self.old_lines,
            &self.new_lines,
            view_mode,
        );
        self.recalc_hunk_rows();
    }

    fn recalc_hunk_rows(&mut self) {
        self.hunk_rows = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                matches!(row, DisplayRow::HunkHeader { .. }).then_some(index)
            })
            .collect();

        if self.current_hunk >= self.hunk_rows.len() {
            self.current_hunk = self.hunk_rows.len().saturating_sub(1);
        }
    }
}

impl ConflictViewState {
    fn new(
        title: SharedString,
        path: Option<String>,
        text: String,
        result_input: Entity<InputState>,
    ) -> Self {
        let conflicts = diffview::parse_conflicts(&text);
        let rows = build_conflict_rows(&text, &conflicts);
        let mut this = Self {
            title,
            path,
            text,
            result_input,
            show_result_editor: true,
            conflicts,
            rows,
            conflict_rows: Vec::new(),
            current_conflict: 0,
            scroll_handle: VirtualListScrollHandle::new(),
            scroll_state: ScrollbarState::default(),
            list_item_sizes: Rc::new(Vec::new()),
            list_item_height: px(0.),
        };
        this.recalc_conflict_rows();
        this
    }

    fn item_sizes(&mut self, row_height: Pixels) -> Rc<Vec<Size<Pixels>>> {
        let count = self.rows.len();
        if self.list_item_height != row_height || self.list_item_sizes.len() != count {
            self.list_item_height = row_height;
            self.list_item_sizes = Rc::new(vec![size(px(0.), row_height); count]);
        }
        self.list_item_sizes.clone()
    }

    fn rebuild(&mut self) {
        self.conflicts = diffview::parse_conflicts(&self.text);
        self.rows = build_conflict_rows(&self.text, &self.conflicts);
        self.recalc_conflict_rows();
    }

    fn recalc_conflict_rows(&mut self) {
        self.conflict_rows = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                matches!(row, ConflictRow::BlockHeader { .. }).then_some(index)
            })
            .collect();

        if self.current_conflict >= self.conflict_rows.len() {
            self.current_conflict = self.conflict_rows.len().saturating_sub(1);
        }
    }
}

fn demo_texts() -> (String, String) {
    let old = r#"fn main() {
    let x = 1;
    println!("x = {}", x);
    println!("keep 00");
    println!("keep 01");
    println!("keep 02");
    println!("keep 03");
    println!("keep 04");
    println!("keep 05");
    println!("keep 06");
    println!("keep 07");
    println!("keep 08");
    println!("keep 09");
    println!("keep 10");
    println!("keep 11");
    println!("keep 12");
    println!("keep 13");
    println!("keep 14");
    println!("keep 15");
    println!("keep 16");
    println!("keep 17");
    println!("keep 18");
    println!("keep 19");
    println!("tail");
}
"#;

    let new = r#"fn main() {
    let   x = 2;
    println!( "x = {}", x);
    println!("keep 00");
    println!("keep 01");
    println!("keep 02");
    println!("keep 03");
    println!("keep 04");
    println!("keep 05");
    println!("keep 06");
    println!("keep 07");
    println!("keep 08");
    println!("keep 09");
    println!("keep 10");
    println!("keep 11");
    println!("keep 12");
    println!("keep 13");
    println!("keep 14");
    println!("keep 15");
    println!("keep 16");
    println!("keep 17");
    println!("keep 18");
    println!("keep 19");
    println!("tail");
    println!("done");
}
"#;

    (old.to_string(), new.to_string())
}

fn large_demo_texts(line_count: usize) -> (String, String) {
    let mut old = String::new();
    let mut new = String::new();
    old.reserve(line_count.saturating_mul(32));
    new.reserve(line_count.saturating_mul(34));

    for i in 0..line_count {
        let base = format!("{i:05} let value_{i} = {i};\n");
        old.push_str(&base);

        if i > 0 && i % 251 == 0 {
            continue;
        }

        if i % 199 == 0 {
            new.push_str(&format!("{i:05} // inserted comment for {i}\n"));
        }

        if i % 97 == 0 {
            new.push_str(&format!("{i:05} let value_{i} = {i} + 1;\n"));
        } else if i % 123 == 0 {
            new.push_str(&format!("{i:05} let  value_{i}  =  {i};\n"));
        } else {
            new.push_str(&base);
        }
    }

    (old, new)
}

fn conflict_demo_text() -> String {
    r#"fn main() {
    println!("before");
<<<<<<< HEAD
    println!("ours 1");
=======
    println!("theirs 1");
>>>>>>> feature

    println!("between");
<<<<<<< ours
    println!("ours 2");
||||||| base
    println!("base 2");
=======
    println!("theirs 2");
>>>>>>> theirs

    println!("after");
}
"#
    .to_string()
}

fn build_diff_model(
    old_text: &str,
    new_text: &str,
    ignore_whitespace: bool,
    context_lines: usize,
) -> (diffview::DiffModel, Vec<String>, Vec<String>) {
    let old_doc = diffview::Document::from_str(old_text);
    let new_doc = diffview::Document::from_str(new_text);
    let old_lines = old_doc.lines();
    let new_lines = new_doc.lines();
    let model = diffview::diff_documents(
        &old_doc,
        &new_doc,
        diffview::DiffOptions {
            context_lines,
            ignore_whitespace,
        },
    );

    (model, old_lines, new_lines)
}

fn build_display_rows_from_model(
    model: &diffview::DiffModel,
    old_lines: &[String],
    new_lines: &[String],
    view_mode: DiffViewMode,
) -> Vec<DisplayRow> {
    let mut rows = Vec::new();
    let mut old_pos = 0usize;
    let mut new_pos = 0usize;

    for hunk in &model.hunks {
        let gap_old = hunk.old_start.saturating_sub(old_pos);
        let gap_new = hunk.new_start.saturating_sub(new_pos);
        let gap_len = gap_old.min(gap_new);
        if gap_len > 0 {
            rows.push(DisplayRow::Fold {
                old_start: old_pos,
                new_start: new_pos,
                len: gap_len,
            });
        }

        rows.push(DisplayRow::HunkHeader {
            text: format!(
                "@@ -{},{} +{},{} @@",
                hunk.old_start + 1,
                hunk.old_len,
                hunk.new_start + 1,
                hunk.new_len
            )
            .into(),
        });

        for row in &hunk.rows {
            let kind = row.kind();
            let old_line = row.old.as_ref().map(|l| l.line_index + 1);
            let new_line = row.new.as_ref().map(|l| l.line_index + 1);
            let old_segments = row
                .old
                .as_ref()
                .map(|l| l.segments.clone())
                .unwrap_or_default();
            let new_segments = row
                .new
                .as_ref()
                .map(|l| l.segments.clone())
                .unwrap_or_default();

            match view_mode {
                DiffViewMode::Split => rows.push(DisplayRow::Code {
                    kind,
                    old_line,
                    new_line,
                    old_segments,
                    new_segments,
                }),
                DiffViewMode::Inline => match kind {
                    diffview::DiffRowKind::Modified => {
                        rows.push(DisplayRow::Code {
                            kind: diffview::DiffRowKind::Removed,
                            old_line,
                            new_line: None,
                            old_segments,
                            new_segments: Vec::new(),
                        });
                        rows.push(DisplayRow::Code {
                            kind: diffview::DiffRowKind::Added,
                            old_line: None,
                            new_line,
                            old_segments: Vec::new(),
                            new_segments,
                        });
                    }
                    diffview::DiffRowKind::Added => rows.push(DisplayRow::Code {
                        kind,
                        old_line: None,
                        new_line,
                        old_segments: Vec::new(),
                        new_segments,
                    }),
                    diffview::DiffRowKind::Removed => rows.push(DisplayRow::Code {
                        kind,
                        old_line,
                        new_line: None,
                        old_segments,
                        new_segments: Vec::new(),
                    }),
                    diffview::DiffRowKind::Unchanged => rows.push(DisplayRow::Code {
                        kind,
                        old_line,
                        new_line,
                        old_segments,
                        new_segments,
                    }),
                },
            }
        }

        old_pos = hunk.old_start + hunk.old_len;
        new_pos = hunk.new_start + hunk.new_len;
    }

    let tail_old = old_lines.len().saturating_sub(old_pos);
    let tail_new = new_lines.len().saturating_sub(new_pos);
    let tail_len = tail_old.min(tail_new);
    if tail_len > 0 {
        rows.push(DisplayRow::Fold {
            old_start: old_pos,
            new_start: new_pos,
            len: tail_len,
        });
    }

    rows
}

fn build_conflict_rows(text: &str, conflicts: &[diffview::ConflictRegion]) -> Vec<ConflictRow> {
    if conflicts.is_empty() {
        return vec![ConflictRow::EmptyState {
            text: "没有检测到冲突标记（<<<<<<< / ======= / >>>>>>>）".into(),
        }];
    }

    let mut rows = Vec::new();
    for (conflict_index, region) in conflicts.iter().enumerate() {
        rows.push(ConflictRow::BlockHeader {
            conflict_index,
            ours_branch_name: region.ours_branch_name.clone().into(),
            theirs_branch_name: region.theirs_branch_name.clone().into(),
            has_base: region.base.is_some(),
        });

        let ours_text = &text[region.ours.clone()];
        let theirs_text = &text[region.theirs.clone()];
        let base_text = region
            .base
            .as_ref()
            .map(|range| &text[range.clone()])
            .unwrap_or("");

        let ours_lines = diffview::Document::from_str(ours_text).lines();
        let base_lines = diffview::Document::from_str(base_text).lines();
        let theirs_lines = diffview::Document::from_str(theirs_text).lines();
        let max_len = ours_lines
            .len()
            .max(base_lines.len())
            .max(theirs_lines.len())
            .max(1);

        for idx in 0..max_len {
            let ours_line = ours_lines.get(idx).cloned();
            let base_line = base_lines.get(idx).cloned();
            let theirs_line = theirs_lines.get(idx).cloned();

            let kind = match (&ours_line, &theirs_line) {
                (None, None) => diffview::DiffRowKind::Unchanged,
                (None, Some(_)) => diffview::DiffRowKind::Added,
                (Some(_), None) => diffview::DiffRowKind::Removed,
                (Some(a), Some(b)) => {
                    if a == b {
                        diffview::DiffRowKind::Unchanged
                    } else {
                        diffview::DiffRowKind::Modified
                    }
                }
            };

            let ours_segments = ours_line
                .map(|line| {
                    vec![diffview::DiffSegment {
                        kind: diffview::DiffSegmentKind::Unchanged,
                        text: line,
                    }]
                })
                .unwrap_or_default();
            let base_segments = base_line
                .map(|line| {
                    vec![diffview::DiffSegment {
                        kind: diffview::DiffSegmentKind::Unchanged,
                        text: line,
                    }]
                })
                .unwrap_or_default();
            let theirs_segments = theirs_line
                .map(|line| {
                    vec![diffview::DiffSegment {
                        kind: diffview::DiffSegmentKind::Unchanged,
                        text: line,
                    }]
                })
                .unwrap_or_default();

            rows.push(ConflictRow::Code {
                kind,
                ours_segments,
                base_segments,
                theirs_segments,
            });
        }
    }

    rows
}

fn read_working_file(repo_root: &Path, path: &str) -> Result<String> {
    let full_path = repo_root.join(path);
    match std::fs::read(&full_path) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(err).with_context(|| format!("读取工作区文件失败：{path}")),
    }
}

fn read_head_file(repo_root: &Path, path: &str, status: &str) -> Result<String> {
    if status == "??" || status.contains('A') {
        return Ok(String::new());
    }

    let spec = format!("HEAD:{path}");
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["show", &spec])
        .output()
        .with_context(|| format!("执行 git show 失败：{spec}"))?;

    if !output.status.success() {
        return Err(anyhow!(
            "git show 返回非零（{}）：{}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn read_index_file(repo_root: &Path, path: &str, status: &str) -> Result<String> {
    if status == "??" {
        return Ok(String::new());
    }

    if let Some((x, _)) = status_xy(status) {
        if x == 'D' {
            return Ok(String::new());
        }
    }

    let spec = format!(":{path}");
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["show", &spec])
        .output()
        .with_context(|| format!("执行 git show 失败：{spec}"))?;

    if !output.status.success() {
        return Err(anyhow!(
            "git show 返回非零（{}）：{}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn git_show(repo_root: &Path, spec: &str) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["show", spec])
        .output()
        .with_context(|| format!("执行 git show 失败：{spec}"))?;

    if !output.status.success() {
        return Err(anyhow!(
            "git show 返回非零（{}）：{}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn read_specified_file(repo_root: &Path, path: &str, spec: &str) -> Result<String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return read_working_file(repo_root, path);
    }

    if spec.eq_ignore_ascii_case("WORKTREE") {
        return read_working_file(repo_root, path);
    }

    if spec.eq_ignore_ascii_case("INDEX") || spec == ":" {
        return git_show(repo_root, &format!(":{path}"));
    }

    if spec.contains(':') {
        return git_show(repo_root, spec);
    }

    git_show(repo_root, &format!("{spec}:{path}"))
}

fn run_git<I, S>(repo_root: &Path, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .context("执行 git 命令失败")?;

    if output.status.success() {
        return Ok(());
    }

    Err(anyhow!(
        "git 命令返回非零（{}）：{}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn run_git_with_stdin<I, S>(repo_root: &Path, args: I, stdin: &str) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    use std::io::Write as _;
    use std::process::Stdio;

    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("执行 git 命令失败")?;

    if let Some(mut input) = child.stdin.take() {
        input
            .write_all(stdin.as_bytes())
            .context("写入 git stdin 失败")?;
    }

    let output = child.wait_with_output().context("等待 git 进程失败")?;
    if output.status.success() {
        return Ok(());
    }

    Err(anyhow!(
        "git 命令返回非零（{}）：{}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn fetch_file_history(repo_root: &Path, path: &str, limit: usize) -> Result<Vec<CommitEntry>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args([
            "log",
            "-n",
            &limit.to_string(),
            "--format=%H%x1f%h%x1f%s%x1e",
            "--",
            path,
        ])
        .output()
        .context("执行 git log 失败")?;

    if !output.status.success() {
        return Err(anyhow!(
            "git log 返回非零（{}）：{}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let mut commits = Vec::new();
    for record in output.stdout.split(|b| *b == 0x1e) {
        if record.is_empty() {
            continue;
        }

        let mut fields = record.split(|b| *b == 0x1f);
        let Some(hash) = fields.next() else { continue };
        let Some(short_hash) = fields.next() else {
            continue;
        };
        let Some(subject) = fields.next() else {
            continue;
        };

        let hash = String::from_utf8_lossy(hash).trim().to_string();
        if hash.is_empty() {
            continue;
        }

        commits.push(CommitEntry {
            hash,
            short_hash: String::from_utf8_lossy(short_hash).trim().to_string(),
            subject: String::from_utf8_lossy(subject).trim().to_string(),
        });
    }

    Ok(commits)
}

fn fetch_git_status(repo_root: &Path) -> Result<Vec<FileEntry>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["status", "--porcelain=v2", "-z"])
        .output()
        .context("执行 git status 失败")?;

    if !output.status.success() {
        return Err(anyhow!(
            "git status 返回非零: {}",
            output.status.code().unwrap_or(-1)
        ));
    }

    let mut entries = Vec::new();
    let mut segments = output.stdout.split(|b| *b == b'\0').peekable();
    while let Some(record) = segments.next() {
        if record.is_empty() {
            continue;
        }

        let record = String::from_utf8_lossy(record);
        if record.starts_with("1 ") {
            if let Some(entry) = parse_type_1_record(&record) {
                entries.push(entry);
            }
            continue;
        }

        if record.starts_with("2 ") {
            if let Some(entry) = parse_type_2_record(&record) {
                entries.push(entry);
            }
            _ = segments.next(); // orig_path
            continue;
        }

        if record.starts_with("u ") {
            if let Some(entry) = parse_unmerged_record(&record) {
                entries.push(entry);
            }
            continue;
        }

        if record.starts_with("? ") {
            if let Some(path) = record.strip_prefix("? ") {
                entries.push(FileEntry {
                    path: path.to_string(),
                    status: "??".to_string(),
                });
            }
            continue;
        }

        if record.starts_with("! ") {
            if let Some(path) = record.strip_prefix("! ") {
                entries.push(FileEntry {
                    path: path.to_string(),
                    status: "!!".to_string(),
                });
            }
        }
    }

    Ok(entries)
}

fn detect_repo_root(start_dir: &Path) -> PathBuf {
    let output = Command::new("git")
        .arg("-C")
        .arg(start_dir)
        .args(["rev-parse", "--show-toplevel"])
        .output();

    let Ok(output) = output else {
        return start_dir.to_path_buf();
    };
    if !output.status.success() {
        return start_dir.to_path_buf();
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return start_dir.to_path_buf();
    }

    PathBuf::from(path)
}

fn parse_type_1_record(record: &str) -> Option<FileEntry> {
    // `1 <xy> <sub> <mH> <mI> <mW> <hH> <hI> <path>`
    let mut parts = record.splitn(9, ' ');
    let _type_ = parts.next()?;
    let status = parts.next()?.to_string();
    let path = parts.nth(6)?.to_string();
    Some(FileEntry { path, status })
}

fn parse_type_2_record(record: &str) -> Option<FileEntry> {
    // `2 <xy> <sub> <mH> <mI> <mW> <hH> <hI> <X> <score> <path> \0 <orig_path>`
    let mut parts = record.splitn(11, ' ');
    let _type_ = parts.next()?;
    let status = parts.next()?.to_string();
    let path = parts.nth(8)?.to_string();
    Some(FileEntry { path, status })
}

fn parse_unmerged_record(record: &str) -> Option<FileEntry> {
    // `u <xy> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>`
    let mut parts = record.splitn(11, ' ');
    let _type_ = parts.next()?;
    let status = parts.next()?.to_string();
    let path = parts.nth(8)?.to_string();
    Some(FileEntry { path, status })
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
        counts.all = entries.len();

        for entry in entries {
            let status = entry.status.as_str();
            if is_untracked_status(status) {
                counts.untracked += 1;
                continue;
            }
            if is_conflict_status(status) {
                counts.conflicts += 1;
                continue;
            }

            if let Some((x, y)) = status_xy(status) {
                if x != ' ' && x != '?' && x != '!' {
                    counts.staged += 1;
                }
                if y != ' ' && y != '?' && y != '!' {
                    counts.unstaged += 1;
                }
            }
        }

        counts
    }
}

fn matches_filter(entry: &FileEntry, filter: StatusFilter) -> bool {
    match filter {
        StatusFilter::All => true,
        StatusFilter::Conflicts => is_conflict_status(&entry.status),
        StatusFilter::Untracked => is_untracked_status(&entry.status),
        StatusFilter::Staged => {
            if is_conflict_status(&entry.status) || is_untracked_status(&entry.status) {
                return false;
            }
            status_xy(&entry.status).is_some_and(|(x, _)| x != ' ' && x != '?' && x != '!')
        }
        StatusFilter::Unstaged => {
            if is_conflict_status(&entry.status) || is_untracked_status(&entry.status) {
                return false;
            }
            status_xy(&entry.status).is_some_and(|(_, y)| y != ' ' && y != '?' && y != '!')
        }
    }
}

fn status_xy(status: &str) -> Option<(char, char)> {
    let mut chars = status.chars();
    Some((chars.next()?, chars.next()?))
}

fn is_untracked_status(status: &str) -> bool {
    status == "??"
}

fn is_conflict_status(status: &str) -> bool {
    status.contains('U') || status == "AA" || status == "DD"
}

fn default_compare_target(status: &str) -> CompareTarget {
    if is_untracked_status(status) {
        return CompareTarget::HeadToWorktree;
    }

    let Some((x, y)) = status_xy(status) else {
        return CompareTarget::HeadToWorktree;
    };

    if y != ' ' && y != '?' && y != '!' {
        return CompareTarget::IndexToWorktree;
    }
    if x != ' ' && x != '?' && x != '!' {
        return CompareTarget::HeadToIndex;
    }

    CompareTarget::HeadToWorktree
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

fn display_ref_label(label: &str) -> SharedString {
    let label = label.trim();
    if label.is_empty() || label.eq_ignore_ascii_case("WORKTREE") {
        return "工作区".into();
    }
    if label == ":" || label.eq_ignore_ascii_case("INDEX") {
        return "暂存".into();
    }
    if label.eq_ignore_ascii_case("HEAD") {
        return "HEAD".into();
    }
    label.to_string().into()
}

fn compare_target_label(target: &CompareTarget) -> SharedString {
    match target {
        CompareTarget::HeadToWorktree => "HEAD ↔ 工作区".into(),
        CompareTarget::IndexToWorktree => "暂存 ↔ 工作区".into(),
        CompareTarget::HeadToIndex => "HEAD ↔ 暂存".into(),
        CompareTarget::Refs { left, right } => {
            format!("{} ↔ {}", display_ref_label(left), display_ref_label(right)).into()
        }
    }
}

fn compare_target_side_label(target: &CompareTarget, side: Side) -> SharedString {
    match (target, side) {
        (CompareTarget::HeadToWorktree, Side::Old) => "HEAD".into(),
        (CompareTarget::HeadToWorktree, Side::New) => "工作区".into(),
        (CompareTarget::IndexToWorktree, Side::Old) => "暂存".into(),
        (CompareTarget::IndexToWorktree, Side::New) => "工作区".into(),
        (CompareTarget::HeadToIndex, Side::Old) => "HEAD".into(),
        (CompareTarget::HeadToIndex, Side::New) => "暂存".into(),
        (CompareTarget::Refs { left, .. }, Side::Old) => display_ref_label(left),
        (CompareTarget::Refs { left: _, right }, Side::New) => display_ref_label(right),
    }
}

fn unified_patch_for_hunk(path: &str, hunk: &diffview::DiffHunk) -> String {
    fn start_number(start: usize, len: usize) -> usize {
        if len == 0 { start } else { start + 1 }
    }

    let old_start = start_number(hunk.old_start, hunk.old_len);
    let new_start = start_number(hunk.new_start, hunk.new_len);

    let mut out = String::new();
    out.push_str(&format!("--- a/{path}\n"));
    out.push_str(&format!("+++ b/{path}\n"));
    out.push_str(&format!(
        "@@ -{old_start},{} +{new_start},{} @@\n",
        hunk.old_len, hunk.new_len
    ));

    for row in &hunk.rows {
        match row.kind() {
            diffview::DiffRowKind::Unchanged => {
                let text = row
                    .old
                    .as_ref()
                    .map(|line| line.text.as_str())
                    .or_else(|| row.new.as_ref().map(|line| line.text.as_str()))
                    .unwrap_or_default();
                out.push(' ');
                out.push_str(text);
                out.push('\n');
            }
            diffview::DiffRowKind::Removed => {
                let text = row
                    .old
                    .as_ref()
                    .map(|line| line.text.as_str())
                    .unwrap_or_default();
                out.push('-');
                out.push_str(text);
                out.push('\n');
            }
            diffview::DiffRowKind::Added => {
                let text = row
                    .new
                    .as_ref()
                    .map(|line| line.text.as_str())
                    .unwrap_or_default();
                out.push('+');
                out.push_str(text);
                out.push('\n');
            }
            diffview::DiffRowKind::Modified => {
                if let Some(old) = row.old.as_ref() {
                    out.push('-');
                    out.push_str(&old.text);
                    out.push('\n');
                }
                if let Some(new) = row.new.as_ref() {
                    out.push('+');
                    out.push_str(&new.text);
                    out.push('\n');
                }
            }
        }
    }

    out
}

fn main() {
    let app = Application::new();

    app.run(move |cx| {
        gpui_component::init(cx);
        init_keybindings(cx);
        cx.activate(true);

        cx.spawn(async move |cx| {
            cx.open_window(
                WindowOptions {
                    titlebar: Some(TitleBar::title_bar_options()),
                    ..Default::default()
                },
                |window, cx| {
                    window.set_window_title("git-viewer");
                    let view = cx.new(|cx| GitViewerApp::new(window, cx));
                    let handle = view.read(cx).focus_handle();
                    window.focus(&handle);
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
