use std::ops::Range;

use gpui::{
    Action, App, Bounds, ClipboardItem, Context, EntityInputHandler, FocusHandle, FontStyle,
    FontWeight, HighlightStyle, KeyBinding, Pixels, Point, ScrollHandle, SharedString,
    StrikethroughStyle, UTF16Selection, UnderlineStyle, Window, actions, point, px,
};
use ropey::Rope;
use serde::Deserialize;
use sum_tree::Bias;

use crate::RichTextTheme;
use crate::rope_ext::RopeExt as _;
use crate::selection::Selection;

use super::style::{InlineStyle, StyleRuns};

pub(super) const CONTEXT: &str = "RichText";

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = rich_text, no_json)]
pub struct Enter {
    pub secondary: bool,
}

actions!(
    rich_text,
    [
        Backspace,
        Delete,
        Escape,
        MoveLeft,
        MoveRight,
        MoveUp,
        MoveDown,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        ToggleBold,
        ToggleItalic,
        ToggleUnderline,
        ToggleStrikethrough,
    ]
);

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some(CONTEXT)),
        KeyBinding::new("delete", Delete, Some(CONTEXT)),
        KeyBinding::new("enter", Enter { secondary: false }, Some(CONTEXT)),
        KeyBinding::new("secondary-enter", Enter { secondary: true }, Some(CONTEXT)),
        KeyBinding::new("escape", Escape, Some(CONTEXT)),
        KeyBinding::new("left", MoveLeft, Some(CONTEXT)),
        KeyBinding::new("right", MoveRight, Some(CONTEXT)),
        KeyBinding::new("up", MoveUp, Some(CONTEXT)),
        KeyBinding::new("down", MoveDown, Some(CONTEXT)),
        KeyBinding::new("shift-left", SelectLeft, Some(CONTEXT)),
        KeyBinding::new("shift-right", SelectRight, Some(CONTEXT)),
        KeyBinding::new("shift-up", SelectUp, Some(CONTEXT)),
        KeyBinding::new("shift-down", SelectDown, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-a", SelectAll, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-a", SelectAll, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-c", Copy, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-c", Copy, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-x", Cut, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-x", Cut, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-v", Paste, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-v", Paste, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-z", Undo, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-z", Undo, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-z", Redo, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-z", Redo, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-b", ToggleBold, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-b", ToggleBold, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-i", ToggleItalic, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-i", ToggleItalic, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-u", ToggleUnderline, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-u", ToggleUnderline, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-x", ToggleStrikethrough, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-x", ToggleStrikethrough, Some(CONTEXT)),
    ]);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockTextSize {
    Small,
    Normal,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph,
    Heading { level: u8 },
    UnorderedListItem,
    OrderedListItem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockFormat {
    pub kind: BlockKind,
    pub size: BlockTextSize,
}

impl Default for BlockFormat {
    fn default() -> Self {
        Self {
            kind: BlockKind::Paragraph,
            size: BlockTextSize::Normal,
        }
    }
}

impl BlockFormat {
    fn split_successor(self) -> Self {
        match self.kind {
            BlockKind::Heading { .. } => BlockFormat::default(),
            BlockKind::Paragraph | BlockKind::UnorderedListItem | BlockKind::OrderedListItem => {
                self
            }
        }
    }
}

#[derive(Clone)]
struct Snapshot {
    text: Rope,
    styles: StyleRuns,
    blocks: Vec<BlockFormat>,
    selection: Selection,
    active_style: InlineStyle,
}

#[derive(Clone)]
pub(crate) struct LineLayoutCache {
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) start_offset: usize,
    pub(crate) line_len: usize,
    pub(crate) text_layout: gpui::TextLayout,
}

pub struct RichTextState {
    pub(crate) focus_handle: FocusHandle,
    pub(crate) scroll_handle: ScrollHandle,
    pub(crate) theme: RichTextTheme,

    pub(crate) text: Rope,
    pub(crate) blocks: Vec<BlockFormat>,
    pub(crate) styles: StyleRuns,
    pub(crate) active_style: InlineStyle,

    /// Selection in UTF-8 byte offsets (anchor..focus).
    pub(crate) selection: Selection,
    pub(crate) ime_marked_range: Option<Selection>,
    pub(crate) selecting: bool,
    pub(crate) preferred_x: Option<Pixels>,

