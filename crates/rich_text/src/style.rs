use gpui::Hsla;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct InlineStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub fg: Option<Hsla>,
    pub bg: Option<Hsla>,
    pub link: Option<String>,
}

impl InlineStyle {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}
