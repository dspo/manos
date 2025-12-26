use std::collections::HashMap;
use std::ops::Range;

use gpui::actions;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::ActiveTheme as _;
use gpui_plate_core::{
    ChildConstraint, Document, Editor, ElementNode, Marks, Node, NodeRole, PlateValue,
    PluginRegistry, Point as CorePoint, Selection, TextNode,
};

pub(super) const CONTEXT: &str = "RichText";

actions!(
    rich_text,
    [
        Backspace,
        Delete,
        Enter,
        MoveLeft,
        MoveRight,
        SelectLeft,
        SelectRight,
        SelectAll,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        InsertDivider,
        InsertMention,
        ToggleBold,
        ToggleItalic,
        ToggleUnderline,
        ToggleStrikethrough,
        ToggleCode,
        ToggleBulletedList,
        ToggleOrderedList,
    ]
);

pub(crate) fn init(cx: &mut App) {
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
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-m", InsertMention, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-m", InsertMention, Some(CONTEXT)),
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
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-e", ToggleCode, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-e", ToggleCode, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-8", ToggleBulletedList, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-8", ToggleBulletedList, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-7", ToggleOrderedList, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-7", ToggleOrderedList, Some(CONTEXT)),
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
        KeyBinding::new("cmd-a", SelectAll, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-a", SelectAll, Some(CONTEXT)),
    ]);
}

fn node_at_path<'a>(doc: &'a Document, path: &[usize]) -> Option<&'a Node> {
    if path.is_empty() {
        return None;
    }

    let mut node = doc.children.get(path[0])?;
    for &ix in path.iter().skip(1) {
        node = match node {
            Node::Element(el) => el.children.get(ix)?,
            Node::Text(_) | Node::Void(_) => return None,
        };
    }
    Some(node)
}

fn element_at_path<'a>(doc: &'a Document, path: &[usize]) -> Option<&'a ElementNode> {
    match node_at_path(doc, path)? {
        Node::Element(el) => Some(el),
        _ => None,
    }
}

fn is_text_block_kind(registry: &PluginRegistry, kind: &str) -> bool {
    registry
        .node_specs()
        .get(kind)
        .map(|spec| {
            spec.role == NodeRole::Block
                && !spec.is_void
                && spec.children == ChildConstraint::InlineOnly
        })
        .unwrap_or(false)
}

fn text_block_paths(doc: &Document, registry: &PluginRegistry) -> Vec<Vec<usize>> {
    fn walk(
        nodes: &[Node],
        path: &mut Vec<usize>,
        out: &mut Vec<Vec<usize>>,
        registry: &PluginRegistry,
    ) {
        for (ix, node) in nodes.iter().enumerate() {
            let Node::Element(el) = node else {
                continue;
            };

            path.push(ix);

            if is_text_block_kind(registry, &el.kind) {
                out.push(path.clone());
            } else {
                walk(&el.children, path, out, registry);
            }

            path.pop();
        }
    }

    let mut out = Vec::new();
    walk(&doc.children, &mut Vec::new(), &mut out, registry);
    out
}

#[derive(Clone)]
pub enum InlineTextSegmentKind {
    Text,
    Void { kind: SharedString },
}

#[derive(Clone)]
pub struct InlineTextSegment {
    pub child_ix: usize,
    pub start: usize,
    pub len: usize,
    pub marks: Marks,
    pub kind: InlineTextSegmentKind,
}

#[derive(Clone)]
pub struct LineLayoutCache {
    pub bounds: Bounds<Pixels>,
    pub text_layout: gpui::TextLayout,
    pub text: SharedString,
    pub segments: Vec<InlineTextSegment>,
}

pub struct RichTextState {
    focus_handle: FocusHandle,
    scroll_handle: ScrollHandle,
    editor: Editor,
    selecting: bool,
    selection_anchor: Option<CorePoint>,
    viewport_bounds: Bounds<Pixels>,
    layout_cache: HashMap<Vec<usize>, LineLayoutCache>,
    text_block_order: Vec<Vec<usize>>,
    ime_marked_range: Option<Range<usize>>,
    did_auto_focus: bool,
}

