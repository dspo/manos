use std::ops::Range;
use std::path::PathBuf;

use gpui::actions;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, TitleBar, WindowExt as _,
    button::{Button, ButtonVariants as _},
    menu::AppMenuBar,
    notification::Notification,
};
use gpui_manos_components::plate_toolbar::{
    PlateIconName, PlateToolbarIconButton, PlateToolbarSeparator,
};
use gpui_plate_core::{Editor, Node, PlateValue, Point as CorePoint, Selection};

use crate::app_menus::{About, Open, Save, SaveAs};

pub(super) const CONTEXT: &str = "RichTextNext";

actions!(
    richtext_next,
    [
        Backspace,
        Delete,
        Enter,
        MoveLeft,
        MoveRight,
        SelectLeft,
        SelectRight,
        Undo,
        Redo,
        InsertDivider,
    ]
);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some(CONTEXT)),
        KeyBinding::new("delete", Delete, Some(CONTEXT)),
        KeyBinding::new("enter", Enter, Some(CONTEXT)),
        KeyBinding::new("left", MoveLeft, Some(CONTEXT)),
        KeyBinding::new("right", MoveRight, Some(CONTEXT)),
        KeyBinding::new("shift-left", SelectLeft, Some(CONTEXT)),
        KeyBinding::new("shift-right", SelectRight, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-z", Undo, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-z", Undo, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-z", Redo, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-z", Redo, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-d", InsertDivider, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-d", InsertDivider, Some(CONTEXT)),
    ]);
}

#[derive(Clone)]
pub struct LineLayoutCache {
    pub bounds: Bounds<Pixels>,
    pub text_layout: gpui::TextLayout,
    pub text: SharedString,
}

pub struct RichTextNextState {
    focus_handle: FocusHandle,
    scroll_handle: ScrollHandle,
    editor: Editor,
    selecting: bool,
    selection_anchor: Option<CorePoint>,
    viewport_bounds: Bounds<Pixels>,
    layout_cache: Vec<Option<LineLayoutCache>>,
    ime_marked_range: Option<Range<usize>>,
    did_auto_focus: bool,
}

impl RichTextNextState {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle().tab_stop(true);
        Self {
            focus_handle,
            scroll_handle: ScrollHandle::new(),
            editor: Editor::with_core_plugins(),
            selecting: false,
            selection_anchor: None,
            viewport_bounds: Bounds::default(),
            layout_cache: Vec::new(),
            ime_marked_range: None,
            did_auto_focus: false,
        }
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub fn can_undo(&self) -> bool {
        self.editor.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.editor.can_redo()
    }

    pub fn plate_value(&self) -> PlateValue {
        PlateValue::from_document(self.editor.doc().clone())
    }

    pub fn load_plate_value(&mut self, value: PlateValue, cx: &mut Context<Self>) {
        let selection = Selection::collapsed(CorePoint::new(vec![0, 0], 0));
        self.editor = Editor::new(
            value.into_document(),
            selection,
            gpui_plate_core::PluginRegistry::core(),
        );
        self.scroll_handle.set_offset(point(px(0.), px(0.)));
        self.ime_marked_range = None;
        cx.notify();
    }

    fn active_text_path(&self) -> Option<Vec<usize>> {
        Some(self.editor.selection().focus.path.clone())
    }

    fn active_block_index(&self) -> usize {
        self.editor
            .selection()
            .focus
            .path
            .first()
            .copied()
            .unwrap_or(0)
    }

    fn set_selection_in_row(
        &mut self,
        row: usize,
        anchor: usize,
        focus: usize,
        cx: &mut Context<Self>,
    ) {
        let row = row.min(self.editor.doc().children.len().saturating_sub(1));
        let path = vec![row, 0];
        let sel = Selection {
            anchor: CorePoint::new(path.clone(), anchor),
            focus: CorePoint::new(path, focus),
        };
        self.editor.set_selection(sel);
        _ = self.scroll_cursor_into_view();
        self.ime_marked_range = None;
        cx.notify();
    }

    fn set_selection_points(
        &mut self,
        anchor: CorePoint,
        focus: CorePoint,
        cx: &mut Context<Self>,
    ) {
        self.editor.set_selection(Selection { anchor, focus });
        _ = self.scroll_cursor_into_view();
        self.ime_marked_range = None;
        cx.notify();
    }

    fn active_text(&self) -> SharedString {
        let path = self.editor.selection().focus.path.clone();
        let mut node = None;
        if path.len() >= 2 {
            if let Some(block) = self.editor.doc().children.get(path[0]) {
                if let Node::Element(el) = block {
                    node = el.children.get(path[1]);
                }
            }
        }
        match node {
            Some(Node::Text(t)) => t.text.clone().into(),
            _ => SharedString::default(),
        }
    }

    fn set_selection_in_active_block(
        &mut self,
        anchor: usize,
        focus: usize,
        cx: &mut Context<Self>,
    ) {
        let block_ix = self.active_block_index();
        let path = vec![block_ix, 0];
        let sel = Selection {
            anchor: CorePoint::new(path.clone(), anchor),
            focus: CorePoint::new(path, focus),
        };
        self.editor.set_selection(sel);
        _ = self.scroll_cursor_into_view();
        self.ime_marked_range = None;
        cx.notify();
    }

    fn ordered_active_range(&self) -> Range<usize> {
        let sel = self.editor.selection();
        if sel.anchor.path != sel.focus.path {
            return sel.focus.offset..sel.focus.offset;
        }
        let mut a = sel.anchor.offset;
        let mut b = sel.focus.offset;
        if b < a {
            std::mem::swap(&mut a, &mut b);
        }
        a..b
    }

    fn ordered_selection_points(&self) -> (CorePoint, CorePoint) {
        let sel = self.editor.selection();
        if sel.anchor.path == sel.focus.path {
            if sel.focus.offset < sel.anchor.offset {
                return (sel.focus.clone(), sel.anchor.clone());
            }
            return (sel.anchor.clone(), sel.focus.clone());
        }
        if sel.focus.path < sel.anchor.path {
            return (sel.focus.clone(), sel.anchor.clone());
        }
        (sel.anchor.clone(), sel.focus.clone())
    }

    fn paragraph_text(&self, row: usize) -> Option<SharedString> {
        let Some(Node::Element(el)) = self.editor.doc().children.get(row) else {
            return None;
        };
        if el.kind != "paragraph" {
            return None;
        }
        let Some(Node::Text(t)) = el.children.first() else {
            return Some(SharedString::default());
        };
        Some(t.text.clone().into())
    }

