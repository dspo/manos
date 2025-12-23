use gpui::{
    AnyElement, App, Bounds, CursorStyle, Element, ElementId, ElementInputHandler, FontStyle,
    FontWeight, GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId,
    InteractiveElement as _, IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement as _, Pixels, Point, Render, SharedString,
    StatefulInteractiveElement as _, StrikethroughStyle, Styled as _, StyledText, UnderlineStyle,
    Window, div, point, px, size,
};

use crate::rope_ext::RopeExt as _;

use super::state::{LineLayoutCache, RichTextState};
use crate::document::{BlockAlign, BlockKind, BlockTextSize, InlineNode, OrderedListStyle};

fn monospace_font_family() -> SharedString {
    #[cfg(target_os = "macos")]
    {
        "Menlo".into()
    }
    #[cfg(target_os = "windows")]
    {
        "Consolas".into()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        "monospace".into()
    }
}

pub(crate) struct RichTextInputHandlerElement {
    state: gpui::Entity<RichTextState>,
}

impl RichTextInputHandlerElement {
    pub(crate) fn new(state: gpui::Entity<RichTextState>) -> Self {
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
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.state.update(cx, |state, _| {
            state.viewport_bounds = bounds;
        });
        // The editor uses an absolute-positioned overlay input handler. Use `BlockMouseExceptScroll`
        // so the underlying scroll area can still receive wheel events.
        window.insert_hitbox(bounds, HitboxBehavior::BlockMouseExceptScroll)
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
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
                    if let Some(offset) = state.read(cx).offset_for_point(event.position) {
                        if let Some(url) = state.read(cx).link_at_offset(offset) {
                            cx.open_url(&url);
                            return;
                        }
                    }
                }

                let focus_handle = { state.read(cx).focus_handle.clone() };
                window.focus(&focus_handle);

                state.update(cx, |this, cx| {
                    let offset = this
                        .offset_for_point(event.position)
                        .unwrap_or(this.text.len());

                    if event.click_count >= 3 {
                        let row = this.text.offset_to_point(offset).row;
                        let range = this.line_range(row);
                        this.set_selection(range.start, range.end);
                    } else if event.click_count == 2 {
                        let range = this.word_range(offset);
                        this.set_selection(range.start, range.end);
                    } else if event.modifiers.shift {
                        let anchor = this.selection.start;
                        this.set_selection(anchor, offset);
                    } else {
                        this.set_selection(offset, offset);
                    }

                    this.selecting = true;
                    this.preferred_x = Some(event.position.x);
                    cx.notify();
                    this.scroll_cursor_into_view(window, cx);
                });
            }
        });

        // Mouse drag selection across blocks.
        window.on_mouse_event({
            let state = self.state.clone();
            move |event: &MouseMoveEvent, _, window, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    return;
                }
                state.update(cx, |this, cx| {
                    this.update_column_resize(event.position.x, cx);
                    if !this.selecting {
                        return;
                    }
                    if let Some(offset) = this.offset_for_point(event.position) {
                        this.selection.end = offset;
                        cx.notify();
                        this.scroll_cursor_into_view(window, cx);
                    }
                });
            }
        });

        window.on_mouse_event({
            let state = self.state.clone();
            move |event: &MouseUpEvent, _, window, cx| {
                if event.button != MouseButton::Left {
                    return;
                }
                state.update(cx, |this, cx| {
                    this.selecting = false;
                    this.stop_column_resize(cx);
                    this.scroll_cursor_into_view(window, cx);
                });
            }
        });
    }
}

pub(crate) struct RichTextLineElement {
    state: gpui::Entity<RichTextState>,
    row: usize,
    text: SharedString,
    styled_text: StyledText,
}

impl RichTextLineElement {
    pub(crate) fn new(state: gpui::Entity<RichTextState>, row: usize) -> Self {
        Self {
            state,
            row,
            text: SharedString::default(),
            styled_text: StyledText::new(SharedString::default()),
        }
    }

