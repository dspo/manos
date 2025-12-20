mod editor;
mod element;
mod rope_ext;
mod selection;
mod state;
mod style;
mod theme;

pub use editor::*;
pub use state::*;
pub use style::InlineStyle;
pub use theme::RichTextTheme;

use gpui::App;

pub fn init(cx: &mut App) {
    state::init(cx);
}