    fn prev_boundary(text: &str, offset: usize) -> usize {
        if offset == 0 {
            return 0;
        }
        let mut ix = (offset - 1).min(text.len());
        while ix > 0 && !text.is_char_boundary(ix) {
            ix -= 1;
        }
        ix
    }

    fn next_boundary(text: &str, offset: usize) -> usize {
        if offset >= text.len() {
            return text.len();
        }
        let mut ix = (offset + 1).min(text.len());
        while ix < text.len() && !text.is_char_boundary(ix) {
            ix += 1;
        }
        ix
    }

    fn is_word_char(ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_'
    }

    fn word_range(text: &str, offset: usize) -> Range<usize> {
        if text.is_empty() {
            return 0..0;
        }

        let offset = offset.min(text.len());
        let mut start = offset;
        while start > 0 && !text.is_char_boundary(start) {
            start -= 1;
        }
        let mut end = offset;
        while end < text.len() && !text.is_char_boundary(end) {
            end += 1;
        }

        let probe_ix = if start == text.len() {
            Self::prev_boundary(text, start)
        } else if start == end && start > 0 {
            Self::prev_boundary(text, start)
        } else {
            start
        };
        let probe = text.get(probe_ix..).and_then(|s| s.chars().next());
        let Some(probe) = probe else {
            return 0..0;
        };

        if probe.is_whitespace() {
            let mut a = probe_ix;
            while a > 0 {
                let prev = Self::prev_boundary(text, a);
                let Some(ch) = text.get(prev..).and_then(|s| s.chars().next()) else {
                    break;
                };
                if !ch.is_whitespace() {
                    break;
                }
                a = prev;
            }
            let mut b = probe_ix + probe.len_utf8();
            while b < text.len() {
                let next = Self::next_boundary(text, b);
                let Some(ch) = text.get(b..).and_then(|s| s.chars().next()) else {
                    break;
                };
                if !ch.is_whitespace() {
                    break;
                }
                b = next;
            }
            return a..b;
        }

        if Self::is_word_char(probe) {
            let mut a = probe_ix;
            while a > 0 {
                let prev = Self::prev_boundary(text, a);
                let Some(ch) = text.get(prev..).and_then(|s| s.chars().next()) else {
                    break;
                };
                if !Self::is_word_char(ch) {
                    break;
                }
                a = prev;
            }
            let mut b = probe_ix + probe.len_utf8();
            while b < text.len() {
                let next = Self::next_boundary(text, b);
                let Some(ch) = text.get(b..).and_then(|s| s.chars().next()) else {
                    break;
                };
                if !Self::is_word_char(ch) {
                    break;
                }
                b = next;
            }
            return a..b;
        }

        probe_ix..probe_ix + probe.len_utf8()
    }

    fn line_range(text: &str, offset: usize) -> Range<usize> {
        if text.is_empty() {
            return 0..0;
        }

        let offset = offset.min(text.len());
        let left = text[..offset].rfind('\n').map(|ix| ix + 1).unwrap_or(0);
        let right = text[offset..]
            .find('\n')
            .map(|ix| offset + ix)
            .unwrap_or(text.len());
        left..right
    }

    fn tx_insert_text_at_point(
        &self,
        at: &CorePoint,
        inserted: &str,
        source: &'static str,
    ) -> Option<gpui_plate_core::Transaction> {
        if at.path.len() != 2 || at.path[1] != 0 {
            return None;
        }
        let row = at.path[0];
        let Some(existing) = self.paragraph_text(row) else {
            return None;
        };

        let existing = existing.to_string();
        let len = existing.len();
        let offset = at.offset.min(len);
        let tail = existing.get(offset..).unwrap_or("").to_string();

        let parts: Vec<&str> = inserted.split('\n').collect();
        if parts.len() <= 1 {
            return Some(
                gpui_plate_core::Transaction::new(vec![gpui_plate_core::Op::InsertText {
                    path: at.path.clone(),
                    offset,
                    text: inserted.to_string(),
                }])
                .selection_after(Selection::collapsed(CorePoint::new(
                    at.path.clone(),
                    offset + inserted.len(),
                )))
                .source(source),
            );
        }

        let mut ops = Vec::new();
        ops.push(gpui_plate_core::Op::RemoveText {
            path: at.path.clone(),
            range: offset..len,
        });
        if !parts[0].is_empty() {
            ops.push(gpui_plate_core::Op::InsertText {
                path: at.path.clone(),
                offset,
                text: parts[0].to_string(),
            });
        }

        let last_ix = parts.len() - 1;
        for (i, part) in parts.iter().enumerate().skip(1) {
            let text = if i == last_ix {
                format!("{part}{tail}")
            } else {
                (*part).to_string()
            };
            ops.push(gpui_plate_core::Op::InsertNode {
                path: vec![row + i],
                node: Node::paragraph(text),
            });
        }

        let selection_after =
            Selection::collapsed(CorePoint::new(vec![row + last_ix, 0], parts[last_ix].len()));
        Some(
            gpui_plate_core::Transaction::new(ops)
                .selection_after(selection_after)
                .source(source),
        )
    }

    fn delete_selection_transaction(&self) -> Option<(gpui_plate_core::Transaction, CorePoint)> {
        let sel = self.editor.selection();
        if sel.is_collapsed() {
            return None;
        }

        let (start, end) = self.ordered_selection_points();
        if start.path.len() != 2 || end.path.len() != 2 || start.path[1] != 0 || end.path[1] != 0 {
            return None;
        }

        let start_row = start.path[0];
        let end_row = end.path[0];
        if start_row == end_row {
            return Some((
                gpui_plate_core::Transaction::new(vec![gpui_plate_core::Op::RemoveText {
                    path: start.path.clone(),
                    range: start.offset..end.offset,
                }])
                .selection_after(Selection::collapsed(start.clone()))
                .source("selection:delete"),
                start,
            ));
        }

        let start_text = self.paragraph_text(start_row)?.to_string();
        let end_text = self.paragraph_text(end_row)?.to_string();
        let tail = end_text.get(end.offset..).unwrap_or("").to_string();
        let start_len = start_text.len();

        let mut ops = Vec::new();
        ops.push(gpui_plate_core::Op::RemoveText {
            path: start.path.clone(),
            range: start.offset..start_len,
        });
        if !tail.is_empty() {
            ops.push(gpui_plate_core::Op::InsertText {
                path: start.path.clone(),
                offset: start.offset,
                text: tail,
            });
        }

        for row in (start_row + 1..=end_row).rev() {
            ops.push(gpui_plate_core::Op::RemoveNode { path: vec![row] });
        }

        Some((
            gpui_plate_core::Transaction::new(ops)
                .selection_after(Selection::collapsed(start.clone()))
                .source("selection:delete_multi_paragraph"),
            start,
        ))
    }