    fn paint_inline_code_background(
        state: &RichTextState,
        row: usize,
        line_text_len: usize,
        bounds: Bounds<Pixels>,
        text_layout: &gpui::TextLayout,
        window: &mut Window,
    ) {
        let Some(block) = state.document.blocks.get(row) else {
            return;
        };

        if line_text_len == 0 {
            return;
        }

        let mut code_ranges: Vec<std::ops::Range<usize>> = Vec::new();
        let mut cursor = 0usize;
        for node in &block.inlines {
            let InlineNode::Text(text) = node;
            let len = text.text.len();
            if len == 0 {
                continue;
            }

            let start = cursor;
            let end = cursor.saturating_add(len).min(line_text_len);
            cursor = end;

            if start >= end {
                continue;
            }

            if text.style.code && text.style.bg.is_none() {
                code_ranges.push(start..end);
            }
        }

        if code_ranges.is_empty() {
            return;
        }

        // Merge adjacent code ranges so we paint contiguous backgrounds as one pill per wrap-line.
        let mut merged: Vec<std::ops::Range<usize>> = Vec::with_capacity(code_ranges.len());
        for range in code_ranges {
            if let Some(last) = merged.last_mut()
                && last.end == range.start
            {
                last.end = range.end;
            } else {
                merged.push(range);
            }
        }

        let line_height = text_layout.line_height();
        let Some(wrapped) = text_layout.line_layout_for_index(0) else {
            return;
        };

        // Collect wrap boundary byte indices (start indices of subsequent wrap lines).
        let mut wrap_indices: Vec<usize> = Vec::with_capacity(wrapped.wrap_boundaries.len());
        let mut last_ix = 0usize;
        for boundary in wrapped.wrap_boundaries.iter() {
            let Some(run) = wrapped.unwrapped_layout.runs.get(boundary.run_ix) else {
                continue;
            };
            let Some(glyph) = run.glyphs.get(boundary.glyph_ix) else {
                continue;
            };
            let ix = glyph.index.min(line_text_len);
            if ix > last_ix && ix < line_text_len {
                wrap_indices.push(ix);
                last_ix = ix;
            }
        }

        let align_width = bounds.size.width;
        let align = block.format.align;

        let background_color = state.theme.code_background;
        let pad_x = px(4.);
        let pad_y = px(1.5);
        let radius = px(4.);

        let mut seg_start = 0usize;
        for (wrap_line_ix, seg_end) in wrap_indices
            .into_iter()
            .chain(std::iter::once(line_text_len))
            .enumerate()
        {
            let seg_start_x = wrapped.unwrapped_layout.x_for_index(seg_start);
            let seg_end_x = wrapped.unwrapped_layout.x_for_index(seg_end);
            let line_width = seg_end_x - seg_start_x;
            let dx = match align {
                BlockAlign::Left => px(0.),
                BlockAlign::Center => (align_width - line_width) / 2.0,
                BlockAlign::Right => align_width - line_width,
            };

            let line_top = bounds.top() + line_height * wrap_line_ix as f32;
            let top = line_top + pad_y;
            let bottom = line_top + line_height - pad_y;
            if bottom <= top {
                continue;
            }

            for range in &merged {
                let start = range.start.max(seg_start);
                let end = range.end.min(seg_end);
                if start >= end {
                    continue;
                }

                let start_x = wrapped.unwrapped_layout.x_for_index(start) - seg_start_x;
                let end_x = wrapped.unwrapped_layout.x_for_index(end) - seg_start_x;
                let left = bounds.left() + dx + start_x - pad_x;
                let right = bounds.left() + dx + end_x + pad_x;
                if right <= left {
                    continue;
                }

                window.paint_quad(gpui::quad(
                    Bounds::from_corners(point(left, top), point(right, bottom)),
                    radius,
                    background_color,
                    gpui::Edges::default(),
                    gpui::transparent_black(),
                    gpui::BorderStyle::default(),
                ));
            }

            seg_start = seg_end;
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
        let state = self.state.read(cx);
        let text: SharedString = state.text.slice_line(self.row).to_string().into();
        self.text = text.clone();

        let base_text_style = window.text_style();
        let base_color = base_text_style.color;
        let render_text: SharedString = if text.is_empty() {
            // Ensure empty lines still have stable layout bounds so hit-testing/caret placement
            // work correctly (especially for list items where the prefix is rendered separately).
            " ".into()
        } else {
            text.clone()
        };

        let mut runs = Vec::new();

        if text.is_empty() {
            runs.push(base_text_style.to_run(render_text.len().max(1)));
        } else if let Some(block) = state.document.blocks.get(self.row) {
            let line_range = state.line_range(self.row);
            let marked = state
                .ime_marked_range
                .map(|range| range.start..range.end)
                .map(|marked| marked.start.max(line_range.start)..marked.end.min(line_range.end))
                .filter(|marked| marked.start < marked.end)
                .map(|marked| (marked.start - line_range.start)..(marked.end - line_range.start));

            let mut cursor = 0usize;
            for node in &block.inlines {
                let InlineNode::Text(text) = node;
                let len = text.text.len();
                if len == 0 {
                    continue;
                }

                let mut style = base_text_style.clone();

                if text.style.bold {
                    style.font_weight = FontWeight::BOLD;
                }
                if text.style.italic {
                    style.font_style = FontStyle::Italic;
                }

                if text.style.code {
                    style.font_family = monospace_font_family();
                }

                if let Some(bg) = text.style.bg {
                    style.background_color = Some(bg);
                }

                // Underline and links.
                if text.style.underline {
                    style.underline = Some(UnderlineStyle {
                        thickness: px(1.),
                        color: text.style.fg,
                        wavy: false,
                    });
                }
                if text.style.link.is_some() {
                    let link_color = text.style.fg.unwrap_or(state.theme.link);
                    style.color = link_color;
                    if let Some(underline) = style.underline.as_mut() {
                        if underline.color.is_none() {
                            underline.color = Some(link_color);
                        }
                    } else {
                        style.underline = Some(UnderlineStyle {
                            thickness: px(1.),
                            color: Some(link_color),
                            wavy: false,
                        });
                    }
                }

                if text.style.strikethrough {
                    style.strikethrough = Some(StrikethroughStyle {
                        thickness: px(1.),
                        color: text.style.fg,
                        ..Default::default()
                    });
                }

                if let Some(color) = text.style.fg {
                    style.color = color;
                    if let Some(underline) = style.underline.as_mut() {
                        if underline.color.is_none() {
                            underline.color = Some(color);
                        }
                    }
                }

                let seg_start = cursor;
                let seg_end = cursor + len;
                cursor = seg_end;

                let Some(marked) = marked.clone() else {
                    runs.push(style.to_run(len));
                    continue;
                };

                if seg_end <= marked.start || seg_start >= marked.end {
                    runs.push(style.to_run(len));
                    continue;
                }

                let before_len = marked.start.saturating_sub(seg_start);
                let mark_len = seg_end
                    .min(marked.end)
                    .saturating_sub(marked.start.max(seg_start));
                let after_len = seg_end.saturating_sub(marked.end.max(seg_start));

                if before_len > 0 {
                    runs.push(style.clone().to_run(before_len));
                }

                if mark_len > 0 {
                    let mut marked_style = style.clone();
                    marked_style.underline = Some(UnderlineStyle {
                        thickness: px(1.),
                        color: Some(base_color),
                        wavy: false,
                    });
                    runs.push(marked_style.to_run(mark_len));
                }

                if after_len > 0 {
                    runs.push(style.to_run(after_len));
                }
            }

            if cursor < render_text.len() {
                runs.push(base_text_style.to_run(render_text.len() - cursor));
            }
        }

        if runs.is_empty() {
            runs.push(base_text_style.to_run(render_text.len().max(1)));
        }

        self.styled_text = StyledText::new(render_text).with_runs(runs);
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

        let line_len = self.text.len();
        let line_start = self.state.read(cx).text.line_start_offset(self.row);
        let text_layout = self.styled_text.layout().clone();

        self.state.update(cx, |state, _| {
            if self.row >= state.layout_cache.len() {
                state.layout_cache.resize_with(self.row + 1, || None);
            }
            state.layout_cache[self.row] = Some(LineLayoutCache {
                bounds,
                start_offset: line_start,
                line_len,
                text_layout,
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
        let hitbox = prepaint;
        window.set_cursor_style(CursorStyle::IBeam, hitbox);

        let text_layout = self.styled_text.layout().clone();
        let (selection_local, caret_column, theme) = {
            let state = self.state.read(cx);
            Self::paint_inline_code_background(
                &state,
                self.row,
                self.text.len(),
                bounds,
                &text_layout,
                window,
            );
            let selection = state.ordered_selection();
            let line_range = state.line_range(self.row);

            let sel_start = selection.start.max(line_range.start);
            let sel_end = selection.end.min(line_range.end);
            let selection_local = (sel_start < sel_end)
                .then(|| (sel_start - line_range.start)..(sel_end - line_range.start));

            let caret_column = (!state.has_selection() && state.focus_handle.is_focused(window))
                .then(|| state.cursor())
                .map(|cursor| state.text.offset_to_point(cursor))
                .and_then(|cursor_point| {
                    (cursor_point.row == self.row).then_some(cursor_point.column)
                });

            (selection_local, caret_column, state.theme)
        };

        self.styled_text
            .paint(global_id, None, bounds, &mut (), &mut (), window, cx);

        if let Some(selection_local) = selection_local {
            let (start_pos, end_pos, line_height) = {
                let state = self.state.read(cx);
                (
                    state.aligned_position_for_row_col(self.row, selection_local.start),
                    state.aligned_position_for_row_col(self.row, selection_local.end),
                    text_layout.line_height(),
                )
            };
            if let (Some(start_pos), Some(end_pos)) = (start_pos, end_pos) {
                Self::paint_selection_positions(
                    start_pos,
                    end_pos,
                    &bounds,
                    line_height,
                    window,
                    theme.selection,
                );
            }
        }

        if let Some(caret_column) = caret_column {
            if let Some(pos) = self
                .state
                .read(cx)
                .aligned_position_for_row_col(self.row, caret_column)
            {
                let line_height = text_layout.line_height();
                let caret_bounds = Bounds::new(pos, size(px(1.5), line_height));
                window.paint_quad(gpui::quad(
                    caret_bounds,
                    px(0.),
                    window.text_style().color,
                    gpui::Edges::default(),
                    gpui::transparent_black(),
                    gpui::BorderStyle::default(),
                ));
            }
        }

        // Mouse selection is handled by `RichTextInputHandlerElement`.
    }
}

pub(crate) struct DividerLineElement {
    state: gpui::Entity<RichTextState>,
    row: usize,
    rule_color: gpui::Hsla,
    text: SharedString,
    styled_text: StyledText,
}

impl DividerLineElement {
    pub(crate) fn new(
        state: gpui::Entity<RichTextState>,
        row: usize,
        rule_color: gpui::Hsla,
    ) -> Self {
        Self {
            state,
            row,
            rule_color,
            text: SharedString::default(),
            styled_text: StyledText::new(SharedString::default()),
        }
    }
}

impl IntoElement for DividerLineElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for DividerLineElement {
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
        let state = self.state.read(cx);
        let text: SharedString = state.text.slice_line(self.row).to_string().into();
        self.text = text;

        // Keep a 1-char placeholder so we have a stable TextLayout for hit-testing/caret.
        let placeholder: SharedString = " ".into();
        let text_style = window.text_style();
        self.styled_text = StyledText::new(placeholder).with_runs(vec![text_style.to_run(1)]);
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

        let line_len = self.text.len();
        let line_start = self.state.read(cx).text.line_start_offset(self.row);
        let text_layout = self.styled_text.layout().clone();

        self.state.update(cx, |state, _| {
            if self.row >= state.layout_cache.len() {
                state.layout_cache.resize_with(self.row + 1, || None);
            }
            state.layout_cache[self.row] = Some(LineLayoutCache {
                bounds,
                start_offset: line_start,
                line_len,
                text_layout,
            });
        });

        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let hitbox = prepaint;
        window.set_cursor_style(CursorStyle::IBeam, hitbox);

        let line_height = self.styled_text.layout().line_height();
        let y = bounds.top() + (line_height - px(1.)) / 2.0;
        let rule_bounds =
            Bounds::from_corners(point(bounds.left(), y), point(bounds.right(), y + px(1.)));

        window.paint_quad(gpui::quad(
            rule_bounds,
            px(0.),
            self.rule_color,
            gpui::Edges::default(),
            gpui::transparent_black(),
            gpui::BorderStyle::default(),
        ));

        // Mouse selection is handled by `RichTextInputHandlerElement`.
    }
}

pub(crate) struct TodoCheckboxElement {
    state: gpui::Entity<RichTextState>,
    row: usize,
    checked: bool,
    styled_text: StyledText,
}

impl TodoCheckboxElement {
    pub(crate) fn new(state: gpui::Entity<RichTextState>, row: usize, checked: bool) -> Self {
        let glyph: SharedString = if checked { "☑".into() } else { "☐".into() };
        Self {
            state,
            row,
            checked,
            styled_text: StyledText::new(glyph),
        }
    }
}

impl IntoElement for TodoCheckboxElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TodoCheckboxElement {
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
        let glyph: SharedString = if self.checked {
            "☑".into()
        } else {
            "☐".into()
        };
        let text_style = window.text_style();
        // `to_run` expects a byte length; checkbox glyphs are multi-byte UTF-8.
        let run_len = glyph.len().max(1);
        self.styled_text = StyledText::new(glyph).with_runs(vec![text_style.to_run(run_len)]);
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
        self.styled_text
            .paint(global_id, None, bounds, &mut (), &mut (), window, cx);

        let hitbox = prepaint.clone();
        window.set_cursor_style(CursorStyle::PointingHand, &hitbox);

        window.on_mouse_event({
            let state = self.state.clone();
            let row = self.row;
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
                    this.toggle_todo_checked(row, window, cx);
                });
            }
        });
    }
}

