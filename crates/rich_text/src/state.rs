use std::{collections::HashMap, ops::Range};

use gpui::{
    Action, App, Bounds, ClipboardItem, Context, EntityInputHandler, FocusHandle, FontStyle,
    FontWeight, HighlightStyle, KeyBinding, Pixels, Point, ScrollHandle, SharedString,
    StrikethroughStyle, UTF16Selection, UnderlineStyle, Window, actions, point, px,
};
use ropey::Rope;
use serde::Deserialize;
use sum_tree::Bias;

use crate::RichTextTheme;
use crate::document::{
    BlockAlign, BlockKind, BlockNode, BlockTextSize, InlineNode, OrderedListStyle,
    RichTextDocument, TextNode,
};
use crate::rope_ext::RopeExt as _;
use crate::selection::Selection;
use crate::value::RichTextValue;

use super::style::InlineStyle;

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

#[derive(Clone)]
struct Snapshot {
    document: RichTextDocument,
    selection: Selection,
    active_style: InlineStyle,
}

#[derive(Clone, Debug)]
struct ColumnResize {
    group: u64,
    handle: usize,
    start_x: Pixels,
    start_weights: Vec<f32>,
}

#[derive(Clone)]
pub(crate) struct LineLayoutCache {
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) start_offset: usize,
    pub(crate) line_len: usize,
    pub(crate) text_layout: gpui::TextLayout,
}

#[derive(Clone, Debug)]
pub struct ActiveLinkOverlay {
    pub url: String,
    /// Anchor position in window coordinates.
    pub anchor: Point<Pixels>,
}

pub struct RichTextState {
    pub(crate) focus_handle: FocusHandle,
    pub(crate) scroll_handle: ScrollHandle,
    pub(crate) theme: RichTextTheme,
    read_only: bool,

    pub(crate) document: RichTextDocument,
    pub(crate) text: Rope,
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
    next_columns_group_id: u64,

    column_widths: HashMap<u64, Vec<f32>>,
    column_resize: Option<ColumnResize>,
}

impl RichTextState {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle().tab_stop(true);

        let document = RichTextDocument::default();
        let text = Rope::from(document.to_plain_text().as_str());
        let layout_cache = vec![None; document.blocks.len().max(1)];