    fn delete_selection_if_any(&mut self, cx: &mut Context<Self>) -> Option<CorePoint> {
        let (tx, collapse_to) = self.delete_selection_transaction()?;
        self.push_tx(tx, cx);
        Some(collapse_to)
    }

    fn offset_for_point(&self, row: usize, point: gpui::Point<Pixels>) -> Option<usize> {
        let cache = self.layout_cache.get(row).and_then(|c| c.as_ref())?;
        let mut local = match cache.text_layout.index_for_position(point) {
            Ok(ix) | Err(ix) => ix,
        };
        local = local.min(cache.text.len());
        Some(local)
    }

    fn block_text_len(&self, row: usize) -> usize {
        if let Some(Node::Element(el)) = self.editor.doc().children.get(row)
            && el.kind == "paragraph"
        {
            if let Some(Node::Text(t)) = el.children.first() {
                return t.text.len();
            }
        }
        0
    }

    fn paragraph_row_for_point(&self, point: gpui::Point<Pixels>) -> Option<usize> {
        let mut first: Option<usize> = None;
        let mut last: Option<usize> = None;

        for (row, cache) in self.layout_cache.iter().enumerate() {
            let Some(cache) = cache.as_ref() else {
                continue;
            };
            first.get_or_insert(row);
            last = Some(row);
            if point.y >= cache.bounds.top() && point.y <= cache.bounds.bottom() {
                return Some(row);
            }
        }

        // Clicked outside any cached line; clamp to nearest known paragraph row.
        if let (Some(first), Some(last)) = (first, last) {
            if let Some(first_cache) = self.layout_cache.get(first).and_then(|c| c.as_ref())
                && point.y < first_cache.bounds.top()
            {
                return Some(first);
            }
            return Some(last);
        }

        None
    }

    fn push_tx(&mut self, tx: gpui_plate_core::Transaction, cx: &mut Context<Self>) {
        if self.editor.apply(tx).is_ok() {
            _ = self.scroll_cursor_into_view();
            cx.notify();
        }
    }

    fn scroll_cursor_into_view(&mut self) -> bool {
        let Some(row) = self.editor.selection().focus.path.first().copied() else {
            return false;
        };
        let Some(cache) = self.layout_cache.get(row).and_then(|c| c.as_ref()) else {
            return false;
        };
        let viewport = self.viewport_bounds;
        if viewport.size.width == px(0.) && viewport.size.height == px(0.) {
            return false;
        }

        let caret_ix = self
            .editor
            .selection()
            .focus
            .offset
            .min(cache.text_layout.len());
        let Some(pos) = cache.text_layout.position_for_index(caret_ix).or_else(|| {
            cache
                .text_layout
                .position_for_index(cache.text_layout.len())
        }) else {
            return false;
        };

        let line_height = cache.text_layout.line_height();
        let caret_bounds = Bounds::from_corners(pos, point(pos.x + px(1.5), pos.y + line_height));

        let prev_offset = self.scroll_handle.offset();
        let mut offset = prev_offset;

        let margin = px(12.);
        if caret_bounds.top() < viewport.top() + margin {
            offset.y += (viewport.top() + margin) - caret_bounds.top();
        } else if caret_bounds.bottom() > viewport.bottom() - margin {
            offset.y -= caret_bounds.bottom() - (viewport.bottom() - margin);
        }

        if offset == prev_offset {
            return false;
        }
        self.scroll_handle.set_offset(offset);
        true
    }

    pub(super) fn backspace(
        &mut self,
        _: &Backspace,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.delete_selection_if_any(cx).is_some() {
            return;
        }

        let text = self.active_text();
        let range = self.ordered_active_range();
        if !range.is_empty() {
            self.push_tx(
                gpui_plate_core::Transaction::new(vec![gpui_plate_core::Op::RemoveText {
                    path: self.active_text_path().unwrap_or_default(),
                    range: range.clone(),
                }])
                .selection_after(Selection::collapsed(CorePoint::new(
                    self.active_text_path().unwrap_or_default(),
                    range.start,
                )))
                .source("key:backspace"),
                cx,
            );
            return;
        }

        let cursor = self.editor.selection().focus.offset;
        let start = Self::prev_boundary(text.as_str(), cursor);
        if start == cursor {
            return;
        }
        self.push_tx(
            gpui_plate_core::Transaction::new(vec![gpui_plate_core::Op::RemoveText {
                path: self.active_text_path().unwrap_or_default(),
                range: start..cursor,
            }])
            .selection_after(Selection::collapsed(CorePoint::new(
                self.active_text_path().unwrap_or_default(),
                start,
            )))
            .source("key:backspace"),
            cx,
        );
    }

    pub(super) fn delete(&mut self, _: &Delete, _window: &mut Window, cx: &mut Context<Self>) {
        if self.delete_selection_if_any(cx).is_some() {
            return;
        }

        let text = self.active_text();
        let range = self.ordered_active_range();
        if !range.is_empty() {
            self.backspace(&Backspace, _window, cx);
            return;
        }

        let cursor = self.editor.selection().focus.offset;
        let end = Self::next_boundary(text.as_str(), cursor);
        if end == cursor {
            return;
        }
        self.push_tx(
            gpui_plate_core::Transaction::new(vec![gpui_plate_core::Op::RemoveText {
                path: self.active_text_path().unwrap_or_default(),
                range: cursor..end,
            }])
            .selection_after(Selection::collapsed(CorePoint::new(
                self.active_text_path().unwrap_or_default(),
                cursor,
            )))
            .source("key:delete"),
            cx,
        );
    }

