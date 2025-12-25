use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::Context as _;
use anyhow::{Result, anyhow};
use gpui::*;
use gpui_component::{
    ActiveTheme as _, Disableable as _, Root, TitleBar, VirtualListScrollHandle, WindowExt as _,
    button::{Button, ButtonVariants as _},
    notification::Notification,
    scroll::{Scrollbar, ScrollbarState},
    switch::Switch,
    v_virtual_list,
};

#[derive(Clone, Debug)]
struct FileEntry {
    path: String,
    status: String,
}

#[derive(Clone, Debug)]
enum AppScreen {
    StatusList,
    DiffView,
}

#[derive(Clone, Copy, Debug)]
struct DiffViewOptions {
    ignore_whitespace: bool,
    context_lines: usize,
}

const MAX_CONTEXT_LINES: usize = 20;

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
    old_text: String,
    new_text: String,
    ignore_whitespace: bool,
    context_lines: usize,
    old_lines: Vec<String>,
    new_lines: Vec<String>,
    rows: Vec<DisplayRow>,
    hunk_rows: Vec<usize>,
    current_hunk: usize,
    scroll_handle: VirtualListScrollHandle,
    scroll_state: ScrollbarState,
}

struct GitViewerApp {
    repo_root: PathBuf,
    files: Vec<FileEntry>,
    loading: bool,
    screen: AppScreen,
    diff_view: Option<DiffViewState>,
    diff_options: DiffViewOptions,
    split_layout: SplitLayout,
    view_mode: DiffViewMode,
}

