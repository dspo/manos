use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, DefiniteLength, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, StyleRefinement, Styled, Window, div, px,
};

use super::state::{CONTEXT, RichTextState};

/// A rich text editor element bound to a [`RichTextState`].
#[derive(IntoElement)]
pub struct RichTextEditor {
    state: Entity<RichTextState>,
    style: StyleRefinement,
    bordered: bool,
    padding: DefiniteLength,
}

impl Styled for RichTextEditor {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RichTextEditor {
    pub fn new(state: &Entity<RichTextState>) -> Self {
        Self {
            state: state.clone(),
            style: StyleRefinement::default(),
            bordered: true,
            padding: px(12.).into(),
        }
    }

    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    pub fn padding(mut self, padding: impl Into<DefiniteLength>) -> Self {
        self.padding = padding.into();
        self
    }
}

impl RenderOnce for RichTextEditor {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (focus_handle, theme) = {
            let state = self.state.read(cx);
            (state.focus_handle.clone(), state.theme)
        };

        div()
            .id(("rich-text-editor", self.state.entity_id()))
            .key_context(CONTEXT)
            .track_focus(&focus_handle)
            .tab_index(0)
            .w_full()
            .h_full()
            .when(self.bordered, |this| {
                this.bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .rounded(theme.radius)
            })
            .p(self.padding)
            .when(!window.is_inspector_picking(cx), |this| {
                this.on_action(window.listener_for(&self.state, RichTextState::backspace))
                    .on_action(window.listener_for(&self.state, RichTextState::delete))
                    .on_action(window.listener_for(&self.state, RichTextState::enter))
                    .on_action(window.listener_for(&self.state, RichTextState::escape))
                    .on_action(window.listener_for(&self.state, RichTextState::left))
                    .on_action(window.listener_for(&self.state, RichTextState::right))
                    .on_action(window.listener_for(&self.state, RichTextState::up))
                    .on_action(window.listener_for(&self.state, RichTextState::down))
                    .on_action(window.listener_for(&self.state, RichTextState::select_left))
                    .on_action(window.listener_for(&self.state, RichTextState::select_right))
                    .on_action(window.listener_for(&self.state, RichTextState::select_up))
                    .on_action(window.listener_for(&self.state, RichTextState::select_down))
                    .on_action(window.listener_for(&self.state, RichTextState::select_all))
                    .on_action(window.listener_for(&self.state, RichTextState::copy))
                    .on_action(window.listener_for(&self.state, RichTextState::cut))
                    .on_action(window.listener_for(&self.state, RichTextState::paste))
                    .on_action(window.listener_for(&self.state, RichTextState::undo))
                    .on_action(window.listener_for(&self.state, RichTextState::redo))
                    .on_action(window.listener_for(&self.state, RichTextState::toggle_bold))
                    .on_action(window.listener_for(&self.state, RichTextState::toggle_italic))
                    .on_action(window.listener_for(&self.state, RichTextState::toggle_underline))
                    .on_action(
                        window.listener_for(&self.state, RichTextState::toggle_strikethrough),
                    )
            })
            .child(self.state.clone())
    }
}