    pub(super) fn enter(&mut self, _: &Enter, _window: &mut Window, cx: &mut Context<Self>) {
        _ = self.delete_selection_if_any(cx);

        let block_ix = self.active_block_index();
        let path = self.active_text_path().unwrap_or_default();
        if path.len() < 2 {
            return;
        }

        let text = self.active_text();
        let cursor = self.editor.selection().focus.offset.min(text.len());
        let tail = text.as_str().get(cursor..).unwrap_or("").to_string();
        let len = text.len();

        let new_block_ix = block_ix.saturating_add(1);
        let tx = gpui_plate_core::Transaction::new(vec![
            gpui_plate_core::Op::RemoveText {
                path: path.clone(),
                range: cursor..len,
            },
            gpui_plate_core::Op::InsertNode {
                path: vec![new_block_ix],
                node: Node::paragraph(tail),
            },
        ])
        .selection_after(Selection::collapsed(CorePoint::new(
            vec![new_block_ix, 0],
            0,
        )))
        .source("key:enter:split_paragraph");
        self.push_tx(tx, cx);
    }

    pub(super) fn left(&mut self, _: &MoveLeft, _window: &mut Window, cx: &mut Context<Self>) {
        let text = self.active_text();
        let sel = self.editor.selection().clone();
        if !sel.is_collapsed() {
            let range = self.ordered_active_range();
            self.set_selection_in_active_block(range.start, range.start, cx);
            return;
        }
        let cursor = sel.focus.offset;
        let new_cursor = Self::prev_boundary(text.as_str(), cursor);
        self.set_selection_in_active_block(new_cursor, new_cursor, cx);
    }

    pub(super) fn right(&mut self, _: &MoveRight, _window: &mut Window, cx: &mut Context<Self>) {
        let text = self.active_text();
        let sel = self.editor.selection().clone();
        if !sel.is_collapsed() {
            let range = self.ordered_active_range();
            self.set_selection_in_active_block(range.end, range.end, cx);
            return;
        }
        let cursor = sel.focus.offset;
        let new_cursor = Self::next_boundary(text.as_str(), cursor);
        self.set_selection_in_active_block(new_cursor, new_cursor, cx);
    }

    pub(super) fn select_left(
        &mut self,
        _: &SelectLeft,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self.active_text();
        let sel = self.editor.selection().clone();
        let cursor = sel.focus.offset;
        let new_cursor = Self::prev_boundary(text.as_str(), cursor);
        let anchor = if sel.is_collapsed() {
            cursor
        } else {
            sel.anchor.offset
        };
        self.set_selection_in_active_block(anchor, new_cursor, cx);
    }

    pub(super) fn select_right(
        &mut self,
        _: &SelectRight,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self.active_text();
        let sel = self.editor.selection().clone();
        let cursor = sel.focus.offset;
        let new_cursor = Self::next_boundary(text.as_str(), cursor);
        let anchor = if sel.is_collapsed() {
            cursor
        } else {
            sel.anchor.offset
        };
        self.set_selection_in_active_block(anchor, new_cursor, cx);
    }

    pub(super) fn undo(&mut self, _: &Undo, _window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.undo() {
            cx.notify();
        }
    }

    pub(super) fn redo(&mut self, _: &Redo, _window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.redo() {
            cx.notify();
        }
    }

    pub(super) fn insert_divider(
        &mut self,
        _: &InsertDivider,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editor.run_command("core.insert_divider", None).is_ok() {
            cx.notify();
        }
    }

    pub fn command_undo(&mut self, cx: &mut Context<Self>) {
        if self.editor.undo() {
            cx.notify();
        }
    }

    pub fn command_redo(&mut self, cx: &mut Context<Self>) {
        if self.editor.redo() {
            cx.notify();
        }
    }

    pub fn command_insert_divider(&mut self, cx: &mut Context<Self>) {
        if self.editor.run_command("core.insert_divider", None).is_ok() {
            cx.notify();
        }
    }

    pub fn command_copy(&mut self, cx: &mut Context<Self>) {
        let sel = self.editor.selection().clone();
        if sel.is_collapsed() {
            return;
        }

        let (start, end) = self.ordered_selection_points();
        if start.path.len() == 2 && end.path.len() == 2 && start.path[1] == 0 && end.path[1] == 0 {
            let start_row = start.path[0];
            let end_row = end.path[0];
            if start_row == end_row {
                let text = self.paragraph_text(start_row).unwrap_or_default();
                let selected = text
                    .as_str()
                    .get(start.offset..end.offset)
                    .unwrap_or("")
                    .to_string();
                cx.write_to_clipboard(ClipboardItem::new_string(selected));
                return;
            }

            let mut out = String::new();
            let start_text = self.paragraph_text(start_row).unwrap_or_default();
            out.push_str(start_text.as_str().get(start.offset..).unwrap_or(""));
            out.push('\n');
            for row in (start_row + 1)..end_row {
                let mid = self.paragraph_text(row).unwrap_or_default();
                out.push_str(mid.as_str());
                out.push('\n');
            }
            let end_text = self.paragraph_text(end_row).unwrap_or_default();
            out.push_str(end_text.as_str().get(..end.offset).unwrap_or(""));
            cx.write_to_clipboard(ClipboardItem::new_string(out));
        }
    }

    pub fn command_cut(&mut self, cx: &mut Context<Self>) {
        self.command_copy(cx);
        _ = self.delete_selection_if_any(cx);
    }

    pub fn command_paste(&mut self, cx: &mut Context<Self>) {
        let Some(clipboard) = cx.read_from_clipboard() else {
            return;
        };
        let mut new_text = clipboard.text().unwrap_or_default();
        new_text = new_text.replace("\r\n", "\n").replace('\r', "\n");
        if new_text.is_empty() {
            return;
        }

        let insert_at = self
            .delete_selection_if_any(cx)
            .unwrap_or_else(|| self.editor.selection().focus.clone());
        let path = insert_at.path.clone();

        if new_text.contains('\n') {
            if let Some(tx) = self.tx_insert_text_at_point(&insert_at, &new_text, "command:paste") {
                self.push_tx(tx, cx);
                return;
            }
        }

        let mut ops = Vec::new();
        ops.push(gpui_plate_core::Op::InsertText {
            path: path.clone(),
            offset: insert_at.offset,
            text: new_text.clone(),
        });

        let tx = gpui_plate_core::Transaction::new(ops)
            .selection_after(Selection::collapsed(CorePoint::new(
                path,
                insert_at.offset + new_text.len(),
            )))
            .source("command:paste");
        self.push_tx(tx, cx);
    }

    pub fn command_select_all(&mut self, cx: &mut Context<Self>) {
        let row = self.active_block_index();
        let len = self.block_text_len(row);
        self.set_selection_in_row(row, 0, len, cx);
    }
}