pub(crate) struct ToggleChevronElement {
    state: gpui::Entity<RichTextState>,
    row: usize,
    collapsed: bool,
    styled_text: StyledText,
}

impl ToggleChevronElement {
    pub(crate) fn new(state: gpui::Entity<RichTextState>, row: usize, collapsed: bool) -> Self {
        let glyph: SharedString = if collapsed {
            "▸".into()
        } else {
            "▾".into()
        };
        Self {
            state,
            row,
            collapsed,
            styled_text: StyledText::new(glyph),
        }
    }
}

impl IntoElement for ToggleChevronElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ToggleChevronElement {
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
        let glyph: SharedString = if self.collapsed {
            "▸".into()
        } else {
            "▾".into()
        };
        let text_style = window.text_style();
        let run_len = glyph.len().max(1);
        self.styled_text = StyledText::new(glyph).with_runs(vec![text_style.to_run(run_len)]);
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
        self.styled_text
            .paint(global_id, None, bounds, &mut (), &mut (), window, cx);

        let hitbox = prepaint.clone();
        window.set_cursor_style(CursorStyle::PointingHand, &hitbox);

        window.on_mouse_event({
            let state = self.state.clone();
            let row = self.row;
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
                    this.toggle_toggle_collapsed(row, window, cx);
                });
            }
        });
    }
}