impl RichTextState {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle().tab_stop(true);
        let editor = Editor::with_richtext_plugins();
        let text_block_order = text_block_paths(editor.doc(), editor.registry());
        Self {
            focus_handle,
            scroll_handle: ScrollHandle::new(),
            editor,
            selecting: false,
            selection_anchor: None,
            viewport_bounds: Bounds::default(),
            layout_cache: HashMap::new(),
            text_block_order,
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
            gpui_plate_core::PluginRegistry::richtext(),
        );
        self.scroll_handle.set_offset(point(px(0.), px(0.)));
        self.layout_cache.clear();
        self.text_block_order = text_block_paths(self.editor.doc(), self.editor.registry());
        self.ime_marked_range = None;
        cx.notify();
    }

    fn active_text_block_path(&self) -> Option<Vec<usize>> {
        self.editor
            .selection()
            .focus
            .path
            .split_last()
            .map(|(_, p)| p.to_vec())
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
        let Some(block_path) = self.active_text_block_path() else {
            return SharedString::default();
        };
        if let Some(cache) = self.layout_cache.get(&block_path) {
            return cache.text.clone();
        }
        self.text_block_text(&block_path).unwrap_or_default()
    }

    pub fn active_marks(&self) -> Marks {
        let path = &self.editor.selection().focus.path;
        match node_at_path(self.editor.doc(), path) {
            Some(Node::Text(t)) => t.marks.clone(),
            _ => Marks::default(),
        }
    }

    fn row_offset_for_point(&self, point: &CorePoint) -> Option<usize> {
        let (child_ix, block_path) = point.path.split_last()?;
        let Some(el) = element_at_path(self.editor.doc(), block_path) else {
            return None;
        };
        if !is_text_block_kind(self.editor.registry(), &el.kind) {
            return None;
        }

        let mut global = 0usize;
        for (ix, child) in el.children.iter().enumerate() {
            match child {
                Node::Text(t) => {
                    if ix == *child_ix {
                        return Some(global + point.offset.min(t.text.len()));
                    }
                    global += t.text.len();
                }
                Node::Void(v) => {
                    global += v.inline_text_len();
                }
                Node::Element(_) => {}
            }
        }

        Some(global)
    }

    fn point_for_block_offset(&self, block_path: &[usize], offset: usize) -> Option<CorePoint> {
        let el = element_at_path(self.editor.doc(), block_path)?;
        if !is_text_block_kind(self.editor.registry(), &el.kind) {
            return None;
        }

        Some(Self::point_for_block_offset_in_children(
            block_path,
            &el.children,
            offset,
        ))
    }

    fn active_list_type(&self) -> Option<SharedString> {
        self.editor
            .run_query::<Option<String>>("list.active_type", None)
            .ok()
            .flatten()
            .map(SharedString::new)
    }

    fn set_selection_in_active_block(
        &mut self,
        anchor: usize,
        focus: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let anchor_point = self
            .point_for_block_offset(&block_path, anchor)
            .unwrap_or_else(|| {
                let mut path = block_path.clone();
                path.push(0);
                CorePoint::new(path, 0)
            });
        let focus_point = self
            .point_for_block_offset(&block_path, focus)
            .unwrap_or_else(|| {
                let mut path = block_path.clone();
                path.push(0);
                CorePoint::new(path, 0)
            });
        let sel = Selection {
            anchor: anchor_point,
            focus: focus_point,
        };
        self.editor.set_selection(sel);
        _ = self.scroll_cursor_into_view();
        self.ime_marked_range = None;
        cx.notify();
    }

    fn ordered_active_range(&self) -> Range<usize> {
        let sel = self.editor.selection();
        let Some(active_block) = self.active_text_block_path() else {
            return 0..0;
        };
        let anchor_block = sel.anchor.path.split_last().map(|(_, p)| p);
        let focus_block = sel.focus.path.split_last().map(|(_, p)| p);
        if anchor_block != Some(active_block.as_slice())
            || focus_block != Some(active_block.as_slice())
        {
            let focus = self.row_offset_for_point(&sel.focus).unwrap_or(0);
            return focus..focus;
        }
        let mut a = self.row_offset_for_point(&sel.anchor).unwrap_or(0);
        let mut b = self.row_offset_for_point(&sel.focus).unwrap_or(0);
        if b < a {
            std::mem::swap(&mut a, &mut b);
        }
        a..b
    }

    fn text_block_text(&self, block_path: &[usize]) -> Option<SharedString> {
        let el = element_at_path(self.editor.doc(), block_path)?;
        if !is_text_block_kind(self.editor.registry(), &el.kind) {
            return None;
        }
        let mut out = String::new();
        for child in &el.children {
            match child {
                Node::Text(t) => out.push_str(&t.text),
                Node::Void(v) => out.push_str(&v.inline_text()),
                Node::Element(_) => {}
            }
        }
        Some(out.into())
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

    fn inline_void_ranges_in_block(&self, block_path: &[usize]) -> Vec<Range<usize>> {
        let Some(el) = element_at_path(self.editor.doc(), block_path) else {
            return Vec::new();
        };
        if !is_text_block_kind(self.editor.registry(), &el.kind) {
            return Vec::new();
        }

        let mut ranges: Vec<Range<usize>> = Vec::new();
        let mut cursor = 0usize;
        for child in &el.children {
            match child {
                Node::Text(t) => cursor += t.text.len(),
                Node::Void(v) => {
                    let len = v.inline_text_len();
                    if len > 0 {
                        ranges.push(cursor..cursor + len);
                        cursor += len;
                    }
                }
                Node::Element(_) => {}
            }
        }
        ranges
    }

    fn prev_cursor_offset_in_block(
        &self,
        block_path: &[usize],
        text: &str,
        offset: usize,
    ) -> usize {
        if offset == 0 {
            return 0;
        }

        let void_ranges = self.inline_void_ranges_in_block(block_path);
        for range in &void_ranges {
            if offset > range.start && offset <= range.end {
                return range.start;
            }
        }

        let mut prev = Self::prev_boundary(text, offset);
        for range in &void_ranges {
            if prev >= range.start && prev < range.end {
                prev = range.start;
                break;
            }
        }
        prev
    }

    fn next_cursor_offset_in_block(
        &self,
        block_path: &[usize],
        text: &str,
        offset: usize,
    ) -> usize {
        if offset >= text.len() {
            return text.len();
        }

        let void_ranges = self.inline_void_ranges_in_block(block_path);
        for range in &void_ranges {
            if offset >= range.start && offset < range.end {
                return range.end;
            }
        }

        let mut next = Self::next_boundary(text, offset);
        for range in &void_ranges {
            if next > range.start && next <= range.end {
                next = range.end;
                break;
            }
        }
        next
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

    fn word_or_void_range_in_block(
        &self,
        block_path: &[usize],
        text: &str,
        offset: usize,
    ) -> Range<usize> {
        for range in self.inline_void_ranges_in_block(block_path) {
            if offset >= range.start && offset < range.end {
                return range;
            }
        }
        Self::word_range(text, offset)
    }

    fn clamp_to_char_boundary(s: &str, mut ix: usize) -> usize {
        ix = ix.min(s.len());
        while ix > 0 && !s.is_char_boundary(ix) {
            ix -= 1;
        }
        ix
    }

    fn split_inline_children_at_offset(children: &[Node], offset: usize) -> (Vec<Node>, Vec<Node>) {
        let mut left: Vec<Node> = Vec::new();
        let mut right: Vec<Node> = Vec::new();

        let mut cursor = 0usize;
        let mut splitting = false;

        for child in children {
            if splitting {
                right.push(child.clone());
                continue;
            }

            match child {
                Node::Text(text) => {
                    let len = text.text.len();
                    if cursor + len <= offset {
                        left.push(Node::Text(text.clone()));
                        cursor += len;
                        continue;
                    }

                    if cursor >= offset {
                        splitting = true;
                        right.push(Node::Text(text.clone()));
                        continue;
                    }

                    let local = offset.saturating_sub(cursor).min(len);
                    let local = Self::clamp_to_char_boundary(&text.text, local);

                    let prefix = text.text.get(..local).unwrap_or("").to_string();
                    let suffix = text.text.get(local..).unwrap_or("").to_string();

                    if !prefix.is_empty() {
                        left.push(Node::Text(TextNode {
                            text: prefix,
                            marks: text.marks.clone(),
                        }));
                    }
                    if !suffix.is_empty() {
                        right.push(Node::Text(TextNode {
                            text: suffix,
                            marks: text.marks.clone(),
                        }));
                    }
                    splitting = true;
                }
                Node::Void(v) => {
                    let len = v.inline_text_len();
                    if cursor + len <= offset {
                        left.push(child.clone());
                        cursor += len;
                        continue;
                    }

                    if cursor >= offset {
                        splitting = true;
                        right.push(child.clone());
                        continue;
                    }

                    // Offset fell "inside" a void segment. Snap to the nearest boundary so the
                    // void stays atomic.
                    let local = offset.saturating_sub(cursor).min(len);
                    if local <= len / 2 {
                        splitting = true;
                        right.push(child.clone());
                    } else {
                        left.push(child.clone());
                        cursor += len;
                        splitting = true;
                    }
                }
                Node::Element(_) => {
                    left.push(child.clone());
                }
            }
        }

        (left, right)
    }

    fn merge_adjacent_text_nodes(nodes: Vec<Node>) -> Vec<Node> {
        let mut out: Vec<Node> = Vec::new();
        for node in nodes {
            match (&mut out.last_mut(), &node) {
                (Some(Node::Text(prev)), Node::Text(next)) if prev.marks == next.marks => {
                    prev.text.push_str(&next.text);
                }
                _ => out.push(node),
            }
        }
        out
    }

    fn ensure_has_text_leaf(mut children: Vec<Node>, marks: &Marks) -> Vec<Node> {
        let has_text = children.iter().any(|n| matches!(n, Node::Text(_)));
        if !has_text {
            children.push(Node::Text(TextNode {
                text: String::new(),
                marks: marks.clone(),
            }));
        }
        children
    }

    fn point_for_block_offset_in_children(
        block_path: &[usize],
        children: &[Node],
        offset: usize,
    ) -> CorePoint {
        let mut text_nodes: Vec<(usize, usize)> = Vec::new();
        for (child_ix, child) in children.iter().enumerate() {
            if let Node::Text(t) = child {
                text_nodes.push((child_ix, t.text.len()));
            }
        }
        if text_nodes.is_empty() {
            let mut path = block_path.to_vec();
            path.push(0);
            return CorePoint::new(path, 0);
        }

        let total: usize = children
            .iter()
            .map(|node| match node {
                Node::Text(t) => t.text.len(),
                Node::Void(v) => v.inline_text_len(),
                Node::Element(_) => 0,
            })
            .sum();

        let mut remaining = offset.min(total);
        for (child_ix, child) in children.iter().enumerate() {
            match child {
                Node::Text(t) => {
                    let len = t.text.len();
                    if remaining < len {
                        let mut path = block_path.to_vec();
                        path.push(child_ix);
                        return CorePoint::new(path, remaining);
                    }
                    if remaining == len {
                        if matches!(children.get(child_ix + 1), Some(Node::Text(_))) {
                            let mut path = block_path.to_vec();
                            path.push(child_ix + 1);
                            return CorePoint::new(path, 0);
                        }
                        let mut path = block_path.to_vec();
                        path.push(child_ix);
                        return CorePoint::new(path, len);
                    }
                    remaining = remaining.saturating_sub(len);
                }
                Node::Void(v) => {
                    let len = v.inline_text_len();
                    if len == 0 {
                        continue;
                    }
                    if remaining < len {
                        let before = remaining;
                        let after = len - remaining;
                        if before <= after {
                            for (prev_ix, prev) in children.iter().enumerate().take(child_ix).rev()
                            {
                                if let Node::Text(t) = prev {
                                    let mut path = block_path.to_vec();
                                    path.push(prev_ix);
                                    return CorePoint::new(path, t.text.len());
                                }
                            }
                        }

                        for (next_ix, next) in children.iter().enumerate().skip(child_ix + 1) {
                            if matches!(next, Node::Text(_)) {
                                let mut path = block_path.to_vec();
                                path.push(next_ix);
                                return CorePoint::new(path, 0);
                            }
                        }

                        let (last_ix, last_len) = text_nodes.last().copied().unwrap();
                        let mut path = block_path.to_vec();
                        path.push(last_ix);
                        return CorePoint::new(path, last_len);
                    }

                    if remaining == len {
                        for (next_ix, next) in children.iter().enumerate().skip(child_ix + 1) {
                            if matches!(next, Node::Text(_)) {
                                let mut path = block_path.to_vec();
                                path.push(next_ix);
                                return CorePoint::new(path, 0);
                            }
                        }
                        let (last_ix, last_len) = text_nodes.last().copied().unwrap();
                        let mut path = block_path.to_vec();
                        path.push(last_ix);
                        return CorePoint::new(path, last_len);
                    }

                    remaining = remaining.saturating_sub(len);
                }
                Node::Element(_) => {}
            }
        }

        let (last_ix, last_len) = text_nodes.last().copied().unwrap();
        let mut path = block_path.to_vec();
        path.push(last_ix);
        CorePoint::new(path, last_len)
    }

    fn tx_replace_range_in_block(
        &self,
        block_path: &[usize],
        range: Range<usize>,
        inserted: &str,
        inserted_marks: &Marks,
        selection_after: Option<(usize, usize)>,
        source: &'static str,
    ) -> Option<gpui_plate_core::Transaction> {
        if inserted.contains('\n') {
            return None;
        }

        let el = element_at_path(self.editor.doc(), block_path)?;
        if !is_text_block_kind(self.editor.registry(), &el.kind) {
            return None;
        }

        let old_children = el.children.clone();
        let row_len = self.block_text_len_in_block(block_path);
        let start = range.start.min(row_len);
        let end = range.end.min(row_len).max(start);

        let (left, rest) = Self::split_inline_children_at_offset(&old_children, start);
        let (_removed, right) = Self::split_inline_children_at_offset(&rest, end - start);

        let mut new_children = left;
        if !inserted.is_empty() {
            new_children.push(Node::Text(TextNode {
                text: inserted.to_string(),
                marks: inserted_marks.clone(),
            }));
        }
        new_children.extend(right);
        new_children = Self::merge_adjacent_text_nodes(new_children);
        new_children = Self::ensure_has_text_leaf(new_children, inserted_marks);

        let (anchor, focus) = if let Some((a, b)) = selection_after {
            (
                Self::point_for_block_offset_in_children(block_path, &new_children, a),
                Self::point_for_block_offset_in_children(block_path, &new_children, b),
            )
        } else {
            let caret = start + inserted.len();
            let point = Self::point_for_block_offset_in_children(block_path, &new_children, caret);
            (point.clone(), point)
        };
        let selection_after = Selection { anchor, focus };

        let mut ops = Vec::new();
        for child_ix in (0..old_children.len()).rev() {
            let mut path = block_path.to_vec();
            path.push(child_ix);
            ops.push(gpui_plate_core::Op::RemoveNode { path });
        }
        for (child_ix, node) in new_children.into_iter().enumerate() {
            let mut path = block_path.to_vec();
            path.push(child_ix);
            ops.push(gpui_plate_core::Op::InsertNode { path, node });
        }

        Some(
            gpui_plate_core::Transaction::new(ops)
                .selection_after(selection_after)
                .source(source),
        )
    }

    fn tx_replace_range_with_multiline_text_in_block(
        &self,
        block_path: &[usize],
        range: Range<usize>,
        inserted: &str,
        inserted_marks: &Marks,
        source: &'static str,
    ) -> Option<gpui_plate_core::Transaction> {
        let el = element_at_path(self.editor.doc(), block_path)?;
        if !is_text_block_kind(self.editor.registry(), &el.kind) {
            return None;
        }

        let parts: Vec<&str> = inserted.split('\n').collect();
        if parts.len() <= 1 {
            return self.tx_replace_range_in_block(
                block_path,
                range,
                inserted,
                inserted_marks,
                None,
                source,
            );
        }

        let is_list_item = el.kind == "list_item";
        let block_attrs = el.attrs.clone();

        let old_children = el.children.clone();
        let row_len = self.block_text_len_in_block(block_path);
        let start = range.start.min(row_len);
        let end = range.end.min(row_len).max(start);

        let (left, rest) = Self::split_inline_children_at_offset(&old_children, start);
        let (_removed, right) = Self::split_inline_children_at_offset(&rest, end - start);

        let mut first_children = left;
        if !parts[0].is_empty() {
            first_children.push(Node::Text(TextNode {
                text: parts[0].to_string(),
                marks: inserted_marks.clone(),
            }));
        }
        first_children = Self::merge_adjacent_text_nodes(first_children);
        first_children = Self::ensure_has_text_leaf(first_children, inserted_marks);

        let last_ix = parts.len() - 1;
        let mut inserted_blocks: Vec<Node> = Vec::new();
        let mut last_children: Vec<Node> = Vec::new();

        for (i, part) in parts.iter().enumerate().skip(1) {
            let mut children: Vec<Node> = Vec::new();
            if !part.is_empty() {
                children.push(Node::Text(TextNode {
                    text: (*part).to_string(),
                    marks: inserted_marks.clone(),
                }));
            }
            if i == last_ix {
                children.extend(right.clone());
                children = Self::merge_adjacent_text_nodes(children);
                children = Self::ensure_has_text_leaf(children, inserted_marks);
                last_children = children.clone();
            } else {
                children = Self::merge_adjacent_text_nodes(children);
                children = Self::ensure_has_text_leaf(children, inserted_marks);
            }

            let node = if is_list_item {
                Node::Element(ElementNode {
                    kind: "list_item".to_string(),
                    attrs: block_attrs.clone(),
                    children,
                })
            } else {
                Node::Element(ElementNode {
                    kind: "paragraph".to_string(),
                    attrs: Default::default(),
                    children,
                })
            };
            inserted_blocks.push(node);
        }

        let mut ops = Vec::new();
        for child_ix in (0..old_children.len()).rev() {
            let mut path = block_path.to_vec();
            path.push(child_ix);
            ops.push(gpui_plate_core::Op::RemoveNode { path });
        }
        for (child_ix, node) in first_children.into_iter().enumerate() {
            let mut path = block_path.to_vec();
            path.push(child_ix);
            ops.push(gpui_plate_core::Op::InsertNode { path, node });
        }

        let (block_ix, parent_path) = block_path.split_last()?;
        for (i, node) in inserted_blocks.into_iter().enumerate() {
            let mut path = parent_path.to_vec();
            path.push(block_ix + i + 1);
            ops.push(gpui_plate_core::Op::InsertNode { path, node });
        }

        let mut last_block_path = parent_path.to_vec();
        last_block_path.push(block_ix + last_ix);
        let caret_offset = parts[last_ix].len();
        let caret_point = Self::point_for_block_offset_in_children(
            &last_block_path,
            &last_children,
            caret_offset,
        );

        Some(
            gpui_plate_core::Transaction::new(ops)
                .selection_after(Selection::collapsed(caret_point))
                .source(source),
        )
    }

    fn tx_insert_multiline_text_at_block_offset(
        &self,
        block_path: &[usize],
        offset: usize,
        inserted: &str,
        inserted_marks: &Marks,
        source: &'static str,
    ) -> Option<gpui_plate_core::Transaction> {
        self.tx_replace_range_with_multiline_text_in_block(
            block_path,
            offset..offset,
            inserted,
            inserted_marks,
            source,
        )
    }

    fn delete_selection_transaction(&self) -> Option<gpui_plate_core::Transaction> {
        let selection = self.editor.selection().clone();
        if selection.is_collapsed() {
            return None;
        }

        let mut start = selection.anchor.clone();
        let mut end = selection.focus.clone();
        if start.path == end.path {
            if end.offset < start.offset {
                std::mem::swap(&mut start, &mut end);
            }
        } else if end.path < start.path {
            std::mem::swap(&mut start, &mut end);
        }

        let start_offset = self.row_offset_for_point(&start)?;
        let end_offset = self.row_offset_for_point(&end)?;

        let start_block: Vec<usize> = start.path.split_last().map(|(_, p)| p.to_vec())?;
        let end_block: Vec<usize> = end.path.split_last().map(|(_, p)| p.to_vec())?;

        let marks = self.active_marks();

        if start_block == end_block {
            return self.tx_replace_range_in_block(
                &start_block,
                start_offset..end_offset,
                "",
                &marks,
                None,
                "selection:delete",
            );
        }

        let start_el = element_at_path(self.editor.doc(), &start_block)?;
        let end_el = element_at_path(self.editor.doc(), &end_block)?;
        if !is_text_block_kind(self.editor.registry(), &start_el.kind)
            || !is_text_block_kind(self.editor.registry(), &end_el.kind)
        {
            return None;
        }

        let (start_ix, start_parent) = start_block.split_last()?;
        let (end_ix, end_parent) = end_block.split_last()?;

        let list_compatible = if start_el.kind == "list_item" {
            if end_el.kind != "list_item" {
                false
            } else {
                let start_type = start_el.attrs.get("list_type").and_then(|v| v.as_str());
                let end_type = end_el.attrs.get("list_type").and_then(|v| v.as_str());
                let start_level = start_el
                    .attrs
                    .get("list_level")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let end_level = end_el
                    .attrs
                    .get("list_level")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                start_type == end_type && start_level == end_level
            }
        } else {
            start_el.kind == end_el.kind
        };

        let same_parent = start_parent == end_parent && *start_ix <= *end_ix;
        if list_compatible && same_parent {
            let start_children = start_el.children.clone();
            let end_children = end_el.children.clone();

            let (start_left, _start_right) =
                Self::split_inline_children_at_offset(&start_children, start_offset);
            let (_end_left, end_right) =
                Self::split_inline_children_at_offset(&end_children, end_offset);

            let mut merged_children = start_left;
            merged_children.extend(end_right);
            merged_children = Self::merge_adjacent_text_nodes(merged_children);
            merged_children = Self::ensure_has_text_leaf(merged_children, &marks);

            let selection_after = Selection::collapsed(Self::point_for_block_offset_in_children(
                &start_block,
                &merged_children,
                start_offset,
            ));

            let mut ops: Vec<gpui_plate_core::Op> = Vec::new();
            for child_ix in (0..start_children.len()).rev() {
                let mut path = start_block.clone();
                path.push(child_ix);
                ops.push(gpui_plate_core::Op::RemoveNode { path });
            }
            for (child_ix, node) in merged_children.into_iter().enumerate() {
                let mut path = start_block.clone();
                path.push(child_ix);
                ops.push(gpui_plate_core::Op::InsertNode { path, node });
            }

            for ix in (*start_ix + 1..=*end_ix).rev() {
                let mut path = start_parent.to_vec();
                path.push(ix);
                ops.push(gpui_plate_core::Op::RemoveNode { path });
            }

            return Some(
                gpui_plate_core::Transaction::new(ops)
                    .selection_after(selection_after)
                    .source("selection:delete_multi_block_merge"),
            );
        }

        let blocks = text_block_paths(self.editor.doc(), self.editor.registry());
        let start_pos = blocks.iter().position(|p| p == &start_block)?;
        let end_pos = blocks.iter().position(|p| p == &end_block)?;
        let (start_pos, end_pos) = if start_pos <= end_pos {
            (start_pos, end_pos)
        } else {
            (end_pos, start_pos)
        };

        let mut ops: Vec<gpui_plate_core::Op> = Vec::new();
        let mut selection_after: Option<Selection> = None;

        for (ix, block_path) in blocks.iter().enumerate().take(end_pos + 1).skip(start_pos) {
            let len = self.block_text_len_in_block(block_path);
            let range = if ix == start_pos {
                start_offset..len
            } else if ix == end_pos {
                0..end_offset.min(len)
            } else {
                0..len
            };
            if range.start >= range.end {
                continue;
            }

            let Some(tx) = self.tx_replace_range_in_block(
                block_path,
                range,
                "",
                &marks,
                None,
                "selection:delete:block",
            ) else {
                continue;
            };

            if selection_after.is_none() && ix == start_pos {
                selection_after = tx.selection_after.clone();
            }
            ops.extend(tx.ops);
        }

        let selection_after = selection_after.unwrap_or_else(|| {
            Selection::collapsed(
                self.point_for_block_offset(&start_block, start_offset)
                    .unwrap_or_else(|| CorePoint::new(start.path.clone(), start.offset)),
            )
        });

        Some(
            gpui_plate_core::Transaction::new(ops)
                .selection_after(selection_after)
                .source("selection:delete_multi_block_clear"),
        )
    }

    fn delete_selection_if_any(&mut self, cx: &mut Context<Self>) -> Option<CorePoint> {
        let tx = self.delete_selection_transaction()?;
        self.push_tx(tx, cx);
        Some(self.editor.selection().focus.clone())
    }

    fn tx_join_block_with_previous_in_block(
        &self,
        block_path: &[usize],
        source: &'static str,
    ) -> Option<gpui_plate_core::Transaction> {
        let (block_ix, parent_path) = block_path.split_last()?;
        if *block_ix == 0 {
            return None;
        }

        let mut prev_path = parent_path.to_vec();
        prev_path.push(block_ix.saturating_sub(1));

        let doc = self.editor.doc();
        let prev_el = element_at_path(doc, &prev_path)?;
        let cur_el = element_at_path(doc, block_path)?;

        if !is_text_block_kind(self.editor.registry(), &prev_el.kind)
            || !is_text_block_kind(self.editor.registry(), &cur_el.kind)
        {
            return None;
        }

        if cur_el.kind == "list_item" {
            if prev_el.kind != "list_item" {
                return None;
            }
            let cur_type = cur_el.attrs.get("list_type").and_then(|v| v.as_str());
            let prev_type = prev_el.attrs.get("list_type").and_then(|v| v.as_str());
            let cur_level = cur_el
                .attrs
                .get("list_level")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let prev_level = prev_el
                .attrs
                .get("list_level")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            if cur_type != prev_type || cur_level != prev_level {
                return None;
            }
        }

        let caret_offset = self.block_text_len_in_block(&prev_path);
        let marks = self.active_marks();

        let mut merged_children = prev_el.children.clone();
        merged_children.extend(cur_el.children.clone());
        merged_children = Self::merge_adjacent_text_nodes(merged_children);
        merged_children = Self::ensure_has_text_leaf(merged_children, &marks);

        let selection_after = Selection::collapsed(Self::point_for_block_offset_in_children(
            &prev_path,
            &merged_children,
            caret_offset,
        ));

        let mut ops: Vec<gpui_plate_core::Op> = Vec::new();
        for child_ix in (0..prev_el.children.len()).rev() {
            let mut path = prev_path.clone();
            path.push(child_ix);
            ops.push(gpui_plate_core::Op::RemoveNode { path });
        }
        for (child_ix, node) in merged_children.into_iter().enumerate() {
            let mut path = prev_path.clone();
            path.push(child_ix);
            ops.push(gpui_plate_core::Op::InsertNode { path, node });
        }
        ops.push(gpui_plate_core::Op::RemoveNode {
            path: block_path.to_vec(),
        });

        Some(
            gpui_plate_core::Transaction::new(ops)
                .selection_after(selection_after)
                .source(source),
        )
    }

    fn tx_join_block_with_next_in_block(
        &self,
        block_path: &[usize],
        source: &'static str,
    ) -> Option<gpui_plate_core::Transaction> {
        let (block_ix, parent_path) = block_path.split_last()?;
        let mut next_path = parent_path.to_vec();
        next_path.push(block_ix + 1);

        let doc = self.editor.doc();
        let cur_el = element_at_path(doc, block_path)?;
        let next_el = element_at_path(doc, &next_path)?;

        if !is_text_block_kind(self.editor.registry(), &cur_el.kind)
            || !is_text_block_kind(self.editor.registry(), &next_el.kind)
        {
            return None;
        }

        if cur_el.kind == "list_item" {
            if next_el.kind != "list_item" {
                return None;
            }
            let cur_type = cur_el.attrs.get("list_type").and_then(|v| v.as_str());
            let next_type = next_el.attrs.get("list_type").and_then(|v| v.as_str());
            let cur_level = cur_el
                .attrs
                .get("list_level")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let next_level = next_el
                .attrs
                .get("list_level")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            if cur_type != next_type || cur_level != next_level {
                return None;
            }
        }

        let caret_offset = self.block_text_len_in_block(block_path);
        let marks = self.active_marks();

        let mut merged_children = cur_el.children.clone();
        merged_children.extend(next_el.children.clone());
        merged_children = Self::merge_adjacent_text_nodes(merged_children);
        merged_children = Self::ensure_has_text_leaf(merged_children, &marks);

        let selection_after = Selection::collapsed(Self::point_for_block_offset_in_children(
            block_path,
            &merged_children,
            caret_offset,
        ));

        let mut ops: Vec<gpui_plate_core::Op> = Vec::new();
        for child_ix in (0..cur_el.children.len()).rev() {
            let mut path = block_path.to_vec();
            path.push(child_ix);
            ops.push(gpui_plate_core::Op::RemoveNode { path });
        }
        for (child_ix, node) in merged_children.into_iter().enumerate() {
            let mut path = block_path.to_vec();
            path.push(child_ix);
            ops.push(gpui_plate_core::Op::InsertNode { path, node });
        }
        ops.push(gpui_plate_core::Op::RemoveNode { path: next_path });

        Some(
            gpui_plate_core::Transaction::new(ops)
                .selection_after(selection_after)
                .source(source),
        )
    }

    fn prev_text_block_path(&self, block_path: &[usize]) -> Option<Vec<usize>> {
        let ix = self
            .text_block_order
            .iter()
            .position(|p| p.as_slice() == block_path)?;
        ix.checked_sub(1)
            .and_then(|prev| self.text_block_order.get(prev).cloned())
    }

    fn next_text_block_path(&self, block_path: &[usize]) -> Option<Vec<usize>> {
        let ix = self
            .text_block_order
            .iter()
            .position(|p| p.as_slice() == block_path)?;
        self.text_block_order.get(ix + 1).cloned()
    }

    fn offset_for_point_in_block(
        &self,
        block_path: &[usize],
        point: gpui::Point<Pixels>,
    ) -> Option<usize> {
        let cache = self.layout_cache.get(block_path)?;
        let mut local = match cache.text_layout.index_for_position(point) {
            Ok(ix) | Err(ix) => ix,
        };
        local = local.min(cache.text.len());
        Some(local)
    }

    fn link_at_offset(&self, block_path: &[usize], offset: usize) -> Option<String> {
        let cache = self.layout_cache.get(block_path)?;

        let candidates = [offset, offset.saturating_sub(1)];
        for candidate in candidates {
            for seg in &cache.segments {
                if seg.len == 0 {
                    continue;
                }
                if candidate >= seg.start && candidate < seg.start + seg.len {
                    return seg.marks.link.clone();
                }
            }
        }

        None
    }

    fn block_text_len_in_block(&self, block_path: &[usize]) -> usize {
        let Some(el) = element_at_path(self.editor.doc(), block_path) else {
            return 0;
        };
        if !is_text_block_kind(self.editor.registry(), &el.kind) {
            return 0;
        }

        el.children
            .iter()
            .map(|child| match child {
                Node::Text(t) => t.text.len(),
                Node::Void(v) => v.inline_text_len(),
                Node::Element(_) => 0,
            })
            .sum()
    }

    fn text_block_path_for_point(&self, point: gpui::Point<Pixels>) -> Option<Vec<usize>> {
        for (path, cache) in &self.layout_cache {
            if cache.bounds.contains(&point) {
                return Some(path.clone());
            }
        }

        let mut best: Option<(Pixels, Vec<usize>)> = None;
        for (path, cache) in &self.layout_cache {
            let dist = if point.y < cache.bounds.top() {
                cache.bounds.top() - point.y
            } else if point.y > cache.bounds.bottom() {
                point.y - cache.bounds.bottom()
            } else {
                px(0.)
            };
            match &best {
                None => best = Some((dist, path.clone())),
                Some((best_dist, _)) if dist < *best_dist => best = Some((dist, path.clone())),
                _ => {}
            }
        }
        best.map(|(_, path)| path)
    }

    fn push_tx(&mut self, tx: gpui_plate_core::Transaction, cx: &mut Context<Self>) {
        if self.editor.apply(tx).is_ok() {
            _ = self.scroll_cursor_into_view();
            self.layout_cache.clear();
            self.text_block_order = text_block_paths(self.editor.doc(), self.editor.registry());
            cx.notify();
        }
    }

    fn run_command_and_refresh(
        &mut self,
        id: &'static str,
        args: Option<serde_json::Value>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.editor.run_command(id, args).is_ok() {
            _ = self.scroll_cursor_into_view();
            self.layout_cache.clear();
            self.text_block_order = text_block_paths(self.editor.doc(), self.editor.registry());
            cx.notify();
            true
        } else {
            false
        }
    }

    fn scroll_cursor_into_view(&mut self) -> bool {
        let Some(block_path) = self.active_text_block_path() else {
            return false;
        };
        let Some(cache) = self.layout_cache.get(block_path.as_slice()) else {
            return false;
        };
        let viewport = self.viewport_bounds;
        if viewport.size.width == px(0.) && viewport.size.height == px(0.) {
            return false;
        }

        let caret_ix = self
            .row_offset_for_point(&self.editor.selection().focus)
            .unwrap_or(0)
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

        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let text = self.text_block_text(&block_path).unwrap_or_default();
        let cursor = self
            .row_offset_for_point(&self.editor.selection().focus)
            .unwrap_or(0)
            .min(text.len());
        let start = self.prev_cursor_offset_in_block(&block_path, text.as_str(), cursor);
        if start == cursor {
            if cursor == 0 {
                if let Some((block_ix, parent_path)) = block_path.split_last()
                    && *block_ix > 0
                {
                    let mut prev_path = parent_path.to_vec();
                    prev_path.push(block_ix.saturating_sub(1));

                    if matches!(
                        node_at_path(self.editor.doc(), &prev_path),
                        Some(Node::Void(v)) if v.kind == "divider"
                    ) {
                        self.push_tx(
                            gpui_plate_core::Transaction::new(vec![
                                gpui_plate_core::Op::RemoveNode { path: prev_path },
                            ])
                            .source("key:backspace:remove_divider"),
                            cx,
                        );
                        return;
                    }

                    if let Some(tx) =
                        self.tx_join_block_with_previous_in_block(&block_path, "key:backspace:join")
                    {
                        self.push_tx(tx, cx);
                        return;
                    }
                }

                if self.active_list_type().is_some() {
                    _ = self.run_command_and_refresh("list.unwrap", None, cx);
                }
            }
            return;
        }
        let marks = self.active_marks();
        if let Some(tx) = self.tx_replace_range_in_block(
            &block_path,
            start..cursor,
            "",
            &marks,
            None,
            "key:backspace",
        ) {
            self.push_tx(tx, cx);
        }
    }

    pub(super) fn delete(&mut self, _: &Delete, _window: &mut Window, cx: &mut Context<Self>) {
        if self.delete_selection_if_any(cx).is_some() {
            return;
        }

        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let text = self.text_block_text(&block_path).unwrap_or_default();
        let cursor = self
            .row_offset_for_point(&self.editor.selection().focus)
            .unwrap_or(0)
            .min(text.len());
        let end = self.next_cursor_offset_in_block(&block_path, text.as_str(), cursor);
        if end == cursor {
            if cursor == text.len()
                && let Some((block_ix, parent_path)) = block_path.split_last()
            {
                let mut next_path = parent_path.to_vec();
                next_path.push(block_ix + 1);

                if matches!(
                    node_at_path(self.editor.doc(), &next_path),
                    Some(Node::Void(v)) if v.kind == "divider"
                ) {
                    self.push_tx(
                        gpui_plate_core::Transaction::new(vec![gpui_plate_core::Op::RemoveNode {
                            path: next_path,
                        }])
                        .source("key:delete:remove_divider"),
                        cx,
                    );
                    return;
                }

                if let Some(tx) =
                    self.tx_join_block_with_next_in_block(&block_path, "key:delete:join")
                {
                    self.push_tx(tx, cx);
                }
            }
            return;
        }
        let marks = self.active_marks();
        if let Some(tx) =
            self.tx_replace_range_in_block(&block_path, cursor..end, "", &marks, None, "key:delete")
        {
            self.push_tx(tx, cx);
        }
    }

    pub(super) fn enter(&mut self, _: &Enter, _window: &mut Window, cx: &mut Context<Self>) {
        _ = self.delete_selection_if_any(cx);

        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let Some(el) = element_at_path(self.editor.doc(), &block_path) else {
            return;
        };
        let block_kind = el.kind.as_str();

        let text_len = self.block_text_len_in_block(&block_path);
        let cursor = self
            .row_offset_for_point(&self.editor.selection().focus)
            .unwrap_or(0)
            .min(text_len);

        if block_kind == "list_item" && text_len == 0 && cursor == 0 {
            _ = self.run_command_and_refresh("list.unwrap", None, cx);
            return;
        }

        let marks = self.active_marks();
        if let Some(tx) = self.tx_insert_multiline_text_at_block_offset(
            &block_path,
            cursor,
            "\n",
            &marks,
            if block_kind == "list_item" {
                "key:enter:split_list_item"
            } else {
                "key:enter:split_paragraph"
            },
        ) {
            self.push_tx(tx, cx);
        }
    }

    pub(super) fn left(&mut self, _: &MoveLeft, _window: &mut Window, cx: &mut Context<Self>) {
        let sel = self.editor.selection().clone();
        if !sel.is_collapsed() {
            let mut start = sel.anchor.clone();
            let mut end = sel.focus.clone();
            if start.path == end.path {
                if end.offset < start.offset {
                    std::mem::swap(&mut start, &mut end);
                }
            } else if end.path < start.path {
                std::mem::swap(&mut start, &mut end);
            }
            self.set_selection_points(start.clone(), start, cx);
            return;
        }
        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let text = self.text_block_text(&block_path).unwrap_or_default();
        let cursor = self
            .row_offset_for_point(&sel.focus)
            .unwrap_or(0)
            .min(text.len());
        if cursor == 0 {
            if let Some(prev_path) = self.prev_text_block_path(&block_path) {
                let offset = self.block_text_len_in_block(&prev_path);
                let point = self
                    .point_for_block_offset(&prev_path, offset)
                    .unwrap_or_else(|| {
                        let mut path = prev_path.clone();
                        path.push(0);
                        CorePoint::new(path, 0)
                    });
                self.set_selection_points(point.clone(), point, cx);
                return;
            }
        }
        let new_cursor = self.prev_cursor_offset_in_block(&block_path, text.as_str(), cursor);
        self.set_selection_in_active_block(new_cursor, new_cursor, cx);
    }

    pub(super) fn right(&mut self, _: &MoveRight, _window: &mut Window, cx: &mut Context<Self>) {
        let sel = self.editor.selection().clone();
        if !sel.is_collapsed() {
            let mut start = sel.anchor.clone();
            let mut end = sel.focus.clone();
            if start.path == end.path {
                if end.offset < start.offset {
                    std::mem::swap(&mut start, &mut end);
                }
            } else if end.path < start.path {
                std::mem::swap(&mut start, &mut end);
            }
            self.set_selection_points(end.clone(), end, cx);
            return;
        }
        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let text = self.text_block_text(&block_path).unwrap_or_default();
        let cursor = self
            .row_offset_for_point(&sel.focus)
            .unwrap_or(0)
            .min(text.len());
        if cursor == text.len() {
            if let Some(next_path) = self.next_text_block_path(&block_path) {
                let point = self
                    .point_for_block_offset(&next_path, 0)
                    .unwrap_or_else(|| {
                        let mut path = next_path.clone();
                        path.push(0);
                        CorePoint::new(path, 0)
                    });
                self.set_selection_points(point.clone(), point, cx);
                return;
            }
        }
        let new_cursor = self.next_cursor_offset_in_block(&block_path, text.as_str(), cursor);
        self.set_selection_in_active_block(new_cursor, new_cursor, cx);
    }

    pub(super) fn select_left(
        &mut self,
        _: &SelectLeft,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sel = self.editor.selection().clone();
        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let text = self.text_block_text(&block_path).unwrap_or_default();
        let cursor = self
            .row_offset_for_point(&sel.focus)
            .unwrap_or(0)
            .min(text.len());
        if cursor == 0 {
            if let Some(prev_path) = self.prev_text_block_path(&block_path) {
                let focus = self
                    .point_for_block_offset(&prev_path, self.block_text_len_in_block(&prev_path))
                    .unwrap_or_else(|| {
                        let mut path = prev_path.clone();
                        path.push(0);
                        CorePoint::new(path, 0)
                    });
                let anchor = if sel.is_collapsed() {
                    self.point_for_block_offset(&block_path, cursor)
                        .unwrap_or_else(|| CorePoint::new(sel.focus.path.clone(), sel.focus.offset))
                } else {
                    sel.anchor.clone()
                };
                self.set_selection_points(anchor, focus, cx);
                return;
            }
        }
        let new_cursor = self.prev_cursor_offset_in_block(&block_path, text.as_str(), cursor);
        let focus = self
            .point_for_block_offset(&block_path, new_cursor)
            .unwrap_or_else(|| CorePoint::new(sel.focus.path.clone(), sel.focus.offset));
        let anchor = if sel.is_collapsed() {
            self.point_for_block_offset(&block_path, cursor)
                .unwrap_or_else(|| CorePoint::new(sel.focus.path.clone(), sel.focus.offset))
        } else {
            sel.anchor.clone()
        };
        self.set_selection_points(anchor, focus, cx);
    }

    pub(super) fn select_right(
        &mut self,
        _: &SelectRight,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sel = self.editor.selection().clone();
        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let text = self.text_block_text(&block_path).unwrap_or_default();
        let cursor = self
            .row_offset_for_point(&sel.focus)
            .unwrap_or(0)
            .min(text.len());
        if cursor == text.len() {
            if let Some(next_path) = self.next_text_block_path(&block_path) {
                let focus = self
                    .point_for_block_offset(&next_path, 0)
                    .unwrap_or_else(|| {
                        let mut path = next_path.clone();
                        path.push(0);
                        CorePoint::new(path, 0)
                    });
                let anchor = if sel.is_collapsed() {
                    self.point_for_block_offset(&block_path, cursor)
                        .unwrap_or_else(|| CorePoint::new(sel.focus.path.clone(), sel.focus.offset))
                } else {
                    sel.anchor.clone()
                };
                self.set_selection_points(anchor, focus, cx);
                return;
            }
        }
        let new_cursor = self.next_cursor_offset_in_block(&block_path, text.as_str(), cursor);
        let focus = self
            .point_for_block_offset(&block_path, new_cursor)
            .unwrap_or_else(|| CorePoint::new(sel.focus.path.clone(), sel.focus.offset));
        let anchor = if sel.is_collapsed() {
            self.point_for_block_offset(&block_path, cursor)
                .unwrap_or_else(|| CorePoint::new(sel.focus.path.clone(), sel.focus.offset))
        } else {
            sel.anchor.clone()
        };
        self.set_selection_points(anchor, focus, cx);
    }

    pub(super) fn undo(&mut self, _: &Undo, _window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.undo() {
            self.layout_cache.clear();
            self.text_block_order = text_block_paths(self.editor.doc(), self.editor.registry());
            cx.notify();
        }
    }

    pub(super) fn redo(&mut self, _: &Redo, _window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.redo() {
            self.layout_cache.clear();
            self.text_block_order = text_block_paths(self.editor.doc(), self.editor.registry());
            cx.notify();
        }
    }

    pub(super) fn copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
        self.command_copy(cx);
    }

    pub(super) fn cut(&mut self, _: &Cut, _window: &mut Window, cx: &mut Context<Self>) {
        self.command_cut(cx);
    }

    pub(super) fn paste(&mut self, _: &Paste, _window: &mut Window, cx: &mut Context<Self>) {
        self.command_paste(cx);
    }

    pub(super) fn select_all(
        &mut self,
        _: &SelectAll,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.command_select_all(cx);
    }

    pub(super) fn insert_divider(
        &mut self,
        _: &InsertDivider,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.run_command_and_refresh("core.insert_divider", None, cx);
    }

    pub(super) fn insert_mention(
        &mut self,
        _: &InsertMention,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.delete_selection_if_any(cx);
        let args = serde_json::json!({ "label": "Alice" });
        _ = self.run_command_and_refresh("mention.insert", Some(args), cx);
    }

    pub(super) fn toggle_bold(
        &mut self,
        _: &ToggleBold,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.run_command_and_refresh("marks.toggle_bold", None, cx);
    }

    pub(super) fn toggle_italic(
        &mut self,
        _: &ToggleItalic,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.run_command_and_refresh("marks.toggle_italic", None, cx);
    }

    pub(super) fn toggle_underline(
        &mut self,
        _: &ToggleUnderline,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.run_command_and_refresh("marks.toggle_underline", None, cx);
    }

    pub(super) fn toggle_strikethrough(
        &mut self,
        _: &ToggleStrikethrough,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.run_command_and_refresh("marks.toggle_strikethrough", None, cx);
    }

    pub(super) fn toggle_code(
        &mut self,
        _: &ToggleCode,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.run_command_and_refresh("marks.toggle_code", None, cx);
    }

    pub(super) fn toggle_bulleted_list(
        &mut self,
        _: &ToggleBulletedList,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.run_command_and_refresh("list.toggle_bulleted", None, cx);
    }

    pub(super) fn toggle_ordered_list(
        &mut self,
        _: &ToggleOrderedList,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.run_command_and_refresh("list.toggle_ordered", None, cx);
    }

    pub fn command_undo(&mut self, cx: &mut Context<Self>) {
        if self.editor.undo() {
            self.layout_cache.clear();
            self.text_block_order = text_block_paths(self.editor.doc(), self.editor.registry());
            cx.notify();
        }
    }

    pub fn command_redo(&mut self, cx: &mut Context<Self>) {
        if self.editor.redo() {
            self.layout_cache.clear();
            self.text_block_order = text_block_paths(self.editor.doc(), self.editor.registry());
            cx.notify();
        }
    }

    pub fn command_insert_divider(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("core.insert_divider", None, cx);
    }

    pub fn command_insert_mention(&mut self, label: String, cx: &mut Context<Self>) {
        _ = self.delete_selection_if_any(cx);
        let args = serde_json::json!({ "label": label });
        _ = self.run_command_and_refresh("mention.insert", Some(args), cx);
    }

    pub fn command_set_heading(&mut self, level: u64, cx: &mut Context<Self>) {
        let args = serde_json::json!({ "level": level });
        _ = self.run_command_and_refresh("block.set_heading", Some(args), cx);
    }

    pub fn command_unset_heading(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("block.unset_heading", None, cx);
    }

    pub fn command_toggle_bold(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("marks.toggle_bold", None, cx);
    }

    pub fn command_toggle_italic(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("marks.toggle_italic", None, cx);
    }

    pub fn command_toggle_underline(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("marks.toggle_underline", None, cx);
    }

    pub fn command_toggle_strikethrough(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("marks.toggle_strikethrough", None, cx);
    }

    pub fn command_toggle_code(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("marks.toggle_code", None, cx);
    }

    pub fn command_set_text_color(&mut self, color: String, cx: &mut Context<Self>) {
        let args = serde_json::json!({ "color": color });
        _ = self.run_command_and_refresh("marks.set_text_color", Some(args), cx);
    }

    pub fn command_unset_text_color(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("marks.unset_text_color", None, cx);
    }

    pub fn command_set_highlight_color(&mut self, color: String, cx: &mut Context<Self>) {
        let args = serde_json::json!({ "color": color });
        _ = self.run_command_and_refresh("marks.set_highlight_color", Some(args), cx);
    }

    pub fn command_unset_highlight_color(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("marks.unset_highlight_color", None, cx);
    }

    pub fn command_toggle_bulleted_list(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("list.toggle_bulleted", None, cx);
    }

    pub fn command_toggle_ordered_list(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("list.toggle_ordered", None, cx);
    }

    pub fn command_insert_table(&mut self, rows: usize, cols: usize, cx: &mut Context<Self>) {
        _ = self.delete_selection_if_any(cx);
        let args = serde_json::json!({ "rows": rows, "cols": cols });
        _ = self.run_command_and_refresh("table.insert", Some(args), cx);
    }

    pub fn command_insert_table_row_below(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("table.insert_row_below", None, cx);
    }

    pub fn command_insert_table_col_right(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("table.insert_col_right", None, cx);
    }

    pub fn command_delete_table_row(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("table.delete_row", None, cx);
    }

    pub fn command_delete_table_col(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("table.delete_col", None, cx);
    }

    pub fn command_set_link(&mut self, url: String, cx: &mut Context<Self>) {
        // UX: if there's no selection, apply link to the word under the caret so users get an
        // immediate visual confirmation (instead of only affecting subsequent input).
        if self.editor.selection().is_collapsed() {
            if let Some(block_path) = self.active_text_block_path()
                && let Some(text) = self.text_block_text(&block_path)
            {
                let cursor = self
                    .row_offset_for_point(&self.editor.selection().focus)
                    .unwrap_or(0)
                    .min(text.len());
                let text = text.as_str();
                let mut range = Self::word_range(text, cursor);

                let mut selected = text.get(range.clone()).unwrap_or("");
                if range.start >= range.end || selected.chars().all(|ch| ch.is_whitespace()) {
                    // If we landed on whitespace, try the next non-whitespace run to the right.
                    let mut ix = range.end.min(text.len());
                    while ix < text.len() {
                        let Some(ch) = text.get(ix..).and_then(|s| s.chars().next()) else {
                            break;
                        };
                        if !ch.is_whitespace() {
                            range = Self::word_range(text, ix);
                            selected = text.get(range.clone()).unwrap_or("");
                            break;
                        }
                        ix = Self::next_boundary(text, ix);
                    }
                }

                if range.start >= range.end || selected.chars().all(|ch| ch.is_whitespace()) {
                    // If there's nothing meaningful to the right, try the left.
                    let mut ix = range.start.min(text.len());
                    while ix > 0 {
                        let prev = Self::prev_boundary(text, ix);
                        let Some(ch) = text.get(prev..).and_then(|s| s.chars().next()) else {
                            break;
                        };
                        if !ch.is_whitespace() {
                            range = Self::word_range(text, prev);
                            selected = text.get(range.clone()).unwrap_or("");
                            break;
                        }
                        ix = prev;
                    }
                }

                if range.start < range.end && !selected.chars().all(|ch| ch.is_whitespace()) {
                    self.set_selection_in_active_block(range.start, range.end, cx);
                }
            }
        }

        let args = serde_json::json!({ "url": url });
        _ = self.run_command_and_refresh("marks.set_link", Some(args), cx);
    }

    pub fn command_unset_link(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("marks.unset_link", None, cx);
    }

    pub fn is_bold_active(&self) -> bool {
        self.editor
            .run_query::<bool>("marks.is_bold_active", None)
            .unwrap_or(false)
    }

    pub fn has_link_active(&self) -> bool {
        self.editor
            .run_query::<bool>("marks.has_link_active", None)
            .unwrap_or(false)
    }

    pub fn active_link_url(&self) -> Option<String> {
        self.editor
            .run_query::<Marks>("marks.get_active", None)
            .ok()
            .and_then(|m| m.link)
    }

    pub fn is_bulleted_list_active(&self) -> bool {
        self.editor
            .run_query::<bool>(
                "list.is_active",
                Some(serde_json::json!({ "type": "bulleted" })),
            )
            .unwrap_or(false)
    }

    pub fn is_ordered_list_active(&self) -> bool {
        self.editor
            .run_query::<bool>(
                "list.is_active",
                Some(serde_json::json!({ "type": "ordered" })),
            )
            .unwrap_or(false)
    }

    pub fn is_table_active(&self) -> bool {
        self.editor
            .run_query::<bool>("table.is_active", None)
            .unwrap_or(false)
    }

    pub fn heading_level(&self) -> Option<u64> {
        self.editor
            .run_query::<Option<u64>>("block.heading_level", None)
            .ok()
            .flatten()
    }

    pub fn command_copy(&mut self, cx: &mut Context<Self>) {
        let selection = self.editor.selection().clone();
        if selection.is_collapsed() {
            return;
        }

        let mut start = selection.anchor.clone();
        let mut end = selection.focus.clone();
        if start.path == end.path {
            if end.offset < start.offset {
                std::mem::swap(&mut start, &mut end);
            }
        } else if end.path < start.path {
            std::mem::swap(&mut start, &mut end);
        }

        let start_offset = self.row_offset_for_point(&start).unwrap_or(0);
        let end_offset = self.row_offset_for_point(&end).unwrap_or(0);

        let Some(start_block) = start.path.split_last().map(|(_, p)| p.to_vec()) else {
            return;
        };
        let Some(end_block) = end.path.split_last().map(|(_, p)| p.to_vec()) else {
            return;
        };

        if start_block == end_block {
            let text = self.text_block_text(&start_block).unwrap_or_default();
            let start = start_offset.min(text.len());
            let end = end_offset.min(text.len()).max(start);
            let selected = text.as_str().get(start..end).unwrap_or("").to_string();
            cx.write_to_clipboard(ClipboardItem::new_string(selected));
            return;
        }

        let blocks = text_block_paths(self.editor.doc(), self.editor.registry());
        let Some(start_pos) = blocks.iter().position(|p| p == &start_block) else {
            return;
        };
        let Some(end_pos) = blocks.iter().position(|p| p == &end_block) else {
            return;
        };
        let (start_pos, end_pos) = if start_pos <= end_pos {
            (start_pos, end_pos)
        } else {
            (end_pos, start_pos)
        };

        let mut out = String::new();
        for (ix, block_path) in blocks.iter().enumerate().take(end_pos + 1).skip(start_pos) {
            let text = self.text_block_text(block_path).unwrap_or_default();
            if ix == start_pos {
                let start = start_offset.min(text.len());
                out.push_str(text.as_str().get(start..).unwrap_or(""));
                out.push('\n');
            } else if ix == end_pos {
                let end = end_offset.min(text.len());
                out.push_str(text.as_str().get(..end).unwrap_or(""));
            } else {
                out.push_str(text.as_str());
                out.push('\n');
            }
        }

        cx.write_to_clipboard(ClipboardItem::new_string(out));
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
        let Some((_, block_path)) = insert_at.path.split_last() else {
            return;
        };
        let offset = self.row_offset_for_point(&insert_at).unwrap_or(0);
        let marks = self.active_marks();

        if let Some(tx) = self.tx_insert_multiline_text_at_block_offset(
            block_path,
            offset,
            &new_text,
            &marks,
            "command:paste",
        ) {
            self.push_tx(tx, cx);
        }
    }

    pub fn command_select_all(&mut self, cx: &mut Context<Self>) {
        let blocks = text_block_paths(self.editor.doc(), self.editor.registry());
        let (Some(first), Some(last)) = (blocks.first(), blocks.last()) else {
            return;
        };

        let anchor = self.point_for_block_offset(first, 0).unwrap_or_else(|| {
            let mut path = first.clone();
            path.push(0);
            CorePoint::new(path, 0)
        });
        let focus = self
            .point_for_block_offset(last, self.block_text_len_in_block(last))
            .unwrap_or_else(|| {
                let mut path = last.clone();
                path.push(0);
                CorePoint::new(path, 0)
            });
        self.set_selection_points(anchor, focus, cx);
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

        let sel = self.editor.selection();
        let anchor_offset = self.row_offset_for_point(&sel.anchor).unwrap_or(0);
        let focus_offset = self.row_offset_for_point(&sel.focus).unwrap_or(0);
        let reversed = if let Some(active_block) = self.active_text_block_path() {
            let anchor_block = sel.anchor.path.split_last().map(|(_, p)| p);
            let focus_block = sel.focus.path.split_last().map(|(_, p)| p);
            anchor_block == Some(active_block.as_slice())
                && focus_block == Some(active_block.as_slice())
                && focus_offset < anchor_offset
        } else {
            false
        };

        Some(UTF16Selection {
            range: utf16,
            reversed,
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

        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let marks = self.active_marks();
        let inserted = new_text.replace("\r\n", "\n").replace('\r', "\n");

        let tx = if inserted.contains('\n') {
            self.tx_replace_range_with_multiline_text_in_block(
                &block_path,
                range_bytes.clone(),
                &inserted,
                &marks,
                "ime:replace_text",
            )
        } else {
            self.tx_replace_range_in_block(
                &block_path,
                range_bytes.clone(),
                &inserted,
                &marks,
                None,
                "ime:replace_text",
            )
        };

        if let Some(tx) = tx {
            self.push_tx(tx, cx);
        }
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

        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let marks = self.active_marks();
        let inserted = new_text.replace("\r\n", "\n").replace('\r', "\n");
        if inserted.contains('\n') {
            return;
        }

        let inserted_len = inserted.len();
        let marked = if inserted_len == 0 {
            None
        } else {
            Some(range_bytes.start..range_bytes.start + inserted_len)
        };
        self.ime_marked_range = marked.clone();

        let selection_after_offsets =
            if let (Some(marked), Some(sel_utf16)) = (marked, new_selected_range_utf16) {
                let rel_start = utf16_to_byte(inserted.as_str(), sel_utf16.start);
                let rel_end = utf16_to_byte(inserted.as_str(), sel_utf16.end);
                Some((marked.start + rel_start, marked.start + rel_end))
            } else {
                None
            };

        if let Some(tx) = self.tx_replace_range_in_block(
            &block_path,
            range_bytes.clone(),
            &inserted,
            &marks,
            selection_after_offsets,
            "ime:replace_and_mark_text",
        ) {
            self.push_tx(tx, cx);
        }
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let block_path = self.active_text_block_path()?;
        let cache = self.layout_cache.get(block_path.as_slice())?;
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
        let block_path = self.active_text_block_path()?;
        let byte_ix = self.offset_for_point_in_block(&block_path, point)?;
        let text = self.active_text();
        Some(byte_to_utf16(text.as_str(), byte_ix))
    }
}

impl gpui::Focusable for RichTextState {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

pub struct RichTextLineElement {
    state: Entity<RichTextState>,
    block_path: Vec<usize>,
    styled_text: StyledText,
    text: SharedString,
    segments: Vec<InlineTextSegment>,
    base_text_style: Option<TextStyle>,
}

impl RichTextLineElement {
    pub fn new(state: Entity<RichTextState>, block_path: Vec<usize>) -> Self {
        Self {
            state,
            block_path,
            styled_text: StyledText::new(SharedString::default()),
            text: SharedString::default(),
            segments: Vec::new(),
            base_text_style: None,
        }
    }

    pub fn with_base_text_style(mut self, style: TextStyle) -> Self {
        self.base_text_style = Some(style);
        self
    }

    fn paint_selection_range(
        layout: &gpui::TextLayout,
        range: Range<usize>,
        bounds: &Bounds<Pixels>,
        line_height: Pixels,
        window: &mut Window,
        selection_color: gpui::Hsla,
    ) {
        let start_ix = range.start.min(layout.len());
        let end_ix = range.end.min(layout.len());
        if start_ix >= end_ix {
            return;
        }

        let Some(first_pos) = layout.position_for_index(0).or_else(|| {
            layout
                .position_for_index(layout.len())
                .or_else(|| Some(bounds.origin))
        }) else {
            return;
        };
        let origin = first_pos;

        let Some(line_layout) = layout.line_layout_for_index(start_ix) else {
            return;
        };

        let mut boundaries: Vec<usize> = line_layout
            .wrap_boundaries()
            .iter()
            .filter_map(|boundary| {
                let run = line_layout.unwrapped_layout.runs.get(boundary.run_ix)?;
                run.glyphs.get(boundary.glyph_ix).map(|g| g.index)
            })
            .collect();

        // `wrap_boundaries` is empty for non-wrapping text; treat it as a single visual line.
        boundaries.push(line_layout.len());
        boundaries.dedup();

        let mut seg_start = 0usize;
        for (seg_ix, seg_end) in boundaries.into_iter().enumerate() {
            let seg_start_ix = seg_start;
            seg_start = seg_end;

            let a = start_ix.max(seg_start_ix);
            let b = end_ix.min(seg_end);
            if a >= b {
                continue;
            }

            let seg_origin_x = line_layout.unwrapped_layout.x_for_index(seg_start_ix);
            let start_x = line_layout.unwrapped_layout.x_for_index(a) - seg_origin_x;
            let end_x = line_layout.unwrapped_layout.x_for_index(b) - seg_origin_x;

            let y = origin.y + line_height * seg_ix as f32;
            window.paint_quad(gpui::quad(
                Bounds::from_corners(
                    point(origin.x + start_x, y),
                    point(origin.x + end_x, y + line_height),
                ),
                px(0.),
                selection_color,
                gpui::Edges::default(),
                gpui::transparent_black(),
                gpui::BorderStyle::default(),
            ));
        }
    }
}

impl IntoElement for RichTextLineElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for RichTextLineElement {
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
        let (text, segments, marked_range) = {
            let state = self.state.read(cx);
            let Some(el) = element_at_path(state.editor.doc(), &self.block_path) else {
                return (window.request_layout(gpui::Style::default(), [], cx), ());
            };
            if !is_text_block_kind(state.editor.registry(), &el.kind) {
                self.text = SharedString::default();
                return (window.request_layout(gpui::Style::default(), [], cx), ());
            }

            let mut combined = String::new();
            let mut segments = Vec::new();
            for (child_ix, child) in el.children.iter().enumerate() {
                match child {
                    Node::Text(t) => {
                        let start = combined.len();
                        combined.push_str(&t.text);
                        segments.push(InlineTextSegment {
                            child_ix,
                            start,
                            len: t.text.len(),
                            marks: t.marks.clone(),
                            kind: InlineTextSegmentKind::Text,
                        });
                    }
                    Node::Void(v) => {
                        let display = v.inline_text();
                        let start = combined.len();
                        combined.push_str(&display);
                        segments.push(InlineTextSegment {
                            child_ix,
                            start,
                            len: display.len(),
                            marks: Marks::default(),
                            kind: InlineTextSegmentKind::Void {
                                kind: v.kind.clone().into(),
                            },
                        });
                    }
                    Node::Element(_) => {}
                }
            }
            if segments.is_empty() {
                segments.push(InlineTextSegment {
                    child_ix: 0,
                    start: 0,
                    len: 0,
                    marks: Marks::default(),
                    kind: InlineTextSegmentKind::Text,
                });
            }

            let marked = (state.active_text_block_path().as_ref() == Some(&self.block_path))
                .then(|| state.ime_marked_range.clone())
                .flatten();
            (SharedString::new(combined), segments, marked)
        };

        self.text = text.clone();
        self.segments = segments.clone();

        let render_text: SharedString = if text.is_empty() { " ".into() } else { text };

        let marked_range = marked_range
            .filter(|_| !self.text.is_empty())
            .map(|r| r.start.min(self.text.len())..r.end.min(self.text.len()))
            .filter(|r| r.start < r.end);

        let mut runs = Vec::new();

        let theme = cx.theme();
        let base_style = self
            .base_text_style
            .clone()
            .unwrap_or_else(|| window.text_style());
        let apply_marks = |mut style: TextStyle,
                           marks: &Marks,
                           kind: &InlineTextSegmentKind,
                           theme: &gpui_component::Theme|
         -> TextStyle {
            if marks.bold {
                style.font_weight = FontWeight::BOLD;
            }
            if marks.italic {
                style.font_style = FontStyle::Italic;
            }
            if marks.code {
                style.font_family = theme.mono_font_family.clone();
                if style.background_color.is_none() {
                    style.background_color = Some(theme.muted);
                }
            }

            if let Some(color) = marks.highlight_color.as_deref().and_then(parse_hex_color) {
                style.background_color = Some(color);
            }
            if let Some(color) = marks.text_color.as_deref().and_then(parse_hex_color) {
                style.color = color;
            }

            let is_link = marks.link.is_some();
            if is_link {
                style.color = theme.blue;
            }
            if marks.underline || is_link {
                style.underline = Some(UnderlineStyle {
                    thickness: px(1.),
                    color: Some(style.color),
                    wavy: false,
                });
            }
            if marks.strikethrough {
                style.strikethrough = Some(StrikethroughStyle {
                    thickness: px(1.),
                    color: Some(style.color),
                });
            }

            if matches!(
                kind,
                InlineTextSegmentKind::Void { kind } if kind.as_ref() == "mention"
            ) {
                style.color = theme.foreground;
                style.background_color = Some(theme.muted);
                style.underline = None;
                style.strikethrough = None;
                style.font_weight = FontWeight::MEDIUM;
            }

            style
        };

        if self.text.is_empty() {
            let marks = segments
                .first()
                .map(|s| s.marks.clone())
                .unwrap_or_default();
            let kind = InlineTextSegmentKind::Text;
            let style = apply_marks(base_style.clone(), &marks, &kind, theme);
            runs.push(style.to_run(1));
        } else {
            for seg in segments.iter().filter(|s| s.len > 0) {
                let style = apply_marks(base_style.clone(), &seg.marks, &seg.kind, theme);

                if let Some(marked) = marked_range.clone() {
                    let seg_start = seg.start;
                    let seg_end = seg.start + seg.len;
                    let sel_start = marked.start.max(seg_start);
                    let sel_end = marked.end.min(seg_end);
                    if sel_start >= sel_end {
                        runs.push(style.to_run(seg.len));
                        continue;
                    }

                    let before = sel_start - seg_start;
                    let inside = sel_end - sel_start;
                    let after = seg_end - sel_end;

                    if before > 0 {
                        runs.push(style.clone().to_run(before));
                    }

                    let mut marked_style = style.clone();
                    marked_style.underline = Some(UnderlineStyle {
                        thickness: px(1.),
                        color: Some(window.text_style().color),
                        wavy: false,
                    });
                    runs.push(marked_style.to_run(inside));

                    if after > 0 {
                        runs.push(style.to_run(after));
                    }
                } else {
                    runs.push(style.to_run(seg.len));
                }
            }
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
        let block_path = self.block_path.clone();
        let text = self.text.clone();
        let segments = self.segments.clone();
        self.state.update(cx, |state, _| {
            state.layout_cache.insert(
                block_path,
                LineLayoutCache {
                    bounds,
                    text_layout: text_layout.clone(),
                    text,
                    segments,
                },
            );
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
        let layout = self.styled_text.layout().clone();
        let line_height = layout.line_height();

        if selection.is_collapsed() {
            if selection
                .focus
                .path
                .split_last()
                .map(|(_, p)| p == self.block_path.as_slice())
                .unwrap_or(false)
                && is_focused
            {
                let caret_ix = {
                    let state = self.state.read(cx);
                    state.row_offset_for_point(&selection.focus).unwrap_or(0)
                }
                .min(layout.len());
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

        let mut start = selection.anchor.clone();
        let mut end = selection.focus.clone();
        if start.path == end.path {
            if end.offset < start.offset {
                std::mem::swap(&mut start, &mut end);
            }
        } else if end.path < start.path {
            std::mem::swap(&mut start, &mut end);
        }
        if start.path.len() < 2 || end.path.len() < 2 {
            return;
        }

        let Some(start_block) = start.path.split_last().map(|(_, p)| p) else {
            return;
        };
        let Some(end_block) = end.path.split_last().map(|(_, p)| p) else {
            return;
        };
        if self.block_path.as_slice() < start_block || self.block_path.as_slice() > end_block {
            return;
        }

        let row_len = self.text.len();
        let (a, b) = if start_block == end_block {
            let state = self.state.read(cx);
            let a = state.row_offset_for_point(&start).unwrap_or(0).min(row_len);
            let b = state.row_offset_for_point(&end).unwrap_or(0).min(row_len);
            (a, b)
        } else if self.block_path.as_slice() == start_block {
            let state = self.state.read(cx);
            let a = state.row_offset_for_point(&start).unwrap_or(0).min(row_len);
            (a, row_len)
        } else if self.block_path.as_slice() == end_block {
            let state = self.state.read(cx);
            let b = state.row_offset_for_point(&end).unwrap_or(0).min(row_len);
            (0, b)
        } else {
            (0, row_len)
        };

        if a >= b {
            return;
        }

        Self::paint_selection_range(
            &layout,
            a..b,
            &bounds,
            line_height,
            window,
            cx.theme().selection,
        );
    }
}

pub(crate) struct RichTextInputHandlerElement {
    state: Entity<RichTextState>,
}

impl RichTextInputHandlerElement {
    pub(crate) fn new(state: Entity<RichTextState>) -> Self {
        Self { state }
    }
}

impl IntoElement for RichTextInputHandlerElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for RichTextInputHandlerElement {
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

                // Cmd/Ctrl-click opens a link instead of moving the caret.
                if event.modifiers.secondary() && !event.modifiers.shift {
                    let url = {
                        let this = state.read(cx);
                        let Some(block_path) = this.text_block_path_for_point(event.position)
                        else {
                            return;
                        };
                        let offset = this
                            .offset_for_point_in_block(&block_path, event.position)
                            .unwrap_or_else(|| this.block_text_len_in_block(&block_path));
                        this.link_at_offset(&block_path, offset)
                    };
                    if let Some(url) = url {
                        cx.open_url(&url);
                        return;
                    }
                }

                let focus_handle = { state.read(cx).focus_handle.clone() };
                window.focus(&focus_handle);

                state.update(cx, |this, cx| {
                    let Some(block_path) = this.text_block_path_for_point(event.position) else {
                        return;
                    };

                    let offset = this
                        .offset_for_point_in_block(&block_path, event.position)
                        .unwrap_or_else(|| this.block_text_len_in_block(&block_path));
                    let focus = this
                        .point_for_block_offset(&block_path, offset)
                        .unwrap_or_else(|| {
                            let mut path = block_path.clone();
                            path.push(0);
                            CorePoint::new(path, 0)
                        });

                    if event.click_count >= 3 {
                        let range = 0..this.block_text_len_in_block(&block_path);
                        let anchor = this
                            .point_for_block_offset(&block_path, range.start)
                            .unwrap_or_else(|| focus.clone());
                        let focus = this
                            .point_for_block_offset(&block_path, range.end)
                            .unwrap_or_else(|| focus.clone());
                        this.set_selection_points(anchor, focus, cx);
                        // Do not enter "drag selecting" mode on multi-click. This avoids tiny mouse
                        // movements between down/up being interpreted as a drag that expands the
                        // selection across paragraphs (which can look like "select all").
                        this.selecting = false;
                        this.selection_anchor = None;
                        return;
                    }

                    if event.click_count == 2 {
                        let text = this.text_block_text(&block_path).unwrap_or_default();
                        let range =
                            this.word_or_void_range_in_block(&block_path, text.as_str(), offset);
                        let anchor = this
                            .point_for_block_offset(&block_path, range.start)
                            .unwrap_or_else(|| focus.clone());
                        let focus = this
                            .point_for_block_offset(&block_path, range.end)
                            .unwrap_or_else(|| focus.clone());
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
                    let Some(block_path) = this.text_block_path_for_point(event.position) else {
                        return;
                    };
                    let offset = this
                        .offset_for_point_in_block(&block_path, event.position)
                        .unwrap_or_else(|| this.block_text_len_in_block(&block_path));
                    let focus = this
                        .point_for_block_offset(&block_path, offset)
                        .unwrap_or_else(|| {
                            let mut path = block_path.clone();
                            path.push(0);
                            CorePoint::new(path, 0)
                        });
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

impl Render for RichTextState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let state = cx.entity().clone();

        if !self.did_auto_focus && !window.is_inspector_picking(cx) {
            window.focus(&self.focus_handle);
            self.did_auto_focus = true;
        }

        let mut blocks: Vec<AnyElement> = Vec::new();
        for (row, node) in self.editor.doc().children.iter().enumerate() {
            match node {
                Node::Element(el) if el.kind == "list_item" => {
                    let list_type = el
                        .attrs
                        .get("list_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("bulleted");
                    let marker = if list_type == "ordered" {
                        let ix = el
                            .attrs
                            .get("list_index")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(1);
                        format!("{ix}.")
                    } else {
                        "•".to_string()
                    };
                    let level = el
                        .attrs
                        .get("list_level")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let indent = px(16. * level as f32);
                    blocks.push(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .gap(px(8.))
                            .pl(indent)
                            .child(
                                div()
                                    .w(px(24.))
                                    .flex()
                                    .justify_end()
                                    .text_color(theme.muted_foreground)
                                    .child(marker),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .child(RichTextLineElement::new(state.clone(), vec![row])),
                            )
                            .into_any_element(),
                    );
                }
                Node::Element(el) if is_text_block_kind(self.editor.registry(), &el.kind) => {
                    let mut line = RichTextLineElement::new(state.clone(), vec![row]);
                    if el.kind == "heading" {
                        let level = el
                            .attrs
                            .get("level")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(1)
                            .clamp(1, 6);
                        let mut style = window.text_style();
                        let size = match level {
                            1 => 28.0,
                            2 => 22.0,
                            3 => 18.0,
                            4 => 16.0,
                            5 => 14.0,
                            _ => 13.0,
                        };
                        style.font_size = px(size).into();
                        style.line_height = px(size * 1.25).into();
                        style.font_weight = FontWeight::SEMIBOLD;
                        line = line.with_base_text_style(style);
                    }
                    blocks.push(line.into_any_element());
                }
                Node::Element(el) if el.kind == "table" => {
                    let mut rows: Vec<AnyElement> = Vec::new();
                    for (row_ix, row_node) in el.children.iter().enumerate() {
                        let Node::Element(row_el) = row_node else {
                            continue;
                        };
                        if row_el.kind != "table_row" {
                            continue;
                        }

                        let mut cells: Vec<AnyElement> = Vec::new();
                        for (cell_ix, cell_node) in row_el.children.iter().enumerate() {
                            let Node::Element(cell_el) = cell_node else {
                                continue;
                            };
                            if cell_el.kind != "table_cell" {
                                continue;
                            }

                            let mut cell_blocks: Vec<AnyElement> = Vec::new();
                            for (block_ix, block_node) in cell_el.children.iter().enumerate() {
                                let block_path = vec![row, row_ix, cell_ix, block_ix];
                                match block_node {
                                    Node::Element(block_el) if block_el.kind == "list_item" => {
                                        let list_type = block_el
                                            .attrs
                                            .get("list_type")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("bulleted");
                                        let marker = if list_type == "ordered" {
                                            let ix = block_el
                                                .attrs
                                                .get("list_index")
                                                .and_then(|v| v.as_u64())
                                                .unwrap_or(1);
                                            format!("{ix}.")
                                        } else {
                                            "•".to_string()
                                        };
                                        let level = block_el
                                            .attrs
                                            .get("list_level")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0);
                                        let indent = px(16. * level as f32);
                                        cell_blocks.push(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .items_start()
                                                .gap(px(8.))
                                                .pl(indent)
                                                .child(
                                                    div()
                                                        .w(px(24.))
                                                        .flex()
                                                        .justify_end()
                                                        .text_color(theme.muted_foreground)
                                                        .child(marker),
                                                )
                                                .child(div().flex_1().min_w(px(0.)).child(
                                                    RichTextLineElement::new(
                                                        state.clone(),
                                                        block_path,
                                                    ),
                                                ))
                                                .into_any_element(),
                                        );
                                    }
                                    Node::Element(block_el)
                                        if is_text_block_kind(
                                            self.editor.registry(),
                                            &block_el.kind,
                                        ) =>
                                    {
                                        let mut line =
                                            RichTextLineElement::new(state.clone(), block_path);
                                        if block_el.kind == "heading" {
                                            let level = block_el
                                                .attrs
                                                .get("level")
                                                .and_then(|v| v.as_u64())
                                                .unwrap_or(1)
                                                .clamp(1, 6);
                                            let mut style = window.text_style();
                                            let size = match level {
                                                1 => 28.0,
                                                2 => 22.0,
                                                3 => 18.0,
                                                4 => 16.0,
                                                5 => 14.0,
                                                _ => 13.0,
                                            };
                                            style.font_size = px(size).into();
                                            style.line_height = px(size * 1.25).into();
                                            style.font_weight = FontWeight::SEMIBOLD;
                                            line = line.with_base_text_style(style);
                                        }
                                        cell_blocks.push(line.into_any_element());
                                    }
                                    Node::Void(v) if v.kind == "divider" => {
                                        cell_blocks.push(
                                            div()
                                                .py(px(8.))
                                                .child(div().w_full().h(px(1.)).bg(theme.border))
                                                .into_any_element(),
                                        );
                                    }
                                    _ => {
                                        cell_blocks.push(
                                            div()
                                                .text_color(theme.muted_foreground)
                                                .italic()
                                                .child("<unknown cell block>")
                                                .into_any_element(),
                                        );
                                    }
                                }
                            }

                            cells.push(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .border_1()
                                    .border_color(theme.border)
                                    .p(px(6.))
                                    .child(div().flex_col().gap(px(6.)).children(cell_blocks))
                                    .into_any_element(),
                            );
                        }

                        rows.push(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(6.))
                                .children(cells)
                                .into_any_element(),
                        );
                    }

                    blocks.push(
                        div()
                            .border_1()
                            .border_color(theme.border)
                            .rounded(theme.radius / 2.)
                            .p(px(6.))
                            .child(div().flex_col().gap(px(6.)).children(rows))
                            .into_any_element(),
                    );
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
                            .child(format!(
                                "<unknown node at {row}: {:?}>",
                                self.editor.doc().children.get(row).map(|n| match n {
                                    Node::Element(el) => el.kind.as_str(),
                                    Node::Void(v) => v.kind.as_str(),
                                    Node::Text(_) => "text",
                                })
                            ))
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
                this.on_action(window.listener_for(&state, RichTextState::backspace))
                    .on_action(window.listener_for(&state, RichTextState::delete))
                    .on_action(window.listener_for(&state, RichTextState::enter))
                    .on_action(window.listener_for(&state, RichTextState::left))
                    .on_action(window.listener_for(&state, RichTextState::right))
                    .on_action(window.listener_for(&state, RichTextState::select_left))
                    .on_action(window.listener_for(&state, RichTextState::select_right))
                    .on_action(window.listener_for(&state, RichTextState::undo))
                    .on_action(window.listener_for(&state, RichTextState::redo))
                    .on_action(window.listener_for(&state, RichTextState::copy))
                    .on_action(window.listener_for(&state, RichTextState::cut))
                    .on_action(window.listener_for(&state, RichTextState::paste))
                    .on_action(window.listener_for(&state, RichTextState::select_all))
                    .on_action(window.listener_for(&state, RichTextState::insert_divider))
                    .on_action(window.listener_for(&state, RichTextState::insert_mention))
                    .on_action(window.listener_for(&state, RichTextState::toggle_bold))
                    .on_action(window.listener_for(&state, RichTextState::toggle_italic))
                    .on_action(window.listener_for(&state, RichTextState::toggle_underline))
                    .on_action(window.listener_for(&state, RichTextState::toggle_strikethrough))
                    .on_action(window.listener_for(&state, RichTextState::toggle_code))
                    .on_action(window.listener_for(&state, RichTextState::toggle_bulleted_list))
                    .on_action(window.listener_for(&state, RichTextState::toggle_ordered_list))
            })
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .child(RichTextInputHandlerElement::new(state.clone())),
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

fn parse_hex_color(value: &str) -> Option<gpui::Hsla> {
    gpui::Rgba::try_from(value).ok().map(gpui::Hsla::from)
}
