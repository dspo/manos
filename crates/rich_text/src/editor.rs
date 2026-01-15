use gpui::{App, Entity, IntoElement, RenderOnce, Window};

use crate::RichTextState;

/// A rich text editor element bound to a [`RichTextState`].
#[derive(IntoElement)]
pub struct RichTextEditor {
    state: Entity<RichTextState>,
}

impl RichTextEditor {
    pub fn new(state: &Entity<RichTextState>) -> Self {
        Self {
            state: state.clone(),
        }
    }
}

impl RenderOnce for RichTextEditor {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        self.state
    }
}