impl Render for RichTextState {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let state = cx.entity().clone();
        let theme = self.theme;
        let muted_foreground = theme.muted_foreground;
        let base_text_size = window.text_style().font_size.to_pixels(window.rem_size());

        let mut ordered_counter = 1usize;
        let mut ordered_style: Option<OrderedListStyle> = None;
        let mut ordered_indent: Option<u8> = None;
        let blocks = self
            .document
            .blocks
            .iter()
            .map(|block| block.format)
            .collect::<Vec<_>>();

        if self.layout_cache.len() != blocks.len() {
            self.layout_cache.resize_with(blocks.len(), || None);
        }
        for cache in &mut self.layout_cache {
            *cache = None;
        }

        let indent_width = 18.0f32;
        let mut hidden_indent_stack: Vec<u8> = Vec::new();

        let mut rendered_blocks: Vec<AnyElement> = Vec::with_capacity(blocks.len());
        let mut row = 0usize;

        let render_block = |row: usize,
                            block: crate::document::BlockFormat,
                            ordered_counter: &mut usize,
                            ordered_style: &mut Option<OrderedListStyle>,
                            ordered_indent: &mut Option<u8>| {
            let line: AnyElement = match block.kind {
                BlockKind::Divider => {
                    DividerLineElement::new(state.clone(), row, theme.border).into_any_element()
                }
                _ => RichTextLineElement::new(state.clone(), row).into_any_element(),
            };

            let mut content = div().child(line);
            content = match block.kind {
                BlockKind::Heading { level } => match level {
                    1 => content
                        .text_size(px(f32::from(base_text_size) * 1.8))
                        .font_weight(gpui::FontWeight::SEMIBOLD),
                    2 => content
                        .text_size(px(f32::from(base_text_size) * 1.4))
                        .font_weight(gpui::FontWeight::SEMIBOLD),
                    3 => content
                        .text_size(px(f32::from(base_text_size) * 1.2))
                        .font_weight(gpui::FontWeight::SEMIBOLD),
                    4 => content
                        .text_size(px(f32::from(base_text_size) * 1.1))
                        .font_weight(gpui::FontWeight::SEMIBOLD),
                    5 => content
                        .text_size(px(f32::from(base_text_size) * 1.05))
                        .font_weight(gpui::FontWeight::SEMIBOLD),
                    _ => content.font_weight(gpui::FontWeight::SEMIBOLD),
                },
                _ => match block.size {
                    BlockTextSize::Small => {
                        content.text_size(px(f32::from(base_text_size) * 0.875))
                    }
                    BlockTextSize::Normal => content,
                    BlockTextSize::Large => {
                        content.text_size(px(f32::from(base_text_size) * 1.125))
                    }
                },
            };

            content = match block.align {
                BlockAlign::Left => content.text_left(),
                BlockAlign::Center => content.text_center(),
                BlockAlign::Right => content.text_right(),
            };

            let mut reset_list_state = || {
                *ordered_counter = 1;
                *ordered_style = None;
                *ordered_indent = None;
            };

            let mut block_el: AnyElement = match block.kind {
                BlockKind::Divider => {
                    reset_list_state();
                    div().w_full().py(px(6.)).child(content).into_any_element()
                }
                BlockKind::UnorderedListItem => {
                    reset_list_state();
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .gap(px(6.))
                        .child(div().text_color(muted_foreground).child("•"))
                        .child(content.flex_1())
                        .into_any_element()
                }
                BlockKind::OrderedListItem => {
                    let style = block.ordered_list_style;
                    if *ordered_style != Some(style) || *ordered_indent != Some(block.indent) {
                        *ordered_counter = 1;
                        *ordered_style = Some(style);
                        *ordered_indent = Some(block.indent);
                    }
                    let prefix =
                        format!("{}.", format_ordered_list_counter(style, *ordered_counter));
                    *ordered_counter += 1;
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .gap(px(6.))
                        .child(div().text_color(muted_foreground).child(prefix))
                        .child(content.flex_1())
                        .into_any_element()
                }
                BlockKind::Todo { checked } => {
                    reset_list_state();
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .gap(px(6.))
                        .child(
                            div()
                                .text_color(muted_foreground)
                                .child(TodoCheckboxElement::new(state.clone(), row, checked)),
                        )
                        .child(content.flex_1())
                        .into_any_element()
                }
                BlockKind::Toggle { collapsed } => {
                    reset_list_state();
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .gap(px(6.))
                        .child(
                            div()
                                .text_color(muted_foreground)
                                .child(ToggleChevronElement::new(state.clone(), row, collapsed)),
                        )
                        .child(content.flex_1())
                        .into_any_element()
                }
                _ => {
                    reset_list_state();
                    content.into_any_element()
                }
            };

            if block.indent > 0 {
                let padding = px(indent_width * block.indent as f32);
                block_el = div().pl(padding).child(block_el).into_any_element();
            }

            block_el
        };

