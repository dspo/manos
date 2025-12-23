use crate::style::InlineStyle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockTextSize {
    Small,
    Normal,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderedListStyle {
    Decimal,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
}

impl Default for OrderedListStyle {
    fn default() -> Self {
        Self::Decimal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph,
    Heading { level: u8 },
    Quote,
    UnorderedListItem,
    OrderedListItem,
    Todo { checked: bool },
    Toggle { collapsed: bool },
    Divider,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockColumns {
    pub group: u64,
    pub count: u8,
    pub column: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockFormat {
    pub kind: BlockKind,
    pub size: BlockTextSize,
    pub ordered_list_style: OrderedListStyle,
    pub align: BlockAlign,
    pub indent: u8,
    pub columns: Option<BlockColumns>,
}

impl Default for BlockFormat {
    fn default() -> Self {
        Self {
            kind: BlockKind::Paragraph,
            size: BlockTextSize::Normal,
            ordered_list_style: OrderedListStyle::default(),
            align: BlockAlign::default(),
            indent: 0,
            columns: None,
        }
    }
}

impl BlockFormat {
    pub(crate) fn split_successor(self) -> Self {
        match self.kind {
            BlockKind::Heading { .. } | BlockKind::Divider => BlockFormat {
                kind: BlockKind::Paragraph,
                size: BlockTextSize::Normal,
                ordered_list_style: self.ordered_list_style,
                align: self.align,
                indent: self.indent,
                columns: self.columns,
            },
            BlockKind::Toggle { .. } => BlockFormat {
                kind: BlockKind::Paragraph,
                size: self.size,
                ordered_list_style: self.ordered_list_style,
                align: self.align,
                indent: self.indent.saturating_add(1),
                columns: self.columns,
            },
            BlockKind::Paragraph
            | BlockKind::Quote
            | BlockKind::UnorderedListItem
            | BlockKind::OrderedListItem
            | BlockKind::Todo { .. } => self,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RichTextDocument {
    pub blocks: Vec<BlockNode>,
}

impl Default for RichTextDocument {
    fn default() -> Self {
        Self {
            blocks: vec![BlockNode::default()],
        }
    }
}

impl RichTextDocument {
    pub fn from_plain_text(text: &str) -> Self {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let mut blocks: Vec<BlockNode> = text
            .split('\n')
            .map(|line| BlockNode::from_text(BlockFormat::default(), line))
            .collect();

        if blocks.is_empty() {
            blocks.push(BlockNode::default());
        }

        Self { blocks }
    }

    pub fn to_plain_text(&self) -> String {
        let mut out = String::new();
        for (ix, block) in self.blocks.iter().enumerate() {
            if ix > 0 {
                out.push('\n');
            }
            out.push_str(&block.to_plain_text());
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockNode {
    pub format: BlockFormat,
    pub inlines: Vec<InlineNode>,
}

impl Default for BlockNode {
    fn default() -> Self {
        Self::from_text(BlockFormat::default(), "")
    }
}

impl BlockNode {
    pub fn from_text(format: BlockFormat, text: &str) -> Self {
        Self {
            format,
            inlines: vec![InlineNode::Text(TextNode {
                text: text.to_string(),
                style: InlineStyle::default(),
            })],
        }
    }

    pub fn to_plain_text(&self) -> String {
        let mut out = String::new();
        for node in &self.inlines {
            match node {
                InlineNode::Text(text) => out.push_str(&text.text),
            }
        }
        out
    }

    pub fn text_len(&self) -> usize {
        self.inlines.iter().fold(0usize, |acc, node| match node {
            InlineNode::Text(text) => acc + text.text.len(),
        })
    }

    pub fn is_text_empty(&self) -> bool {
        self.text_len() == 0
    }

    pub fn normalize(&mut self) {
        let mut normalized: Vec<InlineNode> = Vec::with_capacity(self.inlines.len());

        for node in self.inlines.drain(..) {
            match node {
                InlineNode::Text(text) => {
                    if let Some(InlineNode::Text(prev)) = normalized.last_mut() {
                        if prev.style == text.style {
                            prev.text.push_str(&text.text);
                            continue;
                        }
                    }
                    normalized.push(InlineNode::Text(text));
                }
            }
        }

        let has_any_text = normalized.iter().any(|node| match node {
            InlineNode::Text(text) => !text.text.is_empty(),
        });

        if has_any_text {
            normalized.retain(|node| match node {
                InlineNode::Text(text) => !text.text.is_empty(),
            });
        }

        if normalized.is_empty() {
            normalized.push(InlineNode::Text(TextNode {
                text: String::new(),
                style: InlineStyle::default(),
            }));
        }

        self.inlines = normalized;
    }

    pub fn last_style(&self) -> Option<&InlineStyle> {
        self.inlines.iter().rev().find_map(|node| match node {
            InlineNode::Text(text) => (!text.text.is_empty()).then_some(&text.style),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum InlineNode {
    Text(TextNode),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextNode {
    pub text: String,
    pub style: InlineStyle,
}
