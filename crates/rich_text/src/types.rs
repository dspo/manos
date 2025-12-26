#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockAlign {
    #[default]
    Left,
    Center,
    Right,
}