        'document: while row < blocks.len() {
            let block = blocks[row];

            while let Some(&indent) = hidden_indent_stack.last() {
                if block.indent > indent {
                    row += 1;
                    continue 'document;
                }
                hidden_indent_stack.pop();
            }

            if let Some(cols) = block.columns {
                let group_id = cols.group;
                let count = cols.count.max(2);

                let mut group_end = row;
                while group_end < blocks.len()
                    && blocks[group_end].columns.map(|c| c.group) == Some(group_id)
                {
                    group_end += 1;
                }

                let mut column_children: Vec<Vec<AnyElement>> =
                    (0..count).map(|_| Vec::new()).collect();

                let mut scan_row = row;
                'group: while scan_row < group_end {
                    let scan_block = blocks[scan_row];

                    while let Some(&indent) = hidden_indent_stack.last() {
                        if scan_block.indent > indent {
                            scan_row += 1;
                            continue 'group;
                        }
                        hidden_indent_stack.pop();
                    }

                    let el = render_block(
                        scan_row,
                        scan_block,
                        &mut ordered_counter,
                        &mut ordered_style,
                        &mut ordered_indent,
                    );

                    if let Some(cols) = scan_block.columns {
                        if let Some(col) = column_children.get_mut(cols.column as usize) {
                            col.push(el);
                        } else {
                            rendered_blocks.push(el);
                        }
                    }

                    if matches!(scan_block.kind, BlockKind::Toggle { collapsed: true }) {
                        hidden_indent_stack.push(scan_block.indent);
                    }

                    scan_row += 1;
                }

