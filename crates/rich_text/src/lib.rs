mod document;
mod editor;
mod element;
mod rope_ext;
mod selection;
mod state;
mod style;
mod theme;
mod value;

pub use document::*;
pub use editor::*;
pub use state::*;
pub use style::InlineStyle;
pub use theme::RichTextTheme;
pub use value::*;

use gpui::App;

pub fn init(cx: &mut App) {
    state::init(cx);
}