        Self {
            focus_handle,
            scroll_handle: ScrollHandle::new(),
            theme: RichTextTheme::default(),
            read_only: false,
            document,
            text,
            active_style: InlineStyle::default(),
            selection: Selection::default(),
            ime_marked_range: None,
            selecting: false,
            preferred_x: None,
            viewport_bounds: Bounds::default(),
            layout_cache,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            next_columns_group_id: 1,
            column_widths: HashMap::new(),
            column_resize: None,
        }
    }

    pub fn theme(mut self, theme: RichTextTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn set_read_only(&mut self, read_only: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only == read_only {
            return;
        }
        self.read_only = read_only;
        if read_only {
            self.ime_marked_range = None;
            self.selecting = false;
        }
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn default_richtext_value(mut self, value: RichTextValue) -> Self {
        self.document = value.into_document();
        self.text = Rope::from(self.document.to_plain_text().as_str());
        self.selection = Selection::default();
        self.ime_marked_range = None;
        self.active_style = InlineStyle::default();
        self.layout_cache = vec![None; self.document.blocks.len().max(1)];
        self.column_widths.clear();
        self.column_resize = None;
        self.next_columns_group_id = self
            .document
            .blocks
            .iter()
            .filter_map(|block| block.format.columns.map(|cols| cols.group))
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        self
    }

    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        let value: SharedString = value.into();
        self.document = RichTextDocument::from_plain_text(value.as_str());
        self.text = Rope::from(self.document.to_plain_text().as_str());
        self.selection = Selection::default();
        self.ime_marked_range = None;
        self.active_style = InlineStyle::default();
        self.layout_cache = vec![None; self.document.blocks.len().max(1)];
        self.next_columns_group_id = 1;
        self.column_widths.clear();
        self.column_resize = None;
        self
    }

    pub fn value(&self) -> SharedString {
        SharedString::new(self.text.to_string())
    }

    pub fn richtext_value(&self) -> RichTextValue {
        RichTextValue::from_document(&self.document)
    }

    pub fn set_richtext_value(
        &mut self,
        value: RichTextValue,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.push_undo_snapshot();
        self.apply_loaded_richtext_value(value, true, window, cx);
    }

    pub fn load_richtext_value(
        &mut self,
        value: RichTextValue,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_loaded_richtext_value(value, false, window, cx);
    }

    pub fn set_value(
        &mut self,
        value: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.push_undo_snapshot();
        let value: SharedString = value.into();
        self.document = RichTextDocument::from_plain_text(value.as_str());
        self.text = Rope::from(self.document.to_plain_text().as_str());
        self.selection = Selection::default();
        self.active_style = InlineStyle::default();
        self.ime_marked_range = None;
        self.layout_cache = vec![None; self.document.blocks.len().max(1)];
        self.column_widths.clear();
        self.column_resize = None;
        self.scroll_handle.set_offset(point(px(0.), px(0.)));
        self.preferred_x = None;
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    fn apply_loaded_richtext_value(
        &mut self,
        value: RichTextValue,
        keep_undo: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.document = value.into_document();
        self.text = Rope::from(self.document.to_plain_text().as_str());
        self.selection = Selection::default();
        self.active_style = InlineStyle::default();
        self.ime_marked_range = None;
        self.selecting = false;
        self.preferred_x = None;
        self.layout_cache = vec![None; self.document.blocks.len().max(1)];
        self.column_widths.clear();
        self.column_resize = None;
        self.next_columns_group_id = self
            .document
            .blocks
            .iter()
            .filter_map(|block| block.format.columns.map(|cols| cols.group))
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        self.scroll_handle.set_offset(point(px(0.), px(0.)));

        if !keep_undo {
            self.undo_stack.clear();
            self.redo_stack.clear();
        }

        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub(crate) fn column_weights_for_group(&self, group: u64, count: u8) -> Vec<f32> {
        let count = count.max(2) as usize;
        self.column_widths
            .get(&group)
            .filter(|weights| weights.len() == count)
            .cloned()
            .unwrap_or_else(|| vec![1.0; count])
    }

    pub(crate) fn start_column_resize(
        &mut self,
        group: u64,
        count: u8,
        handle: usize,
        start_x: Pixels,
        cx: &mut Context<Self>,
    ) {
        let count = count.clamp(2, 4) as usize;
        if handle + 1 >= count {
            return;
        }

        let weights = self
            .column_widths
            .entry(group)
            .or_insert_with(|| vec![1.0; count]);

        if weights.len() != count {
            *weights = vec![1.0; count];
        }

        self.selecting = false;
        self.column_resize = Some(ColumnResize {
            group,
            handle,
            start_x,
            start_weights: weights.clone(),
        });
        cx.notify();
    }

    pub(crate) fn update_column_resize(&mut self, position_x: Pixels, cx: &mut Context<Self>) {
        let Some(resize) = self.column_resize.clone() else {
            return;
        };

        let count = resize.start_weights.len();
        if count < 2 || resize.handle + 1 >= count {
            return;
        }

        let viewport_width = self.viewport_bounds.size.width;
        if viewport_width <= px(0.) {
            return;
        }

        let gutter = px(12.);
        let available = viewport_width - gutter * (count.saturating_sub(1) as f32);
        if available <= px(0.) {
            return;
        }

        let total_weight: f32 = resize.start_weights.iter().copied().sum();
        if total_weight <= 0.0001 {
            return;
        }

        let left_ix = resize.handle;
        let right_ix = resize.handle + 1;

        let left_width = available * (resize.start_weights[left_ix] / total_weight);
        let right_width = available * (resize.start_weights[right_ix] / total_weight);

        let min_width = px(80.);
        let mut dx = position_x - resize.start_x;

        let min_dx = min_width - left_width;
        let max_dx = right_width - min_width;
        if dx < min_dx {
            dx = min_dx;
        } else if dx > max_dx {
            dx = max_dx;
        }

        let left_new = left_width + dx;
        let right_new = right_width - dx;
        if left_new < px(1.) || right_new < px(1.) {
            return;
        }

        let left_weight = (left_new / available) * total_weight;
        let right_weight = (right_new / available) * total_weight;

        let weights = self
            .column_widths
            .entry(resize.group)
            .or_insert_with(|| resize.start_weights.clone());
        if weights.len() != count {
            *weights = resize.start_weights.clone();
        }
        weights[left_ix] = left_weight.max(0.001);
        weights[right_ix] = right_weight.max(0.001);

        cx.notify();
    }

    pub(crate) fn stop_column_resize(&mut self, cx: &mut Context<Self>) {
        if self.column_resize.take().is_some() {
            cx.notify();
        }
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

    pub(crate) fn word_range(&self, offset: usize) -> Range<usize> {
        let len = self.text.len();
        if len == 0 {
            return 0..0;
        }

        let mut offset = offset.min(len);
        if offset == len {
            offset = len.saturating_sub(1);
        }
        offset = self.text.clip_offset(offset, Bias::Left);

        if matches!(self.text.char_at(offset), Some('\n' | '\r')) && offset > 0 {
            offset = self.text.clip_offset(offset.saturating_sub(1), Bias::Left);
        }

        let Some(ch) = self.text.char_at(offset) else {
            return offset..offset;
        };

        let is_word_char = |c: char| c == '_' || c.is_alphanumeric();
        let is_whitespace_char = |c: char| c.is_whitespace() && !matches!(c, '\n' | '\r');

        enum Group {
            Word,
            Whitespace,
            Other,
        }

        let group = if is_word_char(ch) {
            Group::Word
        } else if is_whitespace_char(ch) {
            Group::Whitespace
        } else {
            Group::Other
        };

        let matches_group = |c: char| match group {
            Group::Word => is_word_char(c),
            Group::Whitespace => is_whitespace_char(c),
            Group::Other => !is_word_char(c) && !is_whitespace_char(c) && !matches!(c, '\n' | '\r'),
        };

        let mut start = offset;
        while start > 0 {
            let prev = self.text.clip_offset(start.saturating_sub(1), Bias::Left);
            if prev == start {
                break;
            }
            let Some(prev_ch) = self.text.char_at(prev) else {
                break;
            };
            if matches_group(prev_ch) {
                start = prev;
            } else {
                break;
            }
        }

        let mut end = self.text.clip_offset(offset + 1, Bias::Right);
        while end < len {
            let Some(next_ch) = self.text.char_at(end) else {
                break;
            };
            if matches_group(next_ch) {
                end = self.text.clip_offset(end + 1, Bias::Right);
            } else {
                break;
            }
        }

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
        self.active_style = self.style_at(offset);
    }

    fn sync_layout_cache_to_document(&mut self) {
        let lines = self.document.blocks.len().max(1);
        if self.layout_cache.len() < lines {
            self.layout_cache
                .extend(std::iter::repeat(None).take(lines - self.layout_cache.len()));
        } else if self.layout_cache.len() > lines {
            self.layout_cache.truncate(lines);
        }
    }

    fn push_undo_snapshot(&mut self) {
        self.undo_stack.push(Snapshot {
            document: self.document.clone(),
            selection: self.selection,
            active_style: self.active_style.clone(),
        });
        self.redo_stack.clear();
        if self.undo_stack.len() > 200 {
            self.undo_stack.remove(0);
        }
    }

    fn restore_snapshot(&mut self, snapshot: Snapshot) {
        self.document = snapshot.document;
        self.text = Rope::from(self.document.to_plain_text().as_str());
        self.selection = snapshot.selection;
        self.active_style = snapshot.active_style;
        self.ime_marked_range = None;
        self.preferred_x = None;
        self.sync_layout_cache_to_document();
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
        if self.read_only {
            return;
        }
        if push_undo {
            self.push_undo_snapshot();
        }

        let range = self.text.clip_offset(range.start, Bias::Left)
            ..self.text.clip_offset(range.end, Bias::Right);

        let new_text = new_text.replace("\r\n", "\n").replace('\r', "\n");

        let removed_text = self.text.slice(range.clone()).to_string();
        let removed_newlines = removed_text
            .as_bytes()
            .iter()
            .filter(|b| **b == b'\n')
            .count();
        let inserted_newlines = new_text.as_bytes().iter().filter(|b| **b == b'\n').count();

        let start_point = self.text.offset_to_point(range.start);
        let end_point = self.text.offset_to_point(range.end);

        self.ensure_document_non_empty();

        let start_row = start_point
            .row
            .min(self.document.blocks.len().saturating_sub(1));
        let end_row = end_point
            .row
            .min(self.document.blocks.len().saturating_sub(1));

        let start_col = start_point
            .column
            .min(self.document.blocks[start_row].text_len());
        let end_col = end_point
            .column
            .min(self.document.blocks[end_row].text_len());

        self.delete_document_range(start_row, start_col, end_row, end_col);
        let active_style = self.active_style.clone();
        self.insert_plain_text(start_row, start_col, &new_text, &active_style);

        // Maintain layout cache length relative to newline changes for better scroll-to-caret.
        for _ in 0..removed_newlines {
            if start_row + 1 < self.layout_cache.len() {
                self.layout_cache.remove(start_row + 1);
            }
        }
        for _ in 0..inserted_newlines {
            self.layout_cache.insert(start_row + 1, None);
        }

        self.ensure_document_non_empty();
        self.text = Rope::from(self.document.to_plain_text().as_str());
        self.sync_layout_cache_to_document();

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

    fn ensure_document_non_empty(&mut self) {
        if self.document.blocks.is_empty() {
            self.document = RichTextDocument::default();
        }
        if self.layout_cache.is_empty() {
            self.layout_cache.push(None);
        }
    }

    fn style_at(&self, mut offset: usize) -> InlineStyle {
        let total_len = self.text.len();
        if total_len == 0 {
            return InlineStyle::default();
        }
        if offset >= total_len {
            offset = total_len.saturating_sub(1);
        }

        let cursor_point = self.text.offset_to_point(offset);
        let row = cursor_point
            .row
            .min(self.document.blocks.len().saturating_sub(1));
        let col = cursor_point.column;

        let Some(block) = self.document.blocks.get(row) else {
            return InlineStyle::default();
        };

        let block_len = block.text_len();
        if block_len == 0 {
            return block
                .inlines
                .iter()
                .find_map(|node| match node {
                    InlineNode::Text(text) => Some(text.style.clone()),
                })
                .unwrap_or_default();
        }

        let col = col.min(block_len);
        if col >= block_len {
            return block.last_style().cloned().unwrap_or_default();
        }

        let mut cursor = 0usize;
        for node in &block.inlines {
            match node {
                InlineNode::Text(text) => {
                    let end = cursor + text.text.len();
                    if col < end {
                        return text.style.clone();
                    }
                    cursor = end;
                }
            }
        }

        block.last_style().cloned().unwrap_or_default()
    }

    fn split_off_inlines_at(block: &mut BlockNode, offset: usize) -> Vec<InlineNode> {
        let offset = offset.min(block.text_len());
        let mut cursor = 0usize;

        for ix in 0..block.inlines.len() {
            let InlineNode::Text(text) = &block.inlines[ix];
            let len = text.text.len();

            if offset == cursor {
                return block.inlines.split_off(ix);
            }

            if offset < cursor + len {
                let rel = offset - cursor;
                let mut tail = block.inlines.split_off(ix);
                let mut head_text = match tail.remove(0) {
                    InlineNode::Text(text) => text,
                };

                let right_text = head_text.text.split_off(rel);
                let right_style = head_text.style.clone();
                block.inlines.push(InlineNode::Text(head_text));

                let mut suffix = vec![InlineNode::Text(TextNode {
                    text: right_text,
                    style: right_style,
                })];
                suffix.append(&mut tail);
                return suffix;
            }

            cursor += len;
        }

        Vec::new()
    }

    fn delete_in_block(&mut self, row: usize, range: Range<usize>) {
        let Some(block) = self.document.blocks.get_mut(row) else {
            return;
        };

        let mut start = range.start;
        let mut end = range.end;
        if end < start {
            std::mem::swap(&mut start, &mut end);
        }

        let block_len = block.text_len();
        start = start.min(block_len);
        end = end.min(block_len);
        if start >= end {
            return;
        }

        let tail = Self::split_off_inlines_at(block, end);
        let _ = Self::split_off_inlines_at(block, start);
        block.inlines.extend(tail);
        block.normalize();
    }

    fn delete_document_range(
        &mut self,
        start_row: usize,
        start_col: usize,
        end_row: usize,
        end_col: usize,
    ) {
        if self.document.blocks.is_empty() {
            return;
        }

        if start_row == end_row {
            self.delete_in_block(start_row, start_col..end_col);
            return;
        }

        let start_len = self
            .document
            .blocks
            .get(start_row)
            .map(|b| b.text_len())
            .unwrap_or(0);
        self.delete_in_block(start_row, start_col..start_len);
        self.delete_in_block(end_row, 0..end_col);

        for _ in (start_row + 1)..end_row {
            if start_row + 1 < self.document.blocks.len() {
                self.document.blocks.remove(start_row + 1);
            }
        }

        if start_row + 1 < self.document.blocks.len() {
            let end_block = self.document.blocks.remove(start_row + 1);
            if let Some(start_block) = self.document.blocks.get_mut(start_row) {
                start_block.inlines.extend(end_block.inlines);
                start_block.normalize();
            }
        }
    }

    fn insert_text_in_block(&mut self, row: usize, col: usize, text: &str, style: &InlineStyle) {
        if text.is_empty() {
            return;
        }
        let Some(block) = self.document.blocks.get_mut(row) else {
            return;
        };

        let col = col.min(block.text_len());
        let tail = Self::split_off_inlines_at(block, col);
        block.inlines.push(InlineNode::Text(TextNode {
            text: text.to_string(),
            style: style.clone(),
        }));
        block.inlines.extend(tail);
        block.normalize();
    }

    fn insert_plain_text(&mut self, row: usize, col: usize, text: &str, style: &InlineStyle) {
        if text.is_empty() {
            return;
        }

        if !text.contains('\n') {
            self.insert_text_in_block(row, col, text, style);
            return;
        }

        let successor_format = self
            .document
            .blocks
            .get(row)
            .map(|b| b.format.split_successor())
            .unwrap_or_default();

        let suffix = {
            let Some(block) = self.document.blocks.get_mut(row) else {
                return;
            };
            Self::split_off_inlines_at(block, col)
        };

        let mut parts = text.split('\n');
        let first = parts.next().unwrap_or_default();
        self.insert_text_in_block(row, col, first, style);

        let mut insert_at = row + 1;
        for part in parts {
            let mut block = BlockNode {
                format: successor_format,
                inlines: vec![InlineNode::Text(TextNode {
                    text: part.to_string(),
                    style: style.clone(),
                })],
            };
            block.normalize();
            self.document.blocks.insert(insert_at, block);
            insert_at += 1;
        }

        let last_row = insert_at.saturating_sub(1);
        if let Some(last) = self.document.blocks.get_mut(last_row) {
            last.inlines.extend(suffix);
            last.normalize();
        }
    }

    fn for_each_block_range_in_selection(
        &self,
        range: Range<usize>,
        mut f: impl FnMut(usize, Range<usize>),
    ) {
        if range.is_empty() || self.document.blocks.is_empty() {
            return;
        }

        let start_point = self.text.offset_to_point(range.start);
        let end_point = self.text.offset_to_point(range.end);

        let start_row = start_point
            .row
            .min(self.document.blocks.len().saturating_sub(1));
        let end_row = end_point
            .row
            .min(self.document.blocks.len().saturating_sub(1));

        for row in start_row..=end_row {
            let block_len = self.document.blocks[row].text_len();
            let local_start = if row == start_row {
                start_point.column.min(block_len)
            } else {
                0
            };
            let local_end = if row == end_row {
                end_point.column.min(block_len)
            } else {
                block_len
            };

            if local_start < local_end {
                f(row, local_start..local_end);
            }
        }
    }

    fn update_inline_styles_in_block_range(
        &mut self,
        row: usize,
        range: Range<usize>,
        mut update: impl FnMut(&mut InlineStyle),
    ) {
        let Some(block) = self.document.blocks.get_mut(row) else {
            return;
        };

        let mut start = range.start;
        let mut end = range.end;
        if end < start {
            std::mem::swap(&mut start, &mut end);
        }

        let block_len = block.text_len();
        start = start.min(block_len);
        end = end.min(block_len);
        if start >= end {
            return;
        }

        let tail = Self::split_off_inlines_at(block, end);
        let mut mid = Self::split_off_inlines_at(block, start);

        for node in &mut mid {
            match node {
                InlineNode::Text(text) => update(&mut text.style),
            }
        }

        block.inlines.extend(mid);
        block.inlines.extend(tail);
        block.normalize();
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

    fn align_offset_x_for_wrap_line(
        cache: &LineLayoutCache,
        wrap_line_ix: usize,
        align: BlockAlign,
    ) -> Pixels {
        let align_width = cache.bounds.size.width;
        if align_width <= px(0.) {
            return px(0.);
        }

        let Some(layout) = cache.text_layout.line_layout_for_index(0) else {
            return px(0.);
        };

        let segments = layout.wrap_boundaries().len().saturating_add(1);
        if segments == 0 {
            return px(0.);
        }
        let wrap_line_ix = wrap_line_ix.min(segments - 1);

        let end_ix = if wrap_line_ix < layout.wrap_boundaries().len() {
            let boundary = layout.wrap_boundaries()[wrap_line_ix];
            layout.runs()[boundary.run_ix]
                .glyphs
                .get(boundary.glyph_ix)
                .map(|g| g.index)
                .unwrap_or(layout.len())
        } else {
            layout.len()
        };

        let line_height = cache.text_layout.line_height();
        let line_width = layout
            .position_for_index(end_ix, line_height)
            .map(|p| p.x)
            .unwrap_or(px(0.));

        match align {
            BlockAlign::Left => px(0.),
            BlockAlign::Center => (align_width - line_width) / 2.0,
            BlockAlign::Right => align_width - line_width,
        }
    }

    pub(crate) fn aligned_position_for_row_col(
        &self,
        row: usize,
        col: usize,
    ) -> Option<Point<Pixels>> {
        let cache = self.layout_cache.get(row)?.as_ref()?;
        let align = self
            .document
            .blocks
            .get(row)
            .map(|b| b.format.align)
            .unwrap_or(BlockAlign::Left);

        let col = col.min(cache.line_len);
        let base = cache
            .text_layout
            .position_for_index(col)
            .or_else(|| cache.text_layout.position_for_index(cache.line_len))?;

        if align == BlockAlign::Left {
            return Some(base);
        }

        let line_height = cache.text_layout.line_height();
        let wrap_line_ix =
            ((base.y - cache.bounds.origin.y).max(px(0.0)) / line_height).floor() as usize;
        let dx = Self::align_offset_x_for_wrap_line(cache, wrap_line_ix, align);
        Some(point(base.x + dx, base.y))
    }

    pub(crate) fn aligned_index_for_row_point(&self, row: usize, position: Point<Pixels>) -> usize {
        let Some(cache) = self.layout_cache.get(row).and_then(|c| c.as_ref()) else {
            return 0;
        };

        let align = self
            .document
            .blocks
            .get(row)
            .map(|b| b.format.align)
            .unwrap_or(BlockAlign::Left);

        let adjusted_point = if align == BlockAlign::Left {
            position
        } else {
            let line_height = cache.text_layout.line_height();
            let wrap_line_ix =
                ((position.y - cache.bounds.origin.y).max(px(0.0)) / line_height).floor() as usize;
            let dx = Self::align_offset_x_for_wrap_line(cache, wrap_line_ix, align);
            point(position.x - dx, position.y)
        };

        let mut local_index = match cache.text_layout.index_for_position(adjusted_point) {
            Ok(ix) | Err(ix) => ix,
        };
        local_index = local_index.min(cache.line_len);
        local_index
    }

    fn caret_bounds_for_offset(&self, offset: usize) -> Option<Bounds<Pixels>> {
        let cursor_point = self.text.offset_to_point(offset);
        let row = cursor_point.row;
        let col = cursor_point.column;

        let cache = self.layout_cache.get(row)?.as_ref()?;
        let pos = self.aligned_position_for_row_col(row, col)?;
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

        let mut enabled = true;
        self.for_each_block_range_in_selection(range, |row, local_range| {
            if !enabled {
                return;
            }
            let mut cursor = 0usize;
            for node in &self.document.blocks[row].inlines {
                match node {
                    InlineNode::Text(text) => {
                        let end = cursor + text.text.len();
                        let overlap_start = local_range.start.max(cursor);
                        let overlap_end = local_range.end.min(end);
                        if overlap_start < overlap_end && !get(&text.style) {
                            enabled = false;
                            return;
                        }
                        cursor = end;
                    }
                }
            }
        });

        enabled
    }

    fn active_color_in_range(
        &self,
        range: Range<usize>,
        get: impl Fn(&InlineStyle) -> Option<gpui::Hsla>,
    ) -> Option<gpui::Hsla> {
        if range.is_empty() {
            return get(&self.active_style);
        }

        let mut value: Option<Option<gpui::Hsla>> = None;
        let mut mixed = false;
        self.for_each_block_range_in_selection(range, |row, local_range| {
            if mixed {
                return;
            }

            let mut cursor = 0usize;
            for node in &self.document.blocks[row].inlines {
                match node {
                    InlineNode::Text(text) => {
                        let end = cursor + text.text.len();
                        let overlap_start = local_range.start.max(cursor);
                        let overlap_end = local_range.end.min(end);
                        if overlap_start < overlap_end {
                            let next = get(&text.style);
                            match value {
                                None => value = Some(next),
                                Some(prev) if prev == next => {}
                                Some(_) => {
                                    mixed = true;
                                    return;
                                }
                            }
                        }
                        cursor = end;
                    }
                }
            }
        });

        if mixed { None } else { value.flatten() }
    }

    fn toggle_attr(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        get: impl Fn(&InlineStyle) -> bool + std::marker::Copy,
        set: impl Fn(&mut InlineStyle, bool) + std::marker::Copy,
    ) {
        if self.read_only {
            return;
        }
        let range = self.ordered_selection();
        if range.is_empty() {
            let enabled = get(&self.active_style);
            set(&mut self.active_style, !enabled);
            cx.notify();
            return;
        }

        self.push_undo_snapshot();
        let enabled = self.is_attr_enabled(range.clone(), get);
        let mut block_ranges: Vec<(usize, Range<usize>)> = Vec::new();
        self.for_each_block_range_in_selection(range, |row, local_range| {
            block_ranges.push((row, local_range));
        });
        for (row, local_range) in block_ranges {
            self.update_inline_styles_in_block_range(row, local_range, |style| {
                set(style, !enabled);
            });
        }
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn bold_mark_active(&self) -> bool {
        self.is_attr_enabled(self.ordered_selection(), |s| s.bold)
    }

    pub fn italic_mark_active(&self) -> bool {
        self.is_attr_enabled(self.ordered_selection(), |s| s.italic)
    }

    pub fn underline_mark_active(&self) -> bool {
        self.is_attr_enabled(self.ordered_selection(), |s| s.underline)
    }

    pub fn strikethrough_mark_active(&self) -> bool {
        self.is_attr_enabled(self.ordered_selection(), |s| s.strikethrough)
    }

    pub fn code_mark_active(&self) -> bool {
        self.is_attr_enabled(self.ordered_selection(), |s| s.code)
    }

    pub fn active_text_color(&self) -> Option<gpui::Hsla> {
        self.active_color_in_range(self.ordered_selection(), |s| s.fg)
    }

    pub fn active_highlight_color(&self) -> Option<gpui::Hsla> {
        self.active_color_in_range(self.ordered_selection(), |s| s.bg)
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

    pub fn toggle_code_mark(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_attr(window, cx, |s| s.code, |s, v| s.code = v);
    }

    pub fn set_block_kind(&mut self, kind: BlockKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        self.push_undo_snapshot();
        let (start_row, end_row) = self.selected_rows();
        for row in start_row..=end_row {
            if let Some(block) = self.document.blocks.get_mut(row) {
                block.format.kind = kind;
            }
        }
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn indent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        const MAX_INDENT: u8 = 8;

        self.push_undo_snapshot();
        let (start_row, end_row) = self.selected_rows();
        for row in start_row..=end_row {
            if let Some(block) = self.document.blocks.get_mut(row) {
                block.format.indent = block.format.indent.saturating_add(1).min(MAX_INDENT);
            }
        }
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn outdent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        self.push_undo_snapshot();
        let (start_row, end_row) = self.selected_rows();
        for row in start_row..=end_row {
            if let Some(block) = self.document.blocks.get_mut(row) {
                block.format.indent = block.format.indent.saturating_sub(1);
            }
        }
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn set_columns(&mut self, count: u8, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }

        let count = count.clamp(2, 4);
        let Some(_) = self.document.blocks.first() else {
            return;
        };

        let group = self.next_columns_group_id;
        self.next_columns_group_id = self.next_columns_group_id.saturating_add(1);

        self.push_undo_snapshot();
        let (start_row, end_row) = self.selected_rows();
        let mut present = vec![false; count as usize];

        for (ix, row) in (start_row..=end_row).enumerate() {
            let Some(block) = self.document.blocks.get_mut(row) else {
                continue;
            };

            let column = (ix % count as usize) as u8;
            block.format.columns = Some(crate::document::BlockColumns {
                group,
                count,
                column,
            });
            present[column as usize] = true;
        }

        // Ensure each column has at least one block by inserting empty paragraphs.
        let base_format = self
            .document
            .blocks
            .get(start_row)
            .map(|b| b.format)
            .unwrap_or_default();

        let mut insert_at = (end_row + 1).min(self.document.blocks.len());
        for column in 0..count {
            if present.get(column as usize).copied().unwrap_or(false) {
                continue;
            }

            let mut format = base_format;
            format.kind = BlockKind::Paragraph;
            format.columns = Some(crate::document::BlockColumns {
                group,
                count,
                column,
            });

            self.document
                .blocks
                .insert(insert_at, BlockNode::from_text(format, ""));
            insert_at += 1;
        }

        // Rebuild the rope if we inserted blocks.
        if insert_at != (end_row + 1).min(self.document.blocks.len()) {
            self.text = Rope::from(self.document.to_plain_text().as_str());
            self.layout_cache = vec![None; self.document.blocks.len().max(1)];
        }

        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn active_block_kind(&self) -> Option<BlockKind> {
        let (start_row, end_row) = self.selected_rows();
        let first = self.document.blocks.get(start_row)?;
        let normalize = |kind: BlockKind| match kind {
            BlockKind::Todo { .. } => BlockKind::Todo { checked: false },
            BlockKind::Toggle { .. } => BlockKind::Toggle { collapsed: false },
            kind => kind,
        };
        let kind = normalize(first.format.kind);

        for row in start_row..=end_row {
            let Some(block) = self.document.blocks.get(row) else {
                continue;
            };
            if normalize(block.format.kind) != kind {
                return None;
            }
        }

        Some(kind)
    }

    pub fn active_columns_count(&self) -> Option<u8> {
        let (start_row, end_row) = self.selected_rows();
        let first = self.document.blocks.get(start_row)?.format.columns?;
        for row in start_row..=end_row {
            let cols = self.document.blocks.get(row)?.format.columns?;
            if cols.group != first.group || cols.count != first.count {
                return None;
            }
        }
        Some(first.count)
    }

    pub fn toggle_list(&mut self, kind: BlockKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        self.push_undo_snapshot();
        let (start_row, end_row) = self.selected_rows();
        let all_match = (start_row..=end_row).all(|row| {
            self.document
                .blocks
                .get(row)
                .is_some_and(|b| b.format.kind == kind)
        });
        for row in start_row..=end_row {
            if let Some(block) = self.document.blocks.get_mut(row) {
                block.format.kind = if all_match {
                    BlockKind::Paragraph
                } else {
                    kind
                };
            }
        }
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn active_ordered_list_style(&self) -> Option<OrderedListStyle> {
        let (start_row, end_row) = self.selected_rows();
        let first = self.document.blocks.get(start_row)?;
        if first.format.kind != BlockKind::OrderedListItem {
            return None;
        }

        let style = first.format.ordered_list_style;
        for row in start_row..=end_row {
            let Some(block) = self.document.blocks.get(row) else {
                continue;
            };
            if block.format.kind != BlockKind::OrderedListItem
                || block.format.ordered_list_style != style
            {
                return None;
            }
        }

        Some(style)
    }

    pub fn set_ordered_list_style(
        &mut self,
        style: OrderedListStyle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        self.push_undo_snapshot();
        let (start_row, end_row) = self.selected_rows();

        for row in start_row..=end_row {
            if let Some(block) = self.document.blocks.get_mut(row) {
                block.format.kind = BlockKind::OrderedListItem;
                block.format.ordered_list_style = style;
            }
        }

        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn toggle_ordered_list_any(
        &mut self,
        style: OrderedListStyle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        self.push_undo_snapshot();
        let (start_row, end_row) = self.selected_rows();

        let all_ordered = (start_row..=end_row).all(|row| {
            self.document
                .blocks
                .get(row)
                .is_some_and(|b| b.format.kind == BlockKind::OrderedListItem)
        });

        for row in start_row..=end_row {
            if let Some(block) = self.document.blocks.get_mut(row) {
                if all_ordered {
                    block.format.kind = BlockKind::Paragraph;
                } else {
                    block.format.kind = BlockKind::OrderedListItem;
                    block.format.ordered_list_style = style;
                }
            }
        }

        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn toggle_ordered_list(
        &mut self,
        style: OrderedListStyle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        self.push_undo_snapshot();
        let (start_row, end_row) = self.selected_rows();

        let all_match = (start_row..=end_row).all(|row| {
            self.document.blocks.get(row).is_some_and(|b| {
                b.format.kind == BlockKind::OrderedListItem && b.format.ordered_list_style == style
            })
        });

        for row in start_row..=end_row {
            if let Some(block) = self.document.blocks.get_mut(row) {
                if all_match {
                    block.format.kind = BlockKind::Paragraph;
                } else {
                    block.format.kind = BlockKind::OrderedListItem;
                    block.format.ordered_list_style = style;
                }
            }
        }

        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn toggle_todo_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        self.push_undo_snapshot();
        let (start_row, end_row) = self.selected_rows();
        let all_todo = (start_row..=end_row).all(|row| {
            self.document
                .blocks
                .get(row)
                .is_some_and(|b| matches!(b.format.kind, BlockKind::Todo { .. }))
        });

        for row in start_row..=end_row {
            if let Some(block) = self.document.blocks.get_mut(row) {
                block.format.kind = if all_todo {
                    BlockKind::Paragraph
                } else {
                    BlockKind::Todo { checked: false }
                };
            }
        }

        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub(crate) fn toggle_todo_checked(
        &mut self,
        row: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let checked = match self.document.blocks.get(row).map(|b| b.format.kind) {
            Some(BlockKind::Todo { checked }) => checked,
            _ => return,
        };

        self.push_undo_snapshot();

        let Some(block) = self.document.blocks.get_mut(row) else {
            return;
        };
        block.format.kind = BlockKind::Todo { checked: !checked };
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub(crate) fn toggle_toggle_collapsed(
        &mut self,
        row: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let collapsed = match self.document.blocks.get(row).map(|b| b.format.kind) {
            Some(BlockKind::Toggle { collapsed }) => collapsed,
            _ => return,
        };

        self.push_undo_snapshot();
        let Some(block) = self.document.blocks.get_mut(row) else {
            return;
        };

        block.format.kind = BlockKind::Toggle {
            collapsed: !collapsed,
        };
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn insert_divider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        self.push_undo_snapshot();

        let cursor = self.cursor();
        let pos = self.text.offset_to_point(cursor);
        let mut row = pos.row.min(self.document.blocks.len().saturating_sub(1));
        if self.document.blocks.is_empty() {
            self.document = RichTextDocument::default();
            row = 0;
        }

        let replace_current =
            self.document.blocks.get(row).is_some_and(|b| {
                b.is_text_empty() && matches!(b.format.kind, BlockKind::Paragraph)
            });

        if replace_current {
            if let Some(block) = self.document.blocks.get_mut(row) {
                block.format.kind = BlockKind::Divider;
            }
        } else {
            row = row.saturating_add(1);
            self.document.blocks.insert(row, BlockNode::default());
            if let Some(block) = self.document.blocks.get_mut(row) {
                block.format.kind = BlockKind::Divider;
            }
        }

        // Ensure a paragraph after the divider and place the cursor there.
        let insert_row = (row + 1).min(self.document.blocks.len());
        self.document
            .blocks
            .insert(insert_row, BlockNode::default());

        self.text = Rope::from(self.document.to_plain_text().as_str());
        self.layout_cache = vec![None; self.document.blocks.len().max(1)];
        self.selection = Selection::default();
        self.ime_marked_range = None;
        self.active_style = InlineStyle::default();
        self.preferred_x = None;

        let offset = self.text.line_start_offset(insert_row);
        self.set_cursor(offset);
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn set_block_size(
        &mut self,
        size: BlockTextSize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        self.push_undo_snapshot();
        let (start_row, end_row) = self.selected_rows();
        for row in start_row..=end_row {
            if let Some(block) = self.document.blocks.get_mut(row) {
                block.format.size = size;
            }
        }
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn set_block_align(
        &mut self,
        align: BlockAlign,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        self.push_undo_snapshot();
        let (start_row, end_row) = self.selected_rows();
        for row in start_row..=end_row {
            if let Some(block) = self.document.blocks.get_mut(row) {
                block.format.align = align;
            }
        }
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn active_block_align(&self) -> BlockAlign {
        let (start_row, end_row) = self.selected_rows();
        let Some(first) = self.document.blocks.get(start_row) else {
            return BlockAlign::default();
        };

        let mut align = first.format.align;
        for row in start_row..=end_row {
            let Some(block) = self.document.blocks.get(row) else {
                continue;
            };
            if block.format.align != align {
                align = BlockAlign::default();
                break;
            }
        }

        align
    }

    pub fn set_text_color(
        &mut self,
        color: Option<gpui::Hsla>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let range = self.ordered_selection();
        if range.is_empty() {
            self.active_style.fg = color;
            cx.notify();
            return;
        }

        self.push_undo_snapshot();
        let mut block_ranges: Vec<(usize, Range<usize>)> = Vec::new();
        self.for_each_block_range_in_selection(range, |row, local_range| {
            block_ranges.push((row, local_range));
        });
        for (row, local_range) in block_ranges {
            self.update_inline_styles_in_block_range(row, local_range, |style| {
                style.fg = color;
            });
        }
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn set_highlight_color(
        &mut self,
        color: Option<gpui::Hsla>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let range = self.ordered_selection();
        if range.is_empty() {
            self.active_style.bg = color;
            cx.notify();
            return;
        }

        self.push_undo_snapshot();
        let mut block_ranges: Vec<(usize, Range<usize>)> = Vec::new();
        self.for_each_block_range_in_selection(range, |row, local_range| {
            block_ranges.push((row, local_range));
        });
        for (row, local_range) in block_ranges {
            self.update_inline_styles_in_block_range(row, local_range, |style| {
                style.bg = color;
            });
        }
        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn set_link(&mut self, url: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let url = url.and_then(|s| {
            let trimmed = s.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        });

        let range = self.ordered_selection();
        if range.is_empty() {
            self.active_style.link = url;
            cx.notify();
            return;
        }

        self.push_undo_snapshot();
        let mut block_ranges: Vec<(usize, Range<usize>)> = Vec::new();
        self.for_each_block_range_in_selection(range, |row, local_range| {
            block_ranges.push((row, local_range));
        });
        for (row, local_range) in block_ranges {
            let url = url.clone();
            self.update_inline_styles_in_block_range(row, local_range, |style| {
                style.link = url.clone();
            });
        }

        cx.notify();
        self.scroll_cursor_into_view(window, cx);
    }

    pub fn current_link_url(&self) -> Option<String> {
        let range = self.ordered_selection();
        if range.is_empty() {
            return self.link_at_offset(self.cursor());
        }

        let mut found: Option<Option<String>> = None;
        self.for_each_block_range_in_selection(range, |row, local_range| {
            let Some(block) = self.document.blocks.get(row) else {
                return;
            };
            let mut cursor = 0usize;
            for node in &block.inlines {
                let InlineNode::Text(text) = node;
                let end = cursor + text.text.len();
                let overlap_start = local_range.start.max(cursor);
                let overlap_end = local_range.end.min(end);
                if overlap_start < overlap_end {
                    let link = text.style.link.clone();
                    match &mut found {
                        None => found = Some(link),
                        Some(existing) => {
                            if *existing != link {
                                found = Some(None);
                            }
                        }
                    }
                }
                cursor = end;
            }
        });

        found.flatten()
    }

    pub fn set_link_at_cursor_or_selection(
        &mut self,
        url: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let url = url.and_then(|s| {
            let trimmed = s.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        });

        let range = self.ordered_selection();
        if !range.is_empty() {
            self.set_link(url, window, cx);
            return;
        }

        let cursor = self.cursor();
        if self.text.len() == 0 || self.document.blocks.is_empty() {
            self.active_style.link = url;
            cx.notify();
            return;
        }

        let point = self.text.offset_to_point(cursor);
        let row = point.row.min(self.document.blocks.len().saturating_sub(1));
        let col = point.column;

        let Some(block) = self.document.blocks.get(row) else {
            self.active_style.link = url;
            cx.notify();
            return;
        };

        let mut cursor_in_block = 0usize;
        let mut target: Option<Range<usize>> = None;
        for node in &block.inlines {
            let InlineNode::Text(text) = node;
            let end = cursor_in_block + text.text.len();
            if col < end {
                if text.style.link.is_some() {
                    target = Some(cursor_in_block..end);
                }
                break;
            }
            cursor_in_block = end;
        }

        if let Some(local_range) = target {
            self.push_undo_snapshot();
            self.update_inline_styles_in_block_range(row, local_range, |style| {
                style.link = url.clone();
            });
            cx.notify();
            self.scroll_cursor_into_view(window, cx);
            return;
        }

        self.active_style.link = url;
        cx.notify();
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

        start_row = start_row.min(self.document.blocks.len().saturating_sub(1));
        end_row = end_row.min(self.document.blocks.len().saturating_sub(1));
        if end_row < start_row {
            std::mem::swap(&mut start_row, &mut end_row);
        }
        (start_row, end_row)
    }

    #[allow(dead_code)]
    pub(crate) fn highlight_styles_for_line(
        &self,
        row: usize,
        base_color: gpui::Hsla,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let line_range = self.line_range(row);
        let line_start = line_range.start;
        let line_end = line_range.end;

        let mut highlights: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
        if let Some(block) = self.document.blocks.get(row) {
            let mut cursor = 0usize;
            for node in &block.inlines {
                let InlineNode::Text(text) = node;

                let len = text.text.len();
                if len == 0 {
                    continue;
                }

                let local = cursor..cursor + len;
                let mut highlight = HighlightStyle::default();

                if text.style.bold {
                    highlight.font_weight = Some(FontWeight::BOLD);
                }
                if text.style.italic {
                    highlight.font_style = Some(FontStyle::Italic);
                }
                if text.style.underline {
                    highlight.underline = Some(UnderlineStyle {
                        thickness: px(1.),
                        color: text.style.fg,
                        wavy: false,
                    });
                }
                if text.style.link.is_some() {
                    let link_color = text.style.fg.unwrap_or(self.theme.link);
                    highlight.color = Some(link_color);
                    if let Some(underline) = highlight.underline.as_mut() {
                        if underline.color.is_none() {
                            underline.color = Some(link_color);
                        }
                    } else {
                        highlight.underline = Some(UnderlineStyle {
                            thickness: px(1.),
                            color: Some(link_color),
                            wavy: false,
                        });
                    }
                }
                if text.style.strikethrough {
                    highlight.strikethrough = Some(StrikethroughStyle {
                        thickness: px(1.),
                        color: text.style.fg,
                        ..Default::default()
                    });
                }
                if let Some(color) = text.style.fg {
                    highlight.color = Some(color);
                }
                if let Some(bg) = text.style.bg {
                    highlight.background_color = Some(bg);
                }

                if highlight != HighlightStyle::default() {
                    highlights.push((local, highlight));
                }

                cursor += len;
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

    pub(crate) fn link_at_offset(&self, offset: usize) -> Option<String> {
        let point = self.text.offset_to_point(offset.min(self.text.len()));
        let row = point.row;
        let col = point.column;

        let block = self.document.blocks.get(row)?;
        let mut cursor = 0usize;
        for node in &block.inlines {
            let InlineNode::Text(text) = node;
            let len = text.text.len();
            if col < cursor + len {
                return text.style.link.clone();
            }
            cursor += len;
        }

        None
    }

    fn link_range_at_offset(&self, offset: usize) -> Option<(Range<usize>, String)> {
        let point = self.text.offset_to_point(offset.min(self.text.len()));
        let row = point.row.min(self.document.blocks.len().saturating_sub(1));
        let col = point.column;

        let block = self.document.blocks.get(row)?;
        let mut ranges: Vec<(Range<usize>, String)> = Vec::new();

        let mut cursor = 0usize;
        let mut current: Option<(usize, String)> = None;
        for node in &block.inlines {
            let InlineNode::Text(text) = node;
            let len = text.text.len();

            match (&mut current, text.style.link.as_ref()) {
                (Some((_start, url)), Some(link)) if url == link => {}
                (Some((start, url)), Some(link)) => {
                    ranges.push((*start..cursor, url.clone()));
                    *start = cursor;
                    *url = link.clone();
                }
                (Some((start, url)), None) => {
                    ranges.push((*start..cursor, url.clone()));
                    current = None;
                }
                (None, Some(link)) => current = Some((cursor, link.clone())),
                (None, None) => {}
            }

            cursor += len;
        }

        if let Some((start, url)) = current {
            ranges.push((start..cursor, url));
        }

        let block_start = self.text.line_start_offset(row);
        for (range, url) in ranges {
            if col >= range.start && col <= range.end {
                return Some(((block_start + range.start)..(block_start + range.end), url));
            }
        }

        None
    }

    pub fn active_link_overlay(&self) -> Option<ActiveLinkOverlay> {
        let viewport = self.viewport_bounds;
        if viewport.size.width <= px(0.) || viewport.size.height <= px(0.) {
            return None;
        }

        let cursor = self.cursor();
        let (range, url) = self.link_range_at_offset(cursor)?;
        let caret_bounds = self.caret_bounds_for_offset(range.start)?;

        let overlay_height = px(40.);
        let vertical_margin = px(8.);
        let prefer_above_y = caret_bounds.top() - overlay_height - vertical_margin;
        let prefer_below_y = caret_bounds.bottom() + vertical_margin;

        let mut anchor = point(caret_bounds.left(), prefer_above_y);

        let min_x = viewport.left() + px(8.);
        let max_x = (viewport.right() - px(200.)).max(min_x);
        if anchor.x < min_x {
            anchor.x = min_x;
        } else if anchor.x > max_x {
            anchor.x = max_x;
        }

        let min_y = viewport.top() + px(4.);
        let max_y = (viewport.bottom() - overlay_height - px(4.)).max(min_y);
        if anchor.y < min_y {
            anchor.y = prefer_below_y;
        }
        if anchor.y < min_y {
            anchor.y = min_y;
        } else if anchor.y > max_y {
            anchor.y = max_y;
        }

        Some(ActiveLinkOverlay { url, anchor })
    }

    pub(crate) fn offset_for_point(&self, point: Point<Pixels>) -> Option<usize> {
        // Find the closest row by 2D distance to its layout bounds. This improves hit-testing in
        // multi-column layouts where multiple rows can share the same y positions.
        let mut best: Option<(usize, f32)> = None;
        for (row, cache) in self.layout_cache.iter().enumerate() {
            let Some(cache) = cache.as_ref() else {
                continue;
            };

            let dx = if point.x < cache.bounds.left() {
                cache.bounds.left() - point.x
            } else if point.x > cache.bounds.right() {
                point.x - cache.bounds.right()
            } else {
                px(0.)
            };
            let dy = if point.y < cache.bounds.top() {
                cache.bounds.top() - point.y
            } else if point.y > cache.bounds.bottom() {
                point.y - cache.bounds.bottom()
            } else {
                px(0.)
            };

            let dist = f32::from(dx) * f32::from(dx) + f32::from(dy) * f32::from(dy);
            if best.is_none() || dist < best.unwrap().1 {
                best = Some((row, dist));
            }
        }

        let (row, _) = best?;
        let cache = self.layout_cache.get(row)?.as_ref()?;

        let local_index = self.aligned_index_for_row_point(row, point);

        Some((cache.start_offset + local_index).min(self.text.len()))
    }

    pub(crate) fn move_vertically(&mut self, direction: i32) -> Option<usize> {
        let cursor = self.cursor();
        let caret_bounds = self.caret_bounds_for_offset(cursor)?;
        let x = self.preferred_x.unwrap_or(caret_bounds.left());

        let row = self.text.offset_to_point(cursor).row;
        let cache = self.layout_cache.get(row)?.as_ref()?;
        let line_height = cache.text_layout.line_height();

        let y = caret_bounds.top() + line_height * 0.5 + (direction as f32) * line_height;
        self.offset_for_point(point(x, y))
    }

    // --- Action handlers ---

    pub(super) fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if self.has_selection() {
            let range = self.ordered_selection();
            self.replace_bytes_range(range, "", window, cx, true, false, None);
            return;
        }

        let cursor = self.cursor();
        if cursor == 0 {
            return;
        }

        // Special: at start of line, downgrade heading/list/quote/todo, or remove dividers.
        let pos = self.text.offset_to_point(cursor);
        if pos.column == 0 && pos.row < self.document.blocks.len() {
            let kind = self.document.blocks[pos.row].format.kind;
            if matches!(kind, BlockKind::Divider) {
                self.push_undo_snapshot();
                self.document.blocks.remove(pos.row);
                self.ensure_document_non_empty();
                self.text = Rope::from(self.document.to_plain_text().as_str());
                self.layout_cache = vec![None; self.document.blocks.len().max(1)];

                let target_row = pos
                    .row
                    .saturating_sub(1)
                    .min(self.document.blocks.len() - 1);
                let col = self
                    .document
                    .blocks
                    .get(target_row)
                    .map(|b| b.text_len())
                    .unwrap_or(0);
                let offset = self.text.line_start_offset(target_row) + col;
                self.set_cursor(offset);
                cx.notify();
                return;
            }
            if matches!(
                kind,
                BlockKind::Heading { .. }
                    | BlockKind::Quote
                    | BlockKind::UnorderedListItem
                    | BlockKind::OrderedListItem
                    | BlockKind::Todo { .. }
                    | BlockKind::Toggle { .. }
            ) {
                self.push_undo_snapshot();
                self.document.blocks[pos.row].format.kind = BlockKind::Paragraph;
                cx.notify();
                return;
            }
        }

        let start = self.previous_boundary(cursor);
        self.replace_bytes_range(start..cursor, "", window, cx, true, false, None);
    }

    pub(super) fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
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
        if self.read_only {
            return;
        }
        if self.has_selection() {
            let range = self.ordered_selection();
            self.replace_bytes_range(range, "", window, cx, true, false, None);
        }

        let cursor = self.cursor();
        let row = self.text.offset_to_point(cursor).row;

        if row < self.document.blocks.len() {
            let kind = self.document.blocks[row].format.kind;

            // Divider is a void block; pressing enter should move to a new paragraph below.
            if matches!(kind, BlockKind::Divider) {
                self.push_undo_snapshot();
                let insert_row = (row + 1).min(self.document.blocks.len());
                self.document
                    .blocks
                    .insert(insert_row, BlockNode::default());

                self.text = Rope::from(self.document.to_plain_text().as_str());
                self.layout_cache = vec![None; self.document.blocks.len().max(1)];
                self.selection = Selection::default();
                self.ime_marked_range = None;
                self.active_style = InlineStyle::default();
                self.preferred_x = None;

                let offset = self.text.line_start_offset(insert_row);
                self.set_cursor(offset);
                cx.notify();
                self.scroll_cursor_into_view(window, cx);
                return;
            }

            // Exit list/quote/todo on an empty line.
            if matches!(
                kind,
                BlockKind::UnorderedListItem
                    | BlockKind::OrderedListItem
                    | BlockKind::Quote
                    | BlockKind::Todo { .. }
                    | BlockKind::Toggle { .. }
            ) && self.document.blocks[row].is_text_empty()
            {
                self.push_undo_snapshot();
                self.document.blocks[row].format.kind = BlockKind::Paragraph;
                cx.notify();
                return;
            }

            // If the current toggle is collapsed, expand it so newly inserted children are visible.
            if let BlockKind::Toggle { collapsed: true } = kind {
                self.document.blocks[row].format.kind = BlockKind::Toggle { collapsed: false };
            }
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
        if self.read_only {
            return;
        }
        let range = self.ordered_selection();
        if range.is_empty() {
            return;
        }
        let selected_text = self.text.slice(range.clone()).to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(selected_text));
        self.replace_bytes_range(range, "", window, cx, true, false, None);
    }

    pub(super) fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let Some(clipboard) = cx.read_from_clipboard() else {
            return;
        };
        let mut new_text = clipboard.text().unwrap_or_default();
        new_text = new_text.replace("\r\n", "\n").replace('\r', "\n");
        let range = self.ordered_selection();
        self.replace_bytes_range(range, &new_text, window, cx, true, false, None);
    }

    pub(super) fn undo(&mut self, _: &Undo, _window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let Some(snapshot) = self.undo_stack.pop() else {
            return;
        };
        self.redo_stack.push(Snapshot {
            document: self.document.clone(),
            selection: self.selection,
            active_style: self.active_style.clone(),
        });
        self.restore_snapshot(snapshot);
        cx.notify();
    }

    pub(super) fn redo(&mut self, _: &Redo, _window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let Some(snapshot) = self.redo_stack.pop() else {
            return;
        };
        self.undo_stack.push(Snapshot {
            document: self.document.clone(),
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