impl EntityInputHandler for RichTextNextState {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.active_text();
        let start = utf16_to_byte(text.as_str(), range_utf16.start);
        let end = utf16_to_byte(text.as_str(), range_utf16.end);
        adjusted_range.replace(byte_to_utf16_range(text.as_str(), start..end));
        Some(text.as_str().get(start..end).unwrap_or("").to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let text = self.active_text();
        let range = self.ordered_active_range();
        let utf16 = byte_to_utf16_range(text.as_str(), range.clone());
        Some(UTF16Selection {
            range: utf16,
            reversed: self.editor.selection().focus.offset < self.editor.selection().anchor.offset,
        })
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self.active_text();
        let range_bytes = range_utf16
            .as_ref()
            .map(|r| utf16_to_byte(text.as_str(), r.start)..utf16_to_byte(text.as_str(), r.end))
            .or_else(|| self.ime_marked_range.clone())
            .unwrap_or_else(|| self.ordered_active_range());

        let mut ops = Vec::new();
        let path = self.active_text_path().unwrap_or_default();

        if range_bytes.start < range_bytes.end {
            ops.push(gpui_plate_core::Op::RemoveText {
                path: path.clone(),
                range: range_bytes.clone(),
            });
        }

        let inserted = new_text.replace("\r\n", "\n").replace('\r', "\n");
        if !inserted.is_empty() {
            ops.push(gpui_plate_core::Op::InsertText {
                path: path.clone(),
                offset: range_bytes.start,
                text: inserted.clone(),
            });
        }

        let new_cursor = range_bytes.start + inserted.len();
        let tx = gpui_plate_core::Transaction::new(ops)
            .selection_after(Selection::collapsed(CorePoint::new(path, new_cursor)))
            .source("ime:replace_text");
        self.push_tx(tx, cx);
        // Committed input ends any active preedit range.
        self.ime_marked_range = None;
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let text = self.active_text();
        self.ime_marked_range
            .as_ref()
            .map(|r| byte_to_utf16_range(text.as_str(), r.clone()))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.ime_marked_range = None;
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self.active_text();
        let range_bytes = range_utf16
            .as_ref()
            .map(|r| utf16_to_byte(text.as_str(), r.start)..utf16_to_byte(text.as_str(), r.end))
            .or_else(|| self.ime_marked_range.clone())
            .unwrap_or_else(|| self.ordered_active_range());

        let mut ops = Vec::new();
        let path = self.active_text_path().unwrap_or_default();

        if range_bytes.start < range_bytes.end {
            ops.push(gpui_plate_core::Op::RemoveText {
                path: path.clone(),
                range: range_bytes.clone(),
            });
        }

        let inserted = new_text.replace("\r\n", "\n").replace('\r', "\n");
        if !inserted.is_empty() {
            ops.push(gpui_plate_core::Op::InsertText {
                path: path.clone(),
                offset: range_bytes.start,
                text: inserted.clone(),
            });
        }

        let inserted_len = inserted.len();
        let marked = if inserted_len == 0 {
            None
        } else {
            Some(range_bytes.start..range_bytes.start + inserted_len)
        };
        self.ime_marked_range = marked.clone();

        let selection_after =
            if let (Some(marked), Some(sel_utf16)) = (marked, new_selected_range_utf16) {
                let rel_start = utf16_to_byte(inserted.as_str(), sel_utf16.start);
                let rel_end = utf16_to_byte(inserted.as_str(), sel_utf16.end);
                Selection {
                    anchor: CorePoint::new(path.clone(), marked.start + rel_start),
                    focus: CorePoint::new(path.clone(), marked.start + rel_end),
                }
            } else {
                Selection::collapsed(CorePoint::new(
                    path.clone(),
                    range_bytes.start + inserted_len,
                ))
            };

        let tx = gpui_plate_core::Transaction::new(ops)
            .selection_after(selection_after)
            .source("ime:replace_and_mark_text");
        self.push_tx(tx, cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let row = self.active_block_index();
        let cache = self.layout_cache.get(row).and_then(|c| c.as_ref())?;
        let text = cache.text.as_str();
        let start = utf16_to_byte(text, range_utf16.start);
        let pos = cache.text_layout.position_for_index(start)?;
        let line_height = cache.text_layout.line_height();
        Some(Bounds::from_corners(
            pos,
            point(pos.x + px(1.0), pos.y + line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let row = self.active_block_index();
        let byte_ix = self.offset_for_point(row, point)?;
        let text = self.active_text();
        Some(byte_to_utf16(text.as_str(), byte_ix))
    }
}

impl gpui::Focusable for RichTextNextState {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

pub struct RichTextNextLineElement {
    state: Entity<RichTextNextState>,
    row: usize,
    styled_text: StyledText,
    text: SharedString,
}

impl RichTextNextLineElement {
    pub fn new(state: Entity<RichTextNextState>, row: usize) -> Self {
        Self {
            state,
            row,
            styled_text: StyledText::new(SharedString::default()),
            text: SharedString::default(),
        }
    }

    fn paint_selection_positions(
        start_position: Point<Pixels>,
        end_position: Point<Pixels>,
        bounds: &Bounds<Pixels>,
        line_height: Pixels,
        window: &mut Window,
        selection_color: gpui::Hsla,
    ) {
        if start_position.y == end_position.y {
            window.paint_quad(gpui::quad(
                Bounds::from_corners(
                    start_position,
                    point(end_position.x, end_position.y + line_height),
                ),
                px(0.),
                selection_color,
                gpui::Edges::default(),
                gpui::transparent_black(),
                gpui::BorderStyle::default(),
            ));
            return;
        }

        window.paint_quad(gpui::quad(
            Bounds::from_corners(
                start_position,
                point(bounds.right(), start_position.y + line_height),
            ),
            px(0.),
            selection_color,
            gpui::Edges::default(),
            gpui::transparent_black(),
            gpui::BorderStyle::default(),
        ));

        if end_position.y > start_position.y + line_height {
            window.paint_quad(gpui::quad(
                Bounds::from_corners(
                    point(bounds.left(), start_position.y + line_height),
                    point(bounds.right(), end_position.y),
                ),
                px(0.),
                selection_color,
                gpui::Edges::default(),
                gpui::transparent_black(),
                gpui::BorderStyle::default(),
            ));
        }

        window.paint_quad(gpui::quad(
            Bounds::from_corners(
                point(bounds.left(), end_position.y),
                point(end_position.x, end_position.y + line_height),
            ),
            px(0.),
            selection_color,
            gpui::Edges::default(),
            gpui::transparent_black(),
            gpui::BorderStyle::default(),
        ));
    }
}

impl IntoElement for RichTextNextLineElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for RichTextNextLineElement {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_element_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let (text, marks, marked_range) = {
            let state = self.state.read(cx);
            let Some(Node::Element(el)) = state.editor.doc().children.get(self.row) else {
                return (window.request_layout(gpui::Style::default(), [], cx), ());
            };
            if el.kind != "paragraph" {
                self.text = SharedString::default();
                return (window.request_layout(gpui::Style::default(), [], cx), ());
            }
            let Some(Node::Text(t)) = el.children.first() else {
                return (window.request_layout(gpui::Style::default(), [], cx), ());
            };
            let active_row = state.active_block_index();
            let marked = (active_row == self.row)
                .then(|| state.ime_marked_range.clone())
                .flatten();
            (SharedString::new(t.text.clone()), t.marks.clone(), marked)
        };

        self.text = text.clone();
        let render_text: SharedString = if text.is_empty() { " ".into() } else { text };

        let mut style = window.text_style();
        if marks.bold {
            style.font_weight = FontWeight::BOLD;
        }
        if marks.link.is_some() {
            style.color = cx.theme().blue;
            style.underline = Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(cx.theme().blue),
                wavy: false,
            });
        }

        let run_len = render_text.len().max(1);
        let mut runs = Vec::new();

        let marked_range = marked_range
            .filter(|_| !self.text.is_empty())
            .map(|r| r.start.min(self.text.len())..r.end.min(self.text.len()))
            .filter(|r| r.start < r.end);

        if let Some(marked) = marked_range {
            if marked.start > 0 {
                runs.push(style.clone().to_run(marked.start));
            }

            let mut marked_style = style.clone();
            marked_style.underline = Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(window.text_style().color),
                wavy: false,
            });
            runs.push(marked_style.to_run(marked.len()));

            if marked.end < run_len {
                runs.push(style.to_run(run_len - marked.end));
            }
        } else {
            runs.push(style.to_run(run_len));
        }

        self.styled_text = StyledText::new(render_text.clone()).with_runs(runs);
        let (layout_id, _) =
            self.styled_text
                .request_layout(global_element_id, inspector_id, window, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.styled_text
            .prepaint(id, inspector_id, bounds, &mut (), window, cx);

        let text_layout = self.styled_text.layout().clone();
        let row = self.row;
        let text = self.text.clone();
        self.state.update(cx, |state, _| {
            if state.layout_cache.len() <= row {
                state.layout_cache.resize_with(row + 1, || None);
            }
            state.layout_cache[row] = Some(LineLayoutCache {
                bounds,
                text_layout: text_layout.clone(),
                text,
            });
        });

        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let hitbox = prepaint.clone();
        window.set_cursor_style(CursorStyle::IBeam, &hitbox);

        self.styled_text
            .paint(global_id, None, bounds, &mut (), &mut (), window, cx);

        // Paint selection/caret for this row.
        let (selection, is_focused) = {
            let state = self.state.read(cx);
            (
                state.editor.selection().clone(),
                state.focus_handle.is_focused(window),
            )
        };
        let path = vec![self.row, 0];
        let layout = self.styled_text.layout().clone();
        let line_height = layout.line_height();

        if selection.is_collapsed() {
            if selection.focus.path == path && is_focused {
                let caret_ix = selection.focus.offset.min(layout.len());
                if let Some(pos) = layout
                    .position_for_index(caret_ix)
                    .or_else(|| layout.position_for_index(layout.len()))
                {
                    window.paint_quad(gpui::quad(
                        Bounds::from_corners(pos, point(pos.x + px(1.5), pos.y + line_height)),
                        px(0.),
                        window.text_style().color,
                        gpui::Edges::default(),
                        gpui::transparent_black(),
                        gpui::BorderStyle::default(),
                    ));
                }
            }
            return;
        }

        // Highlight selections over root paragraph text nodes (`[row, 0]`).
        let mut start = selection.anchor.clone();
        let mut end = selection.focus.clone();
        if start.path == end.path {
            if end.offset < start.offset {
                std::mem::swap(&mut start, &mut end);
            }
        } else if end.path < start.path {
            std::mem::swap(&mut start, &mut end);
        }
        if start.path.len() != 2 || end.path.len() != 2 || start.path[1] != 0 || end.path[1] != 0 {
            return;
        }

        let start_row = start.path[0];
        let end_row = end.path[0];
        if self.row < start_row || self.row > end_row {
            return;
        }

        let row_len = self.text.len();
        let (a, b) = if start_row == end_row {
            (start.offset.min(row_len), end.offset.min(row_len))
        } else if self.row == start_row {
            (start.offset.min(row_len), row_len)
        } else if self.row == end_row {
            (0, end.offset.min(row_len))
        } else {
            (0, row_len)
        };

        if a >= b {
            return;
        }

        if let (Some(start_pos), Some(end_pos)) = (
            layout.position_for_index(a.min(layout.len())),
            layout
                .position_for_index(b.min(layout.len()))
                .or_else(|| layout.position_for_index(layout.len())),
        ) {
            Self::paint_selection_positions(
                start_pos,
                end_pos,
                &bounds,
                line_height,
                window,
                cx.theme().selection,
            );
        }
    }
}

pub(crate) struct RichTextNextInputHandlerElement {
    state: Entity<RichTextNextState>,
}

impl RichTextNextInputHandlerElement {
    pub(crate) fn new(state: Entity<RichTextNextState>) -> Self {
        Self { state }
    }
}

impl IntoElement for RichTextNextInputHandlerElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for RichTextNextInputHandlerElement {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = gpui::Style::default();
        style.size.width = gpui::relative(1.).into();
        style.size.height = gpui::relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.state.update(cx, |state, _| {
            state.viewport_bounds = bounds;
        });
        // Mirror the existing rich text editor behavior: capture mouse events for selection,
        // but allow scrolling to propagate to the underlying scroll area.
        window.insert_hitbox(bounds, HitboxBehavior::BlockMouseExceptScroll)
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.state.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.state.clone()),
            cx,
        );

        window.set_cursor_style(CursorStyle::IBeam, prepaint);

        // Mouse down focuses and sets caret.
        window.on_mouse_event({
            let state = self.state.clone();
            let hitbox = prepaint.clone();
            move |event: &MouseDownEvent, phase, window, cx| {
                if !phase.bubble() || event.button != MouseButton::Left {
                    return;
                }
                if !hitbox.is_hovered(window) {
                    return;
                }

                let focus_handle = { state.read(cx).focus_handle.clone() };
                window.focus(&focus_handle);

                state.update(cx, |this, cx| {
                    let Some(row) = this.paragraph_row_for_point(event.position) else {
                        return;
                    };

                    let offset = this
                        .offset_for_point(row, event.position)
                        .unwrap_or_else(|| this.block_text_len(row));
                    let focus = CorePoint::new(vec![row, 0], offset);

                    if event.click_count >= 3 {
                        let text = this.paragraph_text(row).unwrap_or_default();
                        let range = RichTextNextState::line_range(text.as_str(), offset);
                        let anchor = CorePoint::new(vec![row, 0], range.start);
                        let focus = CorePoint::new(vec![row, 0], range.end);
                        this.set_selection_points(anchor, focus, cx);
                        // Do not enter "drag selecting" mode on multi-click. This avoids tiny mouse
                        // movements between down/up being interpreted as a drag that expands the
                        // selection across paragraphs (which can look like "select all").
                        this.selecting = false;
                        this.selection_anchor = None;
                        return;
                    }

                    if event.click_count == 2 {
                        let text = this.paragraph_text(row).unwrap_or_default();
                        let range = RichTextNextState::word_range(text.as_str(), offset);
                        let anchor = CorePoint::new(vec![row, 0], range.start);
                        let focus = CorePoint::new(vec![row, 0], range.end);
                        this.set_selection_points(anchor.clone(), focus, cx);
                        this.selecting = false;
                        this.selection_anchor = None;
                        return;
                    }

                    let anchor = if event.modifiers.shift {
                        this.editor.selection().anchor.clone()
                    } else {
                        focus.clone()
                    };

                    this.set_selection_points(anchor.clone(), focus, cx);
                    this.selecting = true;
                    this.selection_anchor = Some(anchor);
                });
            }
        });

        window.on_mouse_event({
            let state = self.state.clone();
            move |event: &MouseMoveEvent, _phase, _window, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    return;
                }
                state.update(cx, |this, cx| {
                    if !this.selecting {
                        return;
                    }
                    let Some(row) = this.paragraph_row_for_point(event.position) else {
                        return;
                    };
                    let offset = this
                        .offset_for_point(row, event.position)
                        .unwrap_or_else(|| this.block_text_len(row));
                    let focus = CorePoint::new(vec![row, 0], offset);
                    let anchor = this
                        .selection_anchor
                        .clone()
                        .unwrap_or_else(|| focus.clone());
                    this.set_selection_points(anchor, focus, cx);
                });
            }
        });

        window.on_mouse_event({
            let state = self.state.clone();
            move |event: &MouseUpEvent, _phase, _window, cx| {
                if event.button != MouseButton::Left {
                    return;
                }
                state.update(cx, |this, cx| {
                    this.selecting = false;
                    this.selection_anchor = None;
                    cx.notify();
                });
            }
        });
    }
}

