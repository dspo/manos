use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) enum Side {
    Old,
    New,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ConflictPane {
    Ours,
    Base,
    Theirs,
}

pub(super) fn render_side(
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

pub(super) fn render_segments(
    segments: &[diffview::DiffSegment],
    theme: &gpui_component::Theme,
) -> Vec<Div> {
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

pub(super) fn preserve_spaces(text: &str) -> String {
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

pub(super) fn code_language_for_path(path: &str) -> SharedString {
    let path = Path::new(path);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let language = match ext.as_str() {
        "rs" => "rust",
        "toml" => "toml",
        "json" | "jsonc" => "json",
        "yml" | "yaml" => "yaml",
        "md" | "markdown" | "mdx" => "markdown",
        "js" => "javascript",
        "ts" => "typescript",
        "tsx" => "tsx",
        "jsx" => "tsx",
        "css" | "scss" => "css",
        "html" | "htm" => "html",
        "go" => "go",
        "py" => "python",
        "sh" | "bash" => "bash",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => "cpp",
        "cs" => "csharp",
        "swift" => "swift",
        "zig" => "zig",
        "sql" => "sql",
        "proto" => "proto",
        "rb" => "ruby",
        "scala" => "scala",
        "cmake" => "cmake",
        "ejs" => "ejs",
        "erb" => "erb",
        "diff" | "patch" => "diff",
        _ => {
            if file_name == "makefile" {
                "make"
            } else {
                "text"
            }
        }
    };

    language.into()
}

impl DiffViewState {
    pub(super) fn new(
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
            orig_path: None,
            status,
            compare_target,
            rows_view_mode: view_mode,
            old_text,
            new_text,
            old_lines,
            new_lines,
            diff_model,
            rows,
            selected_rows: HashSet::new(),
            selection_anchor: None,
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

    pub(super) fn from_precomputed(
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
            orig_path: None,
            status,
            compare_target,
            rows_view_mode: view_mode,
            old_text,
            new_text,
            old_lines,
            new_lines,
            diff_model,
            rows,
            selected_rows: HashSet::new(),
            selection_anchor: None,
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

    pub(super) fn from_precomputed_rows(
        title: SharedString,
        path: Option<String>,
        status: Option<String>,
        compare_target: CompareTarget,
        view_mode: DiffViewMode,
        old_text: String,
        new_text: String,
        diff_model: diffview::DiffModel,
        old_lines: Vec<String>,
        new_lines: Vec<String>,
        rows: Vec<DisplayRow>,
        hunk_rows: Vec<usize>,
    ) -> Self {
        Self {
            title,
            path,
            orig_path: None,
            status,
            compare_target,
            rows_view_mode: view_mode,
            old_text,
            new_text,
            old_lines,
            new_lines,
            diff_model,
            rows,
            selected_rows: HashSet::new(),
            selection_anchor: None,
            hunk_rows,
            current_hunk: 0,
            scroll_handle: VirtualListScrollHandle::new(),
            scroll_state: ScrollbarState::default(),
            list_item_sizes: Rc::new(Vec::new()),
            list_item_height: px(0.),
        }
    }

    pub(super) fn loading(
        title: SharedString,
        path: Option<String>,
        status: Option<String>,
        compare_target: CompareTarget,
    ) -> Self {
        Self {
            title,
            path,
            orig_path: None,
            status,
            compare_target,
            rows_view_mode: DiffViewMode::Split,
            old_text: String::new(),
            new_text: String::new(),
            old_lines: Vec::new(),
            new_lines: Vec::new(),
            diff_model: diffview::DiffModel::default(),
            rows: vec![DisplayRow::HunkHeader {
                text: "Loading…".into(),
            }],
            selected_rows: HashSet::new(),
            selection_anchor: None,
            hunk_rows: Vec::new(),
            current_hunk: 0,
            scroll_handle: VirtualListScrollHandle::new(),
            scroll_state: ScrollbarState::default(),
            list_item_sizes: Rc::new(Vec::new()),
            list_item_height: px(0.),
        }
    }

    pub(super) fn item_sizes(&mut self, row_height: Pixels) -> Rc<Vec<Size<Pixels>>> {
        let count = self.rows.len();
        if self.list_item_height != row_height || self.list_item_sizes.len() != count {
            self.list_item_height = row_height;
            self.list_item_sizes = Rc::new(vec![size(px(0.), row_height); count]);
        }
        self.list_item_sizes.clone()
    }

    pub(super) fn rebuild_rows(&mut self, view_mode: DiffViewMode) {
        self.rows = build_display_rows_from_model(
            &self.diff_model,
            &self.old_lines,
            &self.new_lines,
            view_mode,
        );
        self.rows_view_mode = view_mode;
        self.recalc_hunk_rows();
    }

    pub(super) fn recalc_hunk_rows(&mut self) {
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
    pub(super) fn loading(
        title: SharedString,
        path: Option<String>,
        result_input: Entity<InputState>,
    ) -> Self {
        Self {
            title,
            path,
            text: String::new(),
            result_input,
            show_result_editor: true,
            conflicts: Vec::new(),
            rows: vec![ConflictRow::EmptyState {
                text: "Loading…".into(),
            }],
            conflict_rows: Vec::new(),
            current_conflict: 0,
            scroll_handle: VirtualListScrollHandle::new(),
            scroll_state: ScrollbarState::default(),
            list_item_sizes: Rc::new(Vec::new()),
            list_item_height: px(0.),
        }
    }

    pub(super) fn new(
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

    pub(super) fn item_sizes(&mut self, row_height: Pixels) -> Rc<Vec<Size<Pixels>>> {
        let count = self.rows.len();
        if self.list_item_height != row_height || self.list_item_sizes.len() != count {
            self.list_item_height = row_height;
            self.list_item_sizes = Rc::new(vec![size(px(0.), row_height); count]);
        }
        self.list_item_sizes.clone()
    }

    pub(super) fn rebuild(&mut self) {
        self.conflicts = diffview::parse_conflicts(&self.text);
        self.rows = build_conflict_rows(&self.text, &self.conflicts);
        self.recalc_conflict_rows();
    }

    pub(super) fn recalc_conflict_rows(&mut self) {
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

pub(super) fn demo_texts() -> (String, String) {
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

pub(super) fn large_demo_texts(line_count: usize) -> (String, String) {
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

pub(super) fn conflict_demo_text() -> String {
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

pub(super) fn build_diff_model(
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

pub(super) fn build_display_rows_from_model(
    model: &diffview::DiffModel,
    old_lines: &[String],
    new_lines: &[String],
    view_mode: DiffViewMode,
) -> Vec<DisplayRow> {
    let mut rows = Vec::new();
    let mut old_pos = 0usize;
    let mut new_pos = 0usize;

    for (hunk_index, hunk) in model.hunks.iter().enumerate() {
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

        for (row_index, row) in hunk.rows.iter().enumerate() {
            let source = Some(DiffRowRef {
                hunk_index,
                row_index,
            });
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
                    source,
                    kind,
                    old_line,
                    new_line,
                    old_segments,
                    new_segments,
                }),
                DiffViewMode::Inline => match kind {
                    diffview::DiffRowKind::Modified => {
                        rows.push(DisplayRow::Code {
                            source,
                            kind: diffview::DiffRowKind::Removed,
                            old_line,
                            new_line: None,
                            old_segments,
                            new_segments: Vec::new(),
                        });
                        rows.push(DisplayRow::Code {
                            source,
                            kind: diffview::DiffRowKind::Added,
                            old_line: None,
                            new_line,
                            old_segments: Vec::new(),
                            new_segments,
                        });
                    }
                    diffview::DiffRowKind::Added => rows.push(DisplayRow::Code {
                        source,
                        kind,
                        old_line: None,
                        new_line,
                        old_segments: Vec::new(),
                        new_segments,
                    }),
                    diffview::DiffRowKind::Removed => rows.push(DisplayRow::Code {
                        source,
                        kind,
                        old_line,
                        new_line: None,
                        old_segments,
                        new_segments: Vec::new(),
                    }),
                    diffview::DiffRowKind::Unchanged => rows.push(DisplayRow::Code {
                        source,
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

pub(super) fn build_conflict_rows(
    text: &str,
    conflicts: &[diffview::ConflictRegion],
) -> Vec<ConflictRow> {
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

pub(super) fn compare_paths_for_file(
    path: &str,
    orig_path: Option<&str>,
    status: &str,
    target: &CompareTarget,
) -> (String, String) {
    let Some(orig_path) = orig_path.filter(|path| !path.trim().is_empty()) else {
        return (path.to_string(), path.to_string());
    };
    if matches!(target, CompareTarget::Refs { .. }) {
        return (path.to_string(), path.to_string());
    }

    let (x, _) = status_xy(status).unwrap_or(('.', '.'));
    let head_path = orig_path.to_string();
    let index_path = if is_clean_status_code(x) {
        head_path.clone()
    } else {
        path.to_string()
    };
    let worktree_path = path.to_string();

    match target {
        CompareTarget::HeadToWorktree => (head_path, worktree_path),
        CompareTarget::IndexToWorktree => (index_path, worktree_path),
        CompareTarget::HeadToIndex => (head_path, index_path),
        CompareTarget::Refs { .. } => (path.to_string(), path.to_string()),
    }
}

pub(super) async fn read_compare_texts_for_target(
    window: &mut AsyncWindowContext,
    repo_root: PathBuf,
    old_path: String,
    new_path: String,
    status: String,
    target: CompareTarget,
) -> (String, Option<String>, String, Option<String>) {
    let executor = window.background_executor();

    let old_handle = {
        let repo_root = repo_root.clone();
        let path = old_path;
        let status = status.clone();
        let target = target.clone();
        executor.spawn(async move {
            let result = match target {
                CompareTarget::HeadToWorktree | CompareTarget::HeadToIndex => {
                    read_head_file(&repo_root, &path, &status)
                }
                CompareTarget::IndexToWorktree => read_index_file(&repo_root, &path, &status),
                CompareTarget::Refs { left, .. } => read_specified_file(&repo_root, &path, &left),
            };

            match result {
                Ok(text) => (text, None),
                Err(err) => (String::new(), Some(err.to_string())),
            }
        })
    };

    let new_handle = {
        let repo_root = repo_root;
        let path = new_path;
        let status = status.clone();
        executor.spawn(async move {
            let result = match target {
                CompareTarget::HeadToWorktree | CompareTarget::IndexToWorktree => {
                    read_working_file(&repo_root, &path)
                }
                CompareTarget::HeadToIndex => read_index_file(&repo_root, &path, &status),
                CompareTarget::Refs { right, .. } => read_specified_file(&repo_root, &path, &right),
            };

            match result {
                Ok(text) => (text, None),
                Err(err) => (String::new(), Some(err.to_string())),
            }
        })
    };

    let (old_text, old_err) = old_handle.await;
    let (new_text, new_err) = new_handle.await;
    (old_text, old_err, new_text, new_err)
}
pub(super) fn display_ref_label(label: &str) -> SharedString {
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

pub(super) fn compare_target_label(target: &CompareTarget) -> SharedString {
    match target {
        CompareTarget::HeadToWorktree => "HEAD ↔ 工作区".into(),
        CompareTarget::IndexToWorktree => "暂存 ↔ 工作区".into(),
        CompareTarget::HeadToIndex => "HEAD ↔ 暂存".into(),
        CompareTarget::Refs { left, right } => {
            format!("{} ↔ {}", display_ref_label(left), display_ref_label(right)).into()
        }
    }
}

pub(super) fn compare_target_side_label(target: &CompareTarget, side: Side) -> SharedString {
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

pub(super) fn compare_target_new_is_worktree(target: &CompareTarget) -> bool {
    match target {
        CompareTarget::HeadToWorktree | CompareTarget::IndexToWorktree => true,
        CompareTarget::HeadToIndex => false,
        CompareTarget::Refs { right, .. } => {
            let right = right.trim();
            right.is_empty() || right.eq_ignore_ascii_case("WORKTREE")
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PatchUnselectedContext {
    Old,
    New,
}

pub(super) fn unified_patch_for_hunk(path: &str, hunk: &diffview::DiffHunk) -> String {
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

pub(super) fn unified_patch_for_selection(
    path: &str,
    model: &diffview::DiffModel,
    selection: &HashSet<DiffRowRef>,
    unselected_context: PatchUnselectedContext,
) -> Option<String> {
    if selection.is_empty() {
        return None;
    }

    let mut sections = Vec::new();
    for (hunk_index, hunk) in model.hunks.iter().enumerate() {
        let mut any_selected = false;
        for row_index in 0..hunk.rows.len() {
            if selection.contains(&DiffRowRef {
                hunk_index,
                row_index,
            }) {
                any_selected = true;
                break;
            }
        }
        if !any_selected {
            continue;
        }

        if let Some(section) = unified_patch_section_for_hunk_selection(
            hunk,
            hunk_index,
            selection,
            unselected_context,
        ) {
            sections.push(section);
        }
    }

    if sections.is_empty() {
        return None;
    }

    let mut out = String::new();
    out.push_str(&format!("--- a/{path}\n"));
    out.push_str(&format!("+++ b/{path}\n"));
    for section in sections {
        out.push_str(&section);
    }
    Some(out)
}

pub(super) fn unified_patch_section_for_hunk_selection(
    hunk: &diffview::DiffHunk,
    hunk_index: usize,
    selection: &HashSet<DiffRowRef>,
    unselected_context: PatchUnselectedContext,
) -> Option<String> {
    fn start_number(start: usize, len: usize) -> usize {
        if len == 0 { start } else { start + 1 }
    }

    let mut has_selected = false;
    let mut has_change = false;
    let mut old_len = 0usize;
    let mut new_len = 0usize;
    let mut lines: Vec<String> = Vec::new();

    for (row_index, row) in hunk.rows.iter().enumerate() {
        let is_selected = selection.contains(&DiffRowRef {
            hunk_index,
            row_index,
        });
        if is_selected {
            has_selected = true;
        }

        match row.kind() {
            diffview::DiffRowKind::Unchanged => {
                let text = row
                    .old
                    .as_ref()
                    .map(|line| line.text.as_str())
                    .or_else(|| row.new.as_ref().map(|line| line.text.as_str()))
                    .unwrap_or_default();
                lines.push(format!(" {text}"));
                old_len += 1;
                new_len += 1;
            }
            diffview::DiffRowKind::Added => {
                let text = row
                    .new
                    .as_ref()
                    .map(|line| line.text.as_str())
                    .unwrap_or_default();
                if is_selected {
                    lines.push(format!("+{text}"));
                    new_len += 1;
                    has_change = true;
                } else if unselected_context == PatchUnselectedContext::New {
                    lines.push(format!(" {text}"));
                    old_len += 1;
                    new_len += 1;
                }
            }
            diffview::DiffRowKind::Removed => {
                let text = row
                    .old
                    .as_ref()
                    .map(|line| line.text.as_str())
                    .unwrap_or_default();
                if is_selected {
                    lines.push(format!("-{text}"));
                    old_len += 1;
                    has_change = true;
                } else if unselected_context == PatchUnselectedContext::Old {
                    lines.push(format!(" {text}"));
                    old_len += 1;
                    new_len += 1;
                }
            }
            diffview::DiffRowKind::Modified => {
                if is_selected {
                    if let Some(old) = row.old.as_ref() {
                        lines.push(format!("-{}", old.text));
                        old_len += 1;
                        has_change = true;
                    }
                    if let Some(new) = row.new.as_ref() {
                        lines.push(format!("+{}", new.text));
                        new_len += 1;
                        has_change = true;
                    }
                } else {
                    let text = match unselected_context {
                        PatchUnselectedContext::Old => row
                            .old
                            .as_ref()
                            .map(|line| line.text.as_str())
                            .or_else(|| row.new.as_ref().map(|line| line.text.as_str()))
                            .unwrap_or_default(),
                        PatchUnselectedContext::New => row
                            .new
                            .as_ref()
                            .map(|line| line.text.as_str())
                            .or_else(|| row.old.as_ref().map(|line| line.text.as_str()))
                            .unwrap_or_default(),
                    };
                    lines.push(format!(" {text}"));
                    old_len += 1;
                    new_len += 1;
                }
            }
        }
    }

    if !has_selected || !has_change {
        return None;
    }

    let old_start = start_number(hunk.old_start, old_len);
    let new_start = start_number(hunk.new_start, new_len);

    let mut out = String::new();
    out.push_str(&format!(
        "@@ -{old_start},{old_len} +{new_start},{new_len} @@\n"
    ));
    for line in lines {
        out.push_str(&line);
        out.push('\n');
    }
    Some(out)
}
