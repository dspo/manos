use std::ops::Range;

use gpui::{
    div, point, px, size, App, Bounds, CursorStyle, Element, ElementId, ElementInputHandler,
    GlobalElementId, HighlightStyle, Hitbox, HitboxBehavior, InspectorElementId,
    InteractiveElement as _, IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement as _, Pixels, Point, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, StyledText, TextLayout, Window,
};

use crate::rope_ext::RopeExt as _;

use super::state::{BlockKind, BlockTextSize, LineLayoutCache, RichTextState};

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
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
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

                let focus_handle = { state.read(cx).focus_handle.clone() };
                window.focus(&focus_handle);

                state.update(cx, |this, cx| {
                    let offset = this
                        .offset_for_point(event.position)
                        .unwrap_or(this.text.len());

                    if event.modifiers.shift {
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
                let selecting = state.read(cx).selecting;
                if !selecting {
                    return;
                }
                if let Some(offset) = state.read(cx).offset_for_point(event.position) {
                    state.update(cx, |this, cx| {
                        this.selection.end = offset;
                        cx.notify();
                        this.scroll_cursor_into_view(window, cx);
                    });
                }
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
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    styled_text: StyledText,
}

impl RichTextLineElement {
    pub(crate) fn new(state: gpui::Entity<RichTextState>, row: usize) -> Self {
        Self {
            state,
            row,
            text: SharedString::default(),
            highlights: Vec::new(),
            styled_text: StyledText::new(SharedString::default()),
        }
    }

    fn paint_selection(
        selection: Range<usize>,
        text_layout: &TextLayout,
        bounds: &Bounds<Pixels>,
        window: &mut Window,
        selection_color: gpui::Hsla,
    ) {
        if selection.is_empty() {
            return;
        }

        let mut start = selection.start;
        let mut end = selection.end;
        if end < start {
            std::mem::swap(&mut start, &mut end);
        }

        let Some(start_position) = text_layout.position_for_index(start) else {
            return;
        };
        let Some(end_position) = text_layout.position_for_index(end) else {
            return;
        };

        let line_height = text_layout.line_height();

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

    fn caret_position(
        layout: &TextLayout,
        line_len: usize,
        local_offset: usize,
    ) -> Option<Point<Pixels>> {
        layout
            .position_for_index(local_offset.min(line_len))
            .or_else(|| layout.position_for_index(line_len))
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
        let highlights = state.highlight_styles_for_line(self.row, window.text_style().color);
        self.text = text.clone();
        self.highlights = highlights.clone();

        let text_style = window.text_style();

        let mut runs = Vec::new();
        let mut ix = 0;
        for (range, highlight) in highlights.iter() {
            if ix < range.start {
                runs.push(text_style.clone().to_run(range.start - ix));
            }
            runs.push(text_style.clone().highlight(*highlight).to_run(range.len()));
            ix = range.end;
        }
        if ix < text.len() {
            runs.push(text_style.to_run(text.len() - ix));
        }

        self.styled_text = StyledText::new(text).with_runs(runs);
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
        self.styled_text
            .paint(global_id, None, bounds, &mut (), &mut (), window, cx);

        let hitbox = prepaint;
        window.set_cursor_style(CursorStyle::IBeam, hitbox);

        let text_layout = self.styled_text.layout().clone();
        let (selection_local, caret_column, theme) = {
            let state = self.state.read(cx);
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

        if let Some(selection_local) = selection_local {
            Self::paint_selection(
                selection_local,
                &text_layout,
                &bounds,
                window,
                theme.selection,
            );
        }

        if let Some(caret_column) = caret_column {
            let line_len = self.text.len();
            if let Some(pos) = Self::caret_position(&text_layout, line_len, caret_column) {
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

impl Render for RichTextState {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let state = cx.entity().clone();
        let muted_foreground = self.theme.muted_foreground;
        let base_text_size = window.text_style().font_size.to_pixels(window.rem_size());

        let mut ordered_counter = 1usize;
        let blocks = self.blocks.clone();

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
                            .children(blocks.into_iter().enumerate().map(move |(row, block)| {
                                let line = RichTextLineElement::new(state.clone(), row);
                                let content = div().flex_1().child(line);

                                let content =
                                    match block.kind {
                                        BlockKind::Heading { level } => match level {
                                            1 => content
                                                .text_size(px(f32::from(base_text_size) * 1.8))
                                                .font_weight(gpui::FontWeight::SEMIBOLD),
                                            2 => content
                                                .text_size(px(f32::from(base_text_size) * 1.4))
                                                .font_weight(gpui::FontWeight::SEMIBOLD),
                                            _ => content
                                                .text_size(px(f32::from(base_text_size) * 1.2))
                                                .font_weight(gpui::FontWeight::SEMIBOLD),
                                        },
                                        _ => match block.size {
                                            BlockTextSize::Small => content
                                                .text_size(px(f32::from(base_text_size) * 0.875)),
                                            BlockTextSize::Normal => content,
                                            BlockTextSize::Large => content
                                                .text_size(px(f32::from(base_text_size) * 1.125)),
                                        },
                                    };

                                match block.kind {
                                    BlockKind::UnorderedListItem => {
                                        ordered_counter = 1;
                                        div()
                                            .flex_row()
                                            .items_start()
                                            .gap(px(8.))
                                            .child(
                                                div()
                                                    .w(px(28.))
                                                    .text_color(muted_foreground)
                                                    .child("•"),
                                            )
                                            .child(content)
                                    }
                                    BlockKind::OrderedListItem => {
                                        let prefix = format!("{}.", ordered_counter);
                                        ordered_counter += 1;

                                        div()
                                            .flex_row()
                                            .items_start()
                                            .gap(px(8.))
                                            .child(
                                                div()
                                                    .w(px(28.))
                                                    .text_color(muted_foreground)
                                                    .child(prefix),
                                            )
                                            .child(content)
                                    }
                                    _ => {
                                        ordered_counter = 1;
                                        content
                                    }
                                }
                            })),
                    ),
            )
    }
}