                let weights = self.column_weights_for_group(group_id, count);

                let mut columns_row = div().flex().flex_row().w_full();
                for (ix, children) in column_children.into_iter().enumerate() {
                    let weight = weights.get(ix).copied().unwrap_or(1.0);

                    let mut column = div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .min_w(px(0.))
                        .overflow_hidden()
                        .flex_1()
                        .children(children);
                    column.style().flex_grow = Some(weight);
                    columns_row = columns_row.child(column);

                    if ix + 1 < count as usize {
                        let handle_ix = ix;
                        columns_row = columns_row.child(
                            div()
                                .flex_none()
                                .w(px(12.))
                                .cursor_col_resize()
                                .on_mouse_down(MouseButton::Left, {
                                    let state = state.clone();
                                    move |event, window, cx| {
                                        cx.stop_propagation();
                                        let focus_handle = { state.read(cx).focus_handle.clone() };
                                        window.focus(&focus_handle);
                                        state.update(cx, |this, cx| {
                                            this.start_column_resize(
                                                group_id,
                                                count,
                                                handle_ix,
                                                event.position.x,
                                                cx,
                                            );
                                        });
                                    }
                                })
                                .child(
                                    div().flex().items_center().justify_center().h_full().child(
                                        div().w(px(2.)).h_full().bg(theme.border).rounded(px(1.)),
                                    ),
                                ),
                        );
                    }
                }