    pub(crate) viewport_bounds: Bounds<Pixels>,
    pub(crate) layout_cache: Vec<Option<LineLayoutCache>>,

    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
}

impl RichTextState {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle().tab_stop(true);

        let text = Rope::new();
        let blocks = vec![BlockFormat::default()];
        let styles = StyleRuns::new(text.len());

        Self {
            focus_handle,
            scroll_handle: ScrollHandle::new(),
            theme: RichTextTheme::default(),
            text,
            blocks,
            styles,
            active_style: InlineStyle::default(),
            selection: Selection::default(),
            ime_marked_range: None,
            selecting: false,
            preferred_x: None,
            viewport_bounds: Bounds::default(),
            layout_cache: vec![None],
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn theme(mut self, theme: RichTextTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        let value: SharedString = value.into();
        self.text = Rope::from(value.as_str());
        self.selection = Selection::default();
        self.ime_marked_range = None;
        self.styles = StyleRuns::new(self.text.len());
        self.active_style = InlineStyle::default();
        self.sync_blocks_to_text();
        self
    }

    pub fn value(&self) -> SharedString {
        SharedString::new(self.text.to_string())
    }

    pub fn set_value(
        &mut self,
        value: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.push_undo_snapshot();
        let value: SharedString = value.into();
        self.text = Rope::from(value.as_str());
        self.styles = StyleRuns::new(self.text.len());
        self.blocks = vec![BlockFormat::default(); self.text.lines_len().max(1)];
        self.selection = Selection::default();
        self.active_style = InlineStyle::default();
        self.ime_marked_range = None;
        self.layout_cache = vec![None; self.text.lines_len().max(1)];
        self.scroll_handle.set_offset(point(px(0.), px(0.)));
        self.preferred_x = None;
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub(crate) fn cursor(&self) -> usize {
        self.ime_marked_range
            .map(|r| r.end)
            .unwrap_or(self.selection.end)
    }

    pub(crate) fn ordered_selection(&self) -> Range<usize> {
        let mut start = self.selection.start;
        let mut end = self.selection.end;
        if end < start {
            std::mem::swap(&mut start, &mut end);
        }
        start..end
    }

    pub(crate) fn has_selection(&self) -> bool {
        self.selection.start != self.selection.end
    }

    pub(crate) fn line_range(&self, row: usize) -> Range<usize> {
        let start = self.text.line_start_offset(row);
        let end = self.text.line_end_offset(row);
        start..end
    }

    pub(crate) fn set_selection(&mut self, anchor: usize, focus: usize) {
        self.selection.start = anchor.min(self.text.len());
        self.selection.end = focus.min(self.text.len());
    }

    pub(crate) fn set_cursor(&mut self, offset: usize) {
        let offset = offset.min(self.text.len());
        self.selection = (offset..offset).into();
        self.preferred_x = None;
        self.active_style = self.styles.style_at(offset);
    }

    fn sync_blocks_to_text(&mut self) {
        let lines = self.text.lines_len().max(1);
        if self.blocks.len() < lines {
            self.blocks
                .extend(std::iter::repeat(BlockFormat::default()).take(lines - self.blocks.len()));
        } else if self.blocks.len() > lines {
            self.blocks.truncate(lines);
        }

        if self.layout_cache.len() < lines {
            self.layout_cache
                .extend(std::iter::repeat(None).take(lines - self.layout_cache.len()));
        } else if self.layout_cache.len() > lines {
            self.layout_cache.truncate(lines);
        }

        self.styles.set_total_len(self.text.len());
    }

    fn push_undo_snapshot(&mut self) {
        self.undo_stack.push(Snapshot {
            text: self.text.clone(),
            styles: self.styles.clone(),
            blocks: self.blocks.clone(),
            selection: self.selection,
            active_style: self.active_style.clone(),
        });
        self.redo_stack.clear();
        if self.undo_stack.len() > 200 {
            self.undo_stack.remove(0);
        }
    }

    fn restore_snapshot(&mut self, snapshot: Snapshot) {
        self.text = snapshot.text;
        self.styles = snapshot.styles;
        self.blocks = snapshot.blocks;
        self.selection = snapshot.selection;
        self.active_style = snapshot.active_style;
        self.ime_marked_range = None;
        self.preferred_x = None;
        self.sync_blocks_to_text();
    }

    fn replace_bytes_range(
        &mut self,
        range: Range<usize>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        push_undo: bool,
        mark_ime: bool,
        new_selected_utf16: Option<Range<usize>>,
    ) {
        if push_undo {
            self.push_undo_snapshot();
        }

        let range = self.text.clip_offset(range.start, Bias::Left)
            ..self.text.clip_offset(range.end, Bias::Right);
        let removed_text = self.text.slice(range.clone()).to_string();
        let removed_newlines = removed_text
            .as_bytes()
            .iter()
            .filter(|b| **b == b'\n')
            .count();
        let inserted_newlines = new_text.as_bytes().iter().filter(|b| **b == b'\n').count();

        let start_row = self.text.offset_to_point(range.start).row;
        let start_block = self.blocks.get(start_row).copied().unwrap_or_default();

        self.text.replace(range.clone(), new_text);

        self.styles.delete_range(range.clone());
        if !new_text.is_empty() {
            self.styles
                .insert_range(range.start, new_text.len(), self.active_style.clone());
        }

        // Update blocks based on newline count delta.
        for _ in 0..removed_newlines {
            if start_row + 1 < self.blocks.len() {
                self.blocks.remove(start_row + 1);
                if start_row + 1 < self.layout_cache.len() {
                    self.layout_cache.remove(start_row + 1);
                }
            }
        }
        let new_block = start_block.split_successor();
        for _ in 0..inserted_newlines {
            self.blocks.insert(start_row + 1, new_block);
            self.layout_cache.insert(start_row + 1, None);
        }
        self.sync_blocks_to_text();

        let new_cursor = (range.start + new_text.len()).min(self.text.len());
        if mark_ime {
            if new_text.is_empty() {
                self.ime_marked_range = None;
                self.set_cursor(range.start);
            } else {
                self.ime_marked_range = Some((range.start..range.start + new_text.len()).into());
                if let Some(new_selected_utf16) = new_selected_utf16 {
                    let new_sel = self.range_from_utf16(&new_selected_utf16);
                    self.set_selection(range.start + new_sel.start, range.start + new_sel.end);
                } else {
                    self.set_cursor(range.start + new_text.len());
                }
            }
        } else {
            self.ime_marked_range = None;
            self.set_cursor(new_cursor);
        }

        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub(crate) fn scroll_cursor_into_view(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let cursor = self.cursor();
        let caret_bounds = self.caret_bounds_for_offset(cursor);
        let Some(caret_bounds) = caret_bounds else {
            return;
        };

        let mut offset = self.scroll_handle.offset();
        let viewport = self.viewport_bounds;

        let margin = px(12.);
        if caret_bounds.top() < viewport.top() + margin {
            offset.y += (viewport.top() + margin) - caret_bounds.top();
        } else if caret_bounds.bottom() > viewport.bottom() - margin {
            offset.y -= caret_bounds.bottom() - (viewport.bottom() - margin);
        }

        if caret_bounds.left() < viewport.left() + margin {
            offset.x += (viewport.left() + margin) - caret_bounds.left();
        } else if caret_bounds.right() > viewport.right() - margin {
            offset.x -= caret_bounds.right() - (viewport.right() - margin);
        }

        self.scroll_handle.set_offset(offset);
        cx.notify();
    }

    fn caret_bounds_for_offset(&self, offset: usize) -> Option<Bounds<Pixels>> {
        let cursor_point = self.text.offset_to_point(offset);
        let row = cursor_point.row;
        let col = cursor_point.column;

        let cache = self.layout_cache.get(row)?.as_ref()?;
        let pos = cache
            .text_layout
            .position_for_index(col)
            .or_else(|| cache.text_layout.position_for_index(cache.line_len))?;
        let line_height = cache.text_layout.line_height();

        Some(Bounds::from_corners(
            pos,
            point(pos.x + px(1.5), pos.y + line_height),
        ))
    }

    fn is_attr_enabled(&self, range: Range<usize>, get: impl Fn(&InlineStyle) -> bool) -> bool {
        if range.is_empty() {
            return get(&self.active_style);
        }
        self.styles
            .iter_runs_in_range(range)
            .all(|(_, style)| get(style))
    }

    fn toggle_attr(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        get: impl Fn(&InlineStyle) -> bool + std::marker::Copy,
        set: impl Fn(&mut InlineStyle, bool) + std::marker::Copy,
    ) {
        let range = self.ordered_selection();
        if range.is_empty() {
            let enabled = get(&self.active_style);
            set(&mut self.active_style, !enabled);
            cx.notify();
            return;
        }

        self.push_undo_snapshot();
        let enabled = self.is_attr_enabled(range.clone(), get);
        self.styles
            .update_range(range, |style| set(style, !enabled));
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn toggle_bold_mark(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_attr(window, cx, |s| s.bold, |s, v| s.bold = v);
    }

    pub fn toggle_italic_mark(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_attr(window, cx, |s| s.italic, |s, v| s.italic = v);
    }

    pub fn toggle_underline_mark(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_attr(window, cx, |s| s.underline, |s, v| s.underline = v);
    }

    pub fn toggle_strikethrough_mark(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_attr(window, cx, |s| s.strikethrough, |s, v| s.strikethrough = v);
    }

    pub fn set_block_kind(&mut self, kind: BlockKind, window: &mut Window, cx: &mut Context<Self>) {
        self.push_undo_snapshot();
        let (start_row, end_row) = self.selected_rows();
        for row in start_row..=end_row {
            if let Some(block) = self.blocks.get_mut(row) {
                block.kind = kind;
            }
        }
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn toggle_list(&mut self, kind: BlockKind, window: &mut Window, cx: &mut Context<Self>) {
        self.push_undo_snapshot();
        let (start_row, end_row) = self.selected_rows();
        let all_match =
            (start_row..=end_row).all(|row| self.blocks.get(row).is_some_and(|b| b.kind == kind));
        for row in start_row..=end_row {
            if let Some(block) = self.blocks.get_mut(row) {
                block.kind = if all_match {
                    BlockKind::Paragraph
                } else {
                    kind
                };
            }
        }
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn set_block_size(
        &mut self,
        size: BlockTextSize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.push_undo_snapshot();
        let (start_row, end_row) = self.selected_rows();
        for row in start_row..=end_row {
            if let Some(block) = self.blocks.get_mut(row) {
                block.size = size;
            }
        }
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn set_text_color(
        &mut self,
        color: Option<gpui::Hsla>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.ordered_selection();
        if range.is_empty() {
            self.active_style.fg = color;
            cx.notify();
            return;
        }

        self.push_undo_snapshot();
        self.styles.update_range(range, |style| style.fg = color);
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn set_highlight_color(
        &mut self,
        color: Option<gpui::Hsla>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.ordered_selection();
        if range.is_empty() {
            self.active_style.bg = color;
            cx.notify();
            return;
        }

        self.push_undo_snapshot();
        self.styles.update_range(range, |style| style.bg = color);
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    fn selected_rows(&self) -> (usize, usize) {
        if !self.has_selection() {
            let row = self.text.offset_to_point(self.cursor()).row;
            return (row, row);
        }

        let range = self.ordered_selection();
        let mut start_row = self.text.offset_to_point(range.start).row;
        let mut end_row = self.text.offset_to_point(range.end).row;

        if range.end > 0 {
            if let Some(ch) = self.text.char_at(range.end.saturating_sub(1)) {
                if ch == '\n' && end_row > 0 {
                    end_row = end_row.saturating_sub(1);
                }
            }
        }

        start_row = start_row.min(self.blocks.len().saturating_sub(1));
        end_row = end_row.min(self.blocks.len().saturating_sub(1));
        if end_row < start_row {
            std::mem::swap(&mut start_row, &mut end_row);
        }
        (start_row, end_row)
    }

    pub(crate) fn highlight_styles_for_line(
        &self,
        row: usize,
        base_color: gpui::Hsla,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let line_range = self.line_range(row);
        let line_start = line_range.start;
        let line_end = line_range.end;

        let mut highlights: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
        for (range, style) in self.styles.iter_runs_in_range(line_start..line_end) {
            let local = (range.start - line_start)..(range.end - line_start);
            let mut highlight = HighlightStyle::default();

            if style.bold {
                highlight.font_weight = Some(FontWeight::BOLD);
            }
            if style.italic {
                highlight.font_style = Some(FontStyle::Italic);
            }
            if style.underline {
                highlight.underline = Some(UnderlineStyle {
                    thickness: px(1.),
                    color: style.fg,
                    wavy: false,
                });
            }
            if style.strikethrough {
                highlight.strikethrough = Some(StrikethroughStyle {
                    thickness: px(1.),
                    color: style.fg,
                    ..Default::default()
                });
            }
            if let Some(color) = style.fg {
                highlight.color = Some(color);
            }
            if let Some(bg) = style.bg {
                highlight.background_color = Some(bg);
            }

            if highlight != HighlightStyle::default() {
                highlights.push((local, highlight));
            }
        }

        if let Some(marked) = self.ime_marked_range {
            let marked = marked.start..marked.end;
            let marked = marked.start.max(line_start)..marked.end.min(line_end);
            if marked.start < marked.end {
                let local = (marked.start - line_start)..(marked.end - line_start);
                let mark = vec![(
                    local,
                    HighlightStyle {
                        underline: Some(UnderlineStyle {
                            thickness: px(1.),
                            color: Some(base_color),
                            wavy: false,
                        }),
                        ..Default::default()
                    },
                )];
                highlights = gpui::combine_highlights(highlights, mark).collect();
            }
        }

        highlights
    }

    pub(crate) fn offset_for_point(&self, point: Point<Pixels>) -> Option<usize> {
        // Find the closest row by y distance.
        let mut best: Option<(usize, Pixels)> = None;
        for (row, cache) in self.layout_cache.iter().enumerate() {
            let Some(cache) = cache.as_ref() else {
                continue;
            };
            let dy = if point.y < cache.bounds.top() {
                cache.bounds.top() - point.y
            } else if point.y > cache.bounds.bottom() {
                point.y - cache.bounds.bottom()
            } else {
                px(0.)
            };

            if best.is_none() || dy < best.unwrap().1 {
                best = Some((row, dy));
            }
        }

        let (row, _) = best?;
        let cache = self.layout_cache.get(row)?.as_ref()?;

        let mut local_index = match cache.text_layout.index_for_position(point) {
            Ok(ix) | Err(ix) => ix,
        };
        local_index = local_index.min(cache.line_len);

        Some((cache.start_offset + local_index).min(self.text.len()))
    }

    pub(crate) fn move_vertically(&mut self, direction: i32) -> Option<usize> {
        let cursor = self.cursor();
        let caret_bounds = self.caret_bounds_for_offset(cursor)?;
        let x = self.preferred_x.unwrap_or(caret_bounds.left());

        let row = self.text.offset_to_point(cursor).row;
        let cache = self.layout_cache.get(row)?.as_ref()?;
        let line_height = cache.text_layout.line_height();

        let y = caret_bounds.top() + (direction as f32) * line_height;
        self.offset_for_point(point(x, y))
    }

    // --- Action handlers ---

    pub(super) fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            let range = self.ordered_selection();
            self.replace_bytes_range(range, "", window, cx, true, false, None);
            return;
        }

        let cursor = self.cursor();
        if cursor == 0 {
            return;
        }

        // Special: at start of line, downgrade heading/list.
        let pos = self.text.offset_to_point(cursor);
        if pos.column == 0 && pos.row < self.blocks.len() {
            let kind = self.blocks[pos.row].kind;
            if matches!(
                kind,
                BlockKind::Heading { .. }
                    | BlockKind::UnorderedListItem
                    | BlockKind::OrderedListItem
            ) {
                self.push_undo_snapshot();
                self.blocks[pos.row].kind = BlockKind::Paragraph;
                cx.notify();
                return;
            }
        }

        let start = self.previous_boundary(cursor);
        self.replace_bytes_range(start..cursor, "", window, cx, true, false, None);
    }

    pub(super) fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            let range = self.ordered_selection();
            self.replace_bytes_range(range, "", window, cx, true, false, None);
            return;
        }

        let cursor = self.cursor();
        if cursor >= self.text.len() {
            return;
        }

        let end = self.next_boundary(cursor);
        self.replace_bytes_range(cursor..end, "", window, cx, true, false, None);
    }

    pub(super) fn enter(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            let range = self.ordered_selection();
            self.replace_bytes_range(range, "", window, cx, true, false, None);
        }

        let cursor = self.cursor();
        let row = self.text.offset_to_point(cursor).row;

        // Exit list on empty list item.
        if row < self.blocks.len()
            && matches!(
                self.blocks[row].kind,
                BlockKind::UnorderedListItem | BlockKind::OrderedListItem
            )
            && self.text.slice_line(row).len() == 0
        {
            self.push_undo_snapshot();
            self.blocks[row].kind = BlockKind::Paragraph;
            cx.notify();
            return;
        }

        self.replace_bytes_range(cursor..cursor, "\n", window, cx, true, false, None);
    }

    pub(super) fn escape(&mut self, _: &Escape, _window: &mut Window, cx: &mut Context<Self>) {
        self.selecting = false;
        self.ime_marked_range = None;
        cx.notify();
    }

    pub(super) fn left(&mut self, _: &MoveLeft, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            let range = self.ordered_selection();
            self.set_cursor(range.start);
            cx.notify();
            self.scroll_cursor_into_view(window, cx);
            return;
        }

        let cursor = self.cursor();
        let new_cursor = self.previous_boundary(cursor);
        self.set_cursor(new_cursor);
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub(super) fn right(&mut self, _: &MoveRight, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            let range = self.ordered_selection();
            self.set_cursor(range.end);
            cx.notify();
            self.scroll_cursor_into_view(window, cx);
            return;
        }

        let cursor = self.cursor();
        let new_cursor = self.next_boundary(cursor);
        self.set_cursor(new_cursor);
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub(super) fn up(&mut self, _: &MoveUp, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            let range = self.ordered_selection();
            self.set_cursor(range.start);
        }

        if let Some(new_cursor) = self.move_vertically(-1) {
            self.set_cursor(new_cursor);
        }

        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub(super) fn down(&mut self, _: &MoveDown, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            let range = self.ordered_selection();
            self.set_cursor(range.end);
        }

        if let Some(new_cursor) = self.move_vertically(1) {
            self.set_cursor(new_cursor);
        }

        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub(super) fn select_left(
        &mut self,
        _: &SelectLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cursor = self.cursor();
        let new_cursor = self.previous_boundary(cursor);
        if !self.has_selection() {
            self.selection.start = cursor;
        }
        self.selection.end = new_cursor;
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub(super) fn select_right(
        &mut self,
        _: &SelectRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cursor = self.cursor();
        let new_cursor = self.next_boundary(cursor);
        if !self.has_selection() {
            self.selection.start = cursor;
        }
        self.selection.end = new_cursor;
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub(super) fn select_up(&mut self, _: &SelectUp, window: &mut Window, cx: &mut Context<Self>) {
        let cursor = self.cursor();
        if !self.has_selection() {
            self.selection.start = cursor;
        }
        if let Some(new_cursor) = self.move_vertically(-1) {
            self.selection.end = new_cursor;
        }
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub(super) fn select_down(
        &mut self,
        _: &SelectDown,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cursor = self.cursor();
        if !self.has_selection() {
            self.selection.start = cursor;
        }
        if let Some(new_cursor) = self.move_vertically(1) {
            self.selection.end = new_cursor;
        }
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub(super) fn select_all(
        &mut self,
        _: &SelectAll,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selection = (0..self.text.len()).into();
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub(super) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let range = self.ordered_selection();
        if range.is_empty() {
            return;
        }
        let selected_text = self.text.slice(range).to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(selected_text));
    }

    pub(super) fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        let range = self.ordered_selection();
        if range.is_empty() {
            return;
        }
        let selected_text = self.text.slice(range.clone()).to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(selected_text));
        self.replace_bytes_range(range, "", window, cx, true, false, None);
    }

    pub(super) fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        let Some(clipboard) = cx.read_from_clipboard() else {
            return;
        };
        let mut new_text = clipboard.text().unwrap_or_default();
        new_text = new_text.replace("\r\n", "\n").replace('\r', "\n");
        let range = self.ordered_selection();
        self.replace_bytes_range(range, &new_text, window, cx, true, false, None);
    }

    pub(super) fn undo(&mut self, _: &Undo, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = self.undo_stack.pop() else {
            return;
        };
        self.redo_stack.push(Snapshot {
            text: self.text.clone(),
            styles: self.styles.clone(),
            blocks: self.blocks.clone(),
            selection: self.selection,
            active_style: self.active_style.clone(),
        });
        self.restore_snapshot(snapshot);
        cx.notify();
    }

    pub(super) fn redo(&mut self, _: &Redo, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = self.redo_stack.pop() else {
            return;
        };
        self.undo_stack.push(Snapshot {
            text: self.text.clone(),
            styles: self.styles.clone(),
            blocks: self.blocks.clone(),
            selection: self.selection,
            active_style: self.active_style.clone(),
        });
        self.restore_snapshot(snapshot);
        cx.notify();
    }

    pub(super) fn toggle_bold(
        &mut self,
        _: &ToggleBold,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_bold_mark(window, cx);
    }

    pub(super) fn toggle_italic(
        &mut self,
        _: &ToggleItalic,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_italic_mark(window, cx);
    }

    pub(super) fn toggle_underline(
        &mut self,
        _: &ToggleUnderline,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_underline_mark(window, cx);
    }

    pub(super) fn toggle_strikethrough(
        &mut self,
        _: &ToggleStrikethrough,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_strikethrough_mark(window, cx);
    }

    // --- Utilities copied from InputState ---

    #[inline]
    pub(crate) fn offset_from_utf16(&self, offset: usize) -> usize {
        self.text.offset_utf16_to_offset(offset)
    }

    #[inline]
    pub(crate) fn offset_to_utf16(&self, offset: usize) -> usize {
        self.text.offset_to_offset_utf16(offset)
    }

    #[inline]
    pub(crate) fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    #[inline]
    pub(crate) fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    pub(crate) fn previous_boundary(&self, offset: usize) -> usize {
        let mut offset = self.text.clip_offset(offset.saturating_sub(1), Bias::Left);
        if let Some(ch) = self.text.char_at(offset) {
            if ch == '\r' {
                offset = offset.saturating_sub(1);
            }
        }
        offset
    }

    pub(crate) fn next_boundary(&self, offset: usize) -> usize {
        let mut offset = self.text.clip_offset(offset + 1, Bias::Right);
        if let Some(ch) = self.text.char_at(offset) {
            if ch == '\n' {
                offset += 1;
            } else if ch == '\r' {
                offset += 2;
            }
        }
        offset.min(self.text.len())
    }
}

impl EntityInputHandler for RichTextState {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        adjusted_range.replace(self.range_to_utf16(&range));
        Some(self.text.slice(range).to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let range = self.ordered_selection();
        Some(UTF16Selection {
            range: self.range_to_utf16(&range),
            reversed: self.selection.end < self.selection.start,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.ime_marked_range
            .map(|range| self.range_to_utf16(&(range.start..range.end)))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.ime_marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.ime_marked_range.map(|range| range.into()))
            .unwrap_or_else(|| self.ordered_selection());

        self.replace_bytes_range(range, new_text, window, cx, true, false, None);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.ime_marked_range.map(|range| range.into()))
            .unwrap_or_else(|| self.ordered_selection());

        self.replace_bytes_range(
            range,
            new_text,
            window,
            cx,
            false,
            true,
            new_selected_range_utf16,
        );
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        let start_point = self.text.offset_to_point(range.start);
        let row = start_point.row;
        let col = start_point.column;

        let cache = self.layout_cache.get(row)?.as_ref()?;
        let pos = cache
            .text_layout
            .position_for_index(col)
            .or_else(|| cache.text_layout.position_for_index(cache.line_len))?;
        let line_height = cache.text_layout.line_height();

        Some(Bounds::from_corners(pos, point(pos.x, pos.y + line_height)))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let offset = self.offset_for_point(point)?;
        Some(self.offset_to_utf16(offset))
    }
}

impl gpui::Focusable for RichTextState {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