impl GitViewerApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let this = cx.entity();
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let repo_root = detect_repo_root(&cwd);
        let repo_root_for_task = repo_root.clone();

        cx.spawn_in(window, async move |_, window| {
            let entries = fetch_git_status(&repo_root_for_task)
                .map_err(|err| {
                    eprintln!("git status failed: {err:?}");
                    err
                })
                .unwrap_or_default();

            let _ = window.update(|_window, cx| {
                this.update(cx, |this, _cx| {
                    this.loading = false;
                    this.files = entries;
                })
            });

            Some(())
        })
        .detach();

        Self {
            repo_root,
            files: Vec::new(),
            loading: true,
            screen: AppScreen::StatusList,
            diff_view: None,
            diff_options: DiffViewOptions {
                ignore_whitespace: false,
                context_lines: 3,
            },
            split_layout: SplitLayout::TwoPane,
            view_mode: DiffViewMode::Split,
        }
    }

    fn open_demo(&mut self) {
        let (old, new) = demo_texts();
        self.open_diff_view("Split Diff Demo".into(), old, new);
    }

    fn open_diff_view(&mut self, title: SharedString, old_text: String, new_text: String) {
        self.diff_view = Some(DiffViewState::new(
            title,
            old_text,
            new_text,
            self.diff_options,
            self.view_mode,
        ));
        self.screen = AppScreen::DiffView;
    }

    fn close_diff_view(&mut self) {
        self.screen = AppScreen::StatusList;
    }

    fn open_file_diff(
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
            Notification::new().message(format!("正在加载 diff：{status} {path}")),
            cx,
        );

        cx.spawn_in(window, async move |_, window| {
            let (old_text, old_err) =
                match read_head_file(&repo_root, &path_for_task, &status_for_task) {
                    Ok(text) => (text, None),
                    Err(err) => (String::new(), Some(err.to_string())),
                };
            let (new_text, new_err) = match read_working_file(&repo_root, &path_for_task) {
                Ok(text) => (text, None),
                Err(err) => (String::new(), Some(err.to_string())),
            };

            window
                .update(|window, cx| {
                    if let Some(err) = old_err {
                        window.push_notification(
                            Notification::new()
                                .message(format!("读取 HEAD 版本失败，按空内容处理：{err}")),
                            cx,
                        );
                    }
                    if let Some(err) = new_err {
                        window.push_notification(
                            Notification::new()
                                .message(format!("读取工作区版本失败，按空内容处理：{err}")),
                            cx,
                        );
                    }

                    this.update(cx, |this, _cx| {
                        this.open_diff_view(
                            format!("{status_for_task} {path_for_task}").into(),
                            old_text,
                            new_text,
                        );
                    });
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn set_ignore_whitespace(&mut self, value: bool) {
        if self.diff_options.ignore_whitespace == value {
            return;
        }
        self.diff_options.ignore_whitespace = value;
        self.rebuild_active_diff_view();
    }

    fn set_context_lines(&mut self, value: usize) {
        let value = value.min(MAX_CONTEXT_LINES);
        if self.diff_options.context_lines == value {
            return;
        }
        self.diff_options.context_lines = value;
        self.rebuild_active_diff_view();
    }

    fn set_view_mode(&mut self, mode: DiffViewMode) {
        if self.view_mode == mode {
            return;
        }
        self.view_mode = mode;
        self.rebuild_active_diff_view();
    }

    fn rebuild_active_diff_view(&mut self) {
        let Some(diff_view) = self.diff_view.as_ref() else {
            return;
        };

        let scroll_handle = diff_view.scroll_handle.clone();
        let scroll_state = diff_view.scroll_state.clone();
        let title = diff_view.title.clone();
        let old_text = diff_view.old_text.clone();
        let new_text = diff_view.new_text.clone();
        let current_hunk = diff_view.current_hunk;

        let mut next =
            DiffViewState::new(title, old_text, new_text, self.diff_options, self.view_mode);
        next.scroll_handle = scroll_handle;
        next.scroll_state = scroll_state;
        next.current_hunk = current_hunk.min(next.hunk_rows.len().saturating_sub(1));

        self.diff_view = Some(next);
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

    fn render_status_list(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Div {
        let header: SharedString = if self.loading {
            "正在加载 git 状态…".into()
        } else {
            format!("发现 {} 个变更文件", self.files.len()).into()
        };

        let demo_button = Button::new("open-demo")
            .label("打开 Diff Demo")
            .ghost()
            .on_click(cx.listener(|this, _, _window, cx| {
                this.open_demo();
                cx.notify();
            }));

        let list: Vec<AnyElement> = if self.loading {
            vec![div().child("加载中…").into_any_element()]
        } else if self.files.is_empty() {
            vec![div().child("没有检测到变更文件").into_any_element()]
        } else {
            self.files
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    let path = entry.path.clone();
                    let status = entry.status.clone();
                    Button::new(("file", index))
                        .label(format!("{status} {path}"))
                        .w_full()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            println!("[git-viewer] 打开 diff: {status} {path}");
                            this.open_file_diff(path.clone(), status.clone(), window, cx);
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
                    .child(demo_button),
            )
            .child(div().flex_col().gap(px(6.)).children(list))
    }

    fn render_diff_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let Some(diff_view) = self.diff_view.as_ref() else {
            return div().p(px(12.)).child("No diff view");
        };

        let view_mode = self.view_mode;
        let inline_mode = view_mode == DiffViewMode::Inline;
        let two_pane = matches!(self.split_layout, SplitLayout::TwoPane);
        let title = diff_view.title.clone();
        let ignore_whitespace = diff_view.ignore_whitespace;
        let context_lines = diff_view.context_lines;
        let hunk_count = diff_view.hunk_rows.len();
        let can_prev_hunk = diff_view.current_hunk > 0;
        let can_next_hunk = diff_view.current_hunk + 1 < diff_view.hunk_rows.len();
        let rows_len = diff_view.rows.len();
        let scroll_handle = diff_view.scroll_handle.clone();
        let scroll_state = diff_view.scroll_state.clone();
        let row_height = window.line_height() + px(4.);

        let toolbar = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(12.))
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
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.close_diff_view();
                                cx.notify();
                            })),
                    )
                    .child(div().child(title)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
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
                    .child(div().child(format!("hunks: {hunk_count} / rows: {rows_len}")))
                    .child(
                        Button::new("prev-hunk")
                            .label("上一 hunk")
                            .ghost()
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
                            .disabled(!can_next_hunk)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.jump_hunk(1);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("expand-all")
                            .label("展开全部")
                            .ghost()
                            .on_click(cx.listener(|this, _, _window, cx| {
                                while this.diff_view.as_ref().map_or(false, |diff_view| {
                                    diff_view
                                        .rows
                                        .iter()
                                        .any(|row| matches!(row, DisplayRow::Fold { .. }))
                                }) {
                                    let next_index =
                                        this.diff_view.as_ref().and_then(|diff_view| {
                                            diff_view.rows.iter().enumerate().find_map(
                                                |(index, row)| {
                                                    matches!(row, DisplayRow::Fold { .. })
                                                        .then_some(index)
                                                },
                                            )
                                        });
                                    let Some(next_index) = next_index else { break };
                                    this.expand_fold(next_index);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.))
                            .child(div().child(format!("上下文: {context_lines}")))
                            .child(
                                Button::new("context-dec")
                                    .label("－")
                                    .ghost()
                                    .disabled(context_lines == 0)
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        let next =
                                            this.diff_options.context_lines.saturating_sub(1);
                                        this.set_context_lines(next);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("context-inc")
                                    .label("＋")
                                    .ghost()
                                    .disabled(context_lines >= MAX_CONTEXT_LINES)
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        let next = this.diff_options.context_lines + 1;
                                        this.set_context_lines(next);
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        Switch::new("two-pane")
                            .label("分栏")
                            .checked(two_pane)
                            .disabled(inline_mode)
                            .on_click(cx.listener(|this, checked, _window, cx| {
                                this.split_layout = if *checked {
                                    SplitLayout::TwoPane
                                } else {
                                    SplitLayout::Aligned
                                };
                                cx.notify();
                            })),
                    )
                    .child(
                        Switch::new("ignore-whitespace")
                            .label("忽略空白")
                            .checked(ignore_whitespace)
                            .on_click(cx.listener(|this, checked, _window, cx| {
                                this.set_ignore_whitespace(*checked);
                                cx.notify();
                            })),
                    ),
            );

        let item_sizes = Rc::new(vec![size(px(0.), row_height); rows_len]);
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
                        .child("HEAD");
                    let new_header = div()
                        .h(pane_header_height)
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .bg(theme.muted.alpha(0.2))
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("工作区");

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
                .child("对比: HEAD ↔ 工作区")
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
                .child("对比: HEAD ↔ 工作区")
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
            .child(format!("rows: {rows_len}"));
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

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(toolbar)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .relative()
                    .overflow_hidden()
                    .child(list)
                    .child(Scrollbar::uniform_scroll(&scroll_state, &scroll_handle)),
            )
            .child(status_bar)
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
        match self.screen {
            AppScreen::StatusList => self.render_status_list(window, cx).into_any_element(),
            AppScreen::DiffView => self.render_diff_view(window, cx).into_any_element(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Side {
    Old,
    New,
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
        old_text: String,
        new_text: String,
        options: DiffViewOptions,
        view_mode: DiffViewMode,
    ) -> Self {
        let (old_lines, new_lines, rows) = build_display_rows(
            &old_text,
            &new_text,
            options.ignore_whitespace,
            options.context_lines,
            view_mode,
        );
        let mut this = Self {
            title,
            old_text,
            new_text,
            ignore_whitespace: options.ignore_whitespace,
            context_lines: options.context_lines,
            old_lines,
            new_lines,
            rows,
            hunk_rows: Vec::new(),
            current_hunk: 0,
            scroll_handle: VirtualListScrollHandle::new(),
            scroll_state: ScrollbarState::default(),
        };
        this.recalc_hunk_rows();
        this
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

fn build_display_rows(
    old_text: &str,
    new_text: &str,
    ignore_whitespace: bool,
    context_lines: usize,
    view_mode: DiffViewMode,
) -> (Vec<String>, Vec<String>, Vec<DisplayRow>) {
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

    let mut rows = Vec::new();
    let mut old_pos = 0usize;
    let mut new_pos = 0usize;

    for hunk in model.hunks {
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

        for row in hunk.rows {
            let kind = row.kind();
            let old_line = row.old.as_ref().map(|l| l.line_index + 1);
            let new_line = row.new.as_ref().map(|l| l.line_index + 1);
            let old_segments = row.old.map(|l| l.segments).unwrap_or_default();
            let new_segments = row.new.map(|l| l.segments).unwrap_or_default();

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

    (old_lines, new_lines, rows)
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

fn main() {
    let app = Application::new();

    app.run(move |cx| {
        gpui_component::init(cx);
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
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