                rendered_blocks.push(columns_row.into_any_element());

                row = group_end;
                continue 'document;
            }

            if matches!(block.kind, BlockKind::Quote) {
                ordered_counter = 1;
                ordered_style = None;
                ordered_indent = None;

                let quote_indent = block.indent;
                let mut quote_lines: Vec<AnyElement> = Vec::new();

                while row < blocks.len()
                    && matches!(blocks[row].kind, BlockKind::Quote)
                    && blocks[row].columns.is_none()
                {
                    let quote_block = blocks[row];
                    let line = RichTextLineElement::new(state.clone(), row).into_any_element();
                    let mut content = div().child(line);

                    content = match quote_block.size {
                        BlockTextSize::Small => {
                            content.text_size(px(f32::from(base_text_size) * 0.875))
                        }
                        BlockTextSize::Normal => content,
                        BlockTextSize::Large => {
                            content.text_size(px(f32::from(base_text_size) * 1.125))
                        }
                    };

                    content = match quote_block.align {
                        BlockAlign::Left => content.text_left(),
                        BlockAlign::Center => content.text_center(),
                        BlockAlign::Right => content.text_right(),
                    };

                    quote_lines.push(
                        content
                            .text_color(theme.muted_foreground)
                            .italic()
                            .into_any_element(),
                    );
                    row += 1;
                }

                let quote_container = div()
                    .flex()
                    .flex_row()
                    .gap(px(12.))
                    .px(px(8.))
                    .py(px(2.))
                    .child(div().w(px(3.)).bg(theme.border).rounded(px(2.)))
                    .child(div().flex_1().flex_col().gap(px(4.)).children(quote_lines))
                    .into_any_element();

                if quote_indent > 0 {
                    rendered_blocks.push(
                        div()
                            .pl(px(indent_width * quote_indent as f32))
                            .child(quote_container)
                            .into_any_element(),
                    );
                } else {
                    rendered_blocks.push(quote_container);
                }

                continue 'document;
            }

            let el = render_block(
                row,
                block,
                &mut ordered_counter,
                &mut ordered_style,
                &mut ordered_indent,
            );
            rendered_blocks.push(el);

            if matches!(block.kind, BlockKind::Toggle { collapsed: true }) {
                hidden_indent_stack.push(block.indent);
            }

            row += 1;
        }

        div()
            .id("rich-text-state")
            .size_full()
            .relative()
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
                    .child(
                        div()
                            .id("rich-text-document")
                            .flex_col()
                            .w_full()
                            .gap(px(4.))
                            .children(rendered_blocks),
                    ),
            )
    }
}

fn format_ordered_list_counter(style: OrderedListStyle, counter: usize) -> String {
    match style {
        OrderedListStyle::Decimal => counter.to_string(),
        OrderedListStyle::LowerAlpha => to_alpha(counter, false),
        OrderedListStyle::UpperAlpha => to_alpha(counter, true),
        OrderedListStyle::LowerRoman => to_roman(counter, false),
        OrderedListStyle::UpperRoman => to_roman(counter, true),
    }
}

fn to_alpha(counter: usize, upper: bool) -> String {
    if counter == 0 {
        return "0".to_string();
    }

    let mut n = counter;
    let mut out = Vec::new();
    while n > 0 {
        n -= 1;
        let ch = (b'a' + (n % 26) as u8) as char;
        out.push(if upper { ch.to_ascii_uppercase() } else { ch });
        n /= 26;
    }
    out.into_iter().rev().collect()
}

fn to_roman(counter: usize, upper: bool) -> String {
    if counter == 0 {
        return "0".to_string();
    }

    let mut n = counter.min(3999);
    let symbols = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];

    let mut out = String::new();
    for (value, sym) in symbols {
        while n >= value {
            out.push_str(sym);
            n -= value;
        }
    }

    if upper { out.to_ascii_uppercase() } else { out }
}