impl Render for RichTextNextState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let state = cx.entity().clone();

        if !self.did_auto_focus && !window.is_inspector_picking(cx) {
            window.focus(&self.focus_handle);
            self.did_auto_focus = true;
        }

        let doc_len = self.editor.doc().children.len();
        if self.layout_cache.len() > doc_len {
            self.layout_cache.truncate(doc_len);
        } else if self.layout_cache.len() < doc_len {
            self.layout_cache.resize_with(doc_len, || None);
        }

        let mut blocks: Vec<AnyElement> = Vec::new();
        for (row, node) in self.editor.doc().children.iter().enumerate() {
            match node {
                Node::Element(el) if el.kind == "paragraph" => {
                    blocks
                        .push(RichTextNextLineElement::new(state.clone(), row).into_any_element());
                }
                Node::Void(v) if v.kind == "divider" => {
                    blocks.push(
                        div()
                            .py(px(8.))
                            .child(div().w_full().h(px(1.)).bg(theme.border))
                            .into_any_element(),
                    );
                }
                _ => {
                    blocks.push(
                        div()
                            .text_color(theme.muted_foreground)
                            .italic()
                            .child(format!("<unknown node at {}>", row))
                            .into_any_element(),
                    );
                }
            }
        }

        div()
            .id(("richtext-next", cx.entity_id()))
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .tab_index(0)
            .w_full()
            .h_full()
            .relative()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded(theme.radius)
            .when(!window.is_inspector_picking(cx), |this| {
                this.on_action(window.listener_for(&state, RichTextNextState::backspace))
                    .on_action(window.listener_for(&state, RichTextNextState::delete))
                    .on_action(window.listener_for(&state, RichTextNextState::enter))
                    .on_action(window.listener_for(&state, RichTextNextState::left))
                    .on_action(window.listener_for(&state, RichTextNextState::right))
                    .on_action(window.listener_for(&state, RichTextNextState::select_left))
                    .on_action(window.listener_for(&state, RichTextNextState::select_right))
                    .on_action(window.listener_for(&state, RichTextNextState::undo))
                    .on_action(window.listener_for(&state, RichTextNextState::redo))
                    .on_action(window.listener_for(&state, RichTextNextState::insert_divider))
            })
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .child(RichTextNextInputHandlerElement::new(state.clone())),
            )
            .child(
                div()
                    .id("scroll-area")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .child(div().p(px(12.)).flex_col().gap(px(6.)).children(blocks)),
            )
    }
}

