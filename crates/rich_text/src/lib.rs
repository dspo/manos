mod editor;
mod state;
mod types;

pub use editor::*;
pub use state::*;
pub use types::*;

pub use gpui_plate_core::{
    Document, ElementNode, Marks, Node, PlateValue, Point, Selection, TextNode,
};

pub type RichTextValue = PlateValue;

use gpui::App;

pub fn init(cx: &mut App) {
    state::init(cx);
}