fn utf16_to_byte(s: &str, utf16_ix: usize) -> usize {
    if utf16_ix == 0 {
        return 0;
    }
    let mut utf16_count = 0usize;
    for (byte_ix, ch) in s.char_indices() {
        if utf16_count >= utf16_ix {
            return byte_ix;
        }
        utf16_count += ch.len_utf16();
    }
    s.len()
}

fn byte_to_utf16(s: &str, byte_ix: usize) -> usize {
    let byte_ix = byte_ix.min(s.len());
    let mut utf16_count = 0usize;
    for (ix, ch) in s.char_indices() {
        if ix >= byte_ix {
            break;
        }
        utf16_count += ch.len_utf16();
    }
    utf16_count
}

fn byte_to_utf16_range(s: &str, range: Range<usize>) -> Range<usize> {
    byte_to_utf16(s, range.start)..byte_to_utf16(s, range.end)
}

pub struct RichTextNextExample {
    app_menu_bar: Entity<AppMenuBar>,
    editor: Entity<RichTextNextState>,
    file_path: Option<PathBuf>,
}

impl RichTextNextExample {
    pub fn new(
        app_menu_bar: Entity<AppMenuBar>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor = cx.new(|cx| RichTextNextState::new(window, cx));
        // Ensure typing works immediately without a first click.
        let focus_handle = editor.read(cx).focus_handle();
        window.focus(&focus_handle);
        Self {
            app_menu_bar,
            editor,
            file_path: None,
        }
    }

    pub fn view(
        app_menu_bar: Entity<AppMenuBar>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| Self::new(app_menu_bar, window, cx))
    }

    fn open_from_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open Plate JSON".into()),
        });

        let editor = self.editor.clone();
        let this = cx.entity();
        cx.spawn_in(window, async move |_, window| {
            let path: PathBuf = picked.await.ok()?.ok()??.into_iter().next()?;
            let content = std::fs::read_to_string(&path).ok()?;
            let parsed = PlateValue::from_json_str(&content).ok();
            window
                .update(|window, cx| match parsed {
                    Some(value) => {
                        _ = this.update(cx, |this, _| this.file_path = Some(path.clone()));
                        _ = editor.update(cx, |editor, cx| editor.load_plate_value(value, cx));
                    }
                    None => {
                        window.push_notification(
                            Notification::new().message("Failed to parse JSON"),
                            cx,
                        );
                    }
                })
                .ok();
            Some(())
        })
        .detach();
    }

    fn save_to_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.editor.read(cx).plate_value();
        let json = value.to_json_pretty().unwrap_or_else(|_| "{}".to_string());

        if let Some(path) = self.file_path.clone() {
            if std::fs::write(&path, json).is_ok() {
                window.push_notification(
                    Notification::new().message(format!("Saved to {}", path.display())),
                    cx,
                );
            }
            return;
        }

        self.save_as(window, cx);
    }

    fn save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.editor.read(cx).plate_value();
        let json = value.to_json_pretty().unwrap_or_else(|_| "{}".to_string());

        let directory = self
            .file_path
            .as_ref()
            .and_then(|path| path.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let suggested_name = self
            .file_path
            .as_ref()
            .and_then(|p| p.file_name().map(|name| name.to_string_lossy().to_string()))
            .unwrap_or_else(|| "plate.json".to_string());

        let picked = cx.prompt_for_new_path(&directory, Some(&suggested_name));
        let this = cx.entity();

        cx.spawn_in(window, async move |_, window| {
            let path: PathBuf = picked.await.ok()?.ok()??;
            match std::fs::write(&path, json) {
                Ok(()) => {
                    window
                        .update(|window, cx| {
                            _ = this.update(cx, |this, _| this.file_path = Some(path.clone()));
                            window.push_notification(
                                Notification::new().message(format!("Saved to {}", path.display())),
                                cx,
                            );
                        })
                        .ok();
                }
                Err(err) => {
                    window
                        .update(|window, cx| {
                            window.push_notification(
                                Notification::new().message(format!("Failed to save: {err}")),
                                cx,
                            );
                        })
                        .ok();
                }
            }
            Some(())
        })
        .detach();
    }
}

impl Render for RichTextNextExample {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let can_undo = self.editor.read(cx).can_undo();
        let can_redo = self.editor.read(cx).can_redo();

        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(theme.muted)
            .on_action(cx.listener(|this, _: &Open, window, cx| {
                this.open_from_file(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Save, window, cx| {
                this.save_to_file(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveAs, window, cx| {
                this.save_as(window, cx);
            }))
            .on_action(cx.listener(|_, _: &About, window, cx| {
                window.push_notification(
                    Notification::new()
                        .message("Manos Rich Text (Next Core)")
                        .autohide(true),
                    cx,
                );
            }))
            // The app menu currently binds Edit actions to the legacy richtext action types.
            // Handle them here so menu items work for this example too.
            .on_action(
                cx.listener(|this, _: &gpui_manos_plate::Undo, _window, cx| {
                    this.editor.update(cx, |ed, cx| ed.command_undo(cx));
                }),
            )
            .on_action(
                cx.listener(|this, _: &gpui_manos_plate::Redo, _window, cx| {
                    this.editor.update(cx, |ed, cx| ed.command_redo(cx));
                }),
            )
            .on_action(
                cx.listener(|this, _: &gpui_manos_plate::Copy, _window, cx| {
                    this.editor.update(cx, |ed, cx| ed.command_copy(cx));
                }),
            )
            .on_action(cx.listener(|this, _: &gpui_manos_plate::Cut, _window, cx| {
                this.editor.update(cx, |ed, cx| ed.command_cut(cx));
            }))
            .on_action(
                cx.listener(|this, _: &gpui_manos_plate::Paste, _window, cx| {
                    this.editor.update(cx, |ed, cx| ed.command_paste(cx));
                }),
            )
            .on_action(
                cx.listener(|this, _: &gpui_manos_plate::SelectAll, _window, cx| {
                    this.editor.update(cx, |ed, cx| ed.command_select_all(cx));
                }),
            )
            .child(
                TitleBar::new().child(div().flex().items_center().child(self.app_menu_bar.clone())),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .p(px(8.))
                    .bg(theme.background)
                    .border_b_1()
                    .border_color(theme.border)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        PlateToolbarIconButton::new("undo", PlateIconName::Undo2)
                            .disabled(!can_undo)
                            .tooltip("Undo")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor.update(cx, |ed, cx| ed.command_undo(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("redo", PlateIconName::Redo2)
                            .disabled(!can_redo)
                            .tooltip("Redo")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor.update(cx, |ed, cx| ed.command_redo(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(PlateToolbarSeparator)
                    .child(
                        PlateToolbarIconButton::new("divider", PlateIconName::Minus)
                            .tooltip("Insert divider (Cmd/Ctrl+Shift+D)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_insert_divider(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(PlateToolbarSeparator)
                    .child(
                        Button::new("open")
                            .icon(IconName::FolderOpen)
                            .small()
                            .ghost()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_from_file(window, cx)),
                            ),
                    )
                    .child(Button::new("save").small().ghost().on_click(
                        cx.listener(|this, _, window, cx| this.save_to_file(window, cx)),
                    )),
            )
            .child(div().flex_1().min_h(px(0.)).child(self.editor.clone()))
    }
}
