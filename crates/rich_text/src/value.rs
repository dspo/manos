use gpui::{Hsla, Rgba};
use serde::{Deserialize, Serialize};

use crate::document::{
    BlockFormat, BlockKind, BlockNode, BlockTextSize, InlineNode, RichTextDocument, TextNode,
};
use crate::style::InlineStyle;

const DEFAULT_SCHEMA: &str = "slate";
const DEFAULT_VERSION: u32 = 1;

fn default_schema() -> String {
    DEFAULT_SCHEMA.to_string()
}

fn default_version() -> u32 {
    DEFAULT_VERSION
}

/// A versioned, Slate-compatible JSON wrapper for persisting rich text documents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RichTextValue {
    #[serde(default = "default_schema")]
    pub schema: String,
    #[serde(default = "default_version")]
    pub version: u32,
    pub document: Vec<SlateNode>,
}

impl RichTextValue {
    pub fn from_document(document: &RichTextDocument) -> Self {
        let mut out: Vec<SlateNode> = Vec::new();

        let mut ix = 0usize;
        while ix < document.blocks.len() {
            let block = &document.blocks[ix];
            match block.format.kind {
                BlockKind::Quote => {
                    let mut items = Vec::new();
                    while ix < document.blocks.len()
                        && matches!(document.blocks[ix].format.kind, BlockKind::Quote)
                    {
                        items.push(SlateNode::Element(block_to_quote_paragraph(
                            &document.blocks[ix],
                        )));
                        ix += 1;
                    }
                    out.push(SlateNode::Element(SlateElement {
                        kind: "blockquote".to_string(),
                        children: items,
                        ..Default::default()
                    }));
                }
                BlockKind::UnorderedListItem => {
                    let mut items = Vec::new();
                    while ix < document.blocks.len()
                        && matches!(
                            document.blocks[ix].format.kind,
                            BlockKind::UnorderedListItem
                        )
                    {
                        items.push(SlateNode::Element(block_to_list_item(&document.blocks[ix])));
                        ix += 1;
                    }
                    out.push(SlateNode::Element(SlateElement {
                        kind: "bulleted-list".to_string(),
                        children: items,
                        ..Default::default()
                    }));
                }
                BlockKind::OrderedListItem => {
                    let mut items = Vec::new();
                    while ix < document.blocks.len()
                        && matches!(document.blocks[ix].format.kind, BlockKind::OrderedListItem)
                    {
                        items.push(SlateNode::Element(block_to_list_item(&document.blocks[ix])));
                        ix += 1;
                    }
                    out.push(SlateNode::Element(SlateElement {
                        kind: "numbered-list".to_string(),
                        children: items,
                        ..Default::default()
                    }));
                }
                _ => {
                    out.push(SlateNode::Element(block_to_element(block)));
                    ix += 1;
                }
            }
        }

        Self {
            schema: default_schema(),
            version: default_version(),
            document: out,
        }
    }

    pub fn from_slate_document(document: Vec<SlateNode>) -> Self {
        Self {
            schema: default_schema(),
            version: default_version(),
            document,
        }
    }

    pub fn into_document(self) -> RichTextDocument {
        slate_document_to_rich_text_document(&self.document)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum SlateNode {
    Text(SlateText),
    Element(SlateElement),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SlateElement {
    #[serde(rename = "type", default)]
    pub kind: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,

    #[serde(default, rename = "textSize", skip_serializing_if = "Option::is_none")]
    pub text_size: Option<String>,

    #[serde(default, rename = "listType", skip_serializing_if = "Option::is_none")]
    pub list_type: Option<String>,

    #[serde(default)]
    pub children: Vec<SlateNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SlateText {
    pub text: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "strikeThrough"
    )]
    pub strikethrough: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<SlateColor>,
    #[serde(
        default,
        rename = "backgroundColor",
        skip_serializing_if = "Option::is_none"
    )]
    pub background_color: Option<SlateColor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum SlateColor {
    Rgba(Rgba),
    Css(String),
}

fn block_to_element(block: &BlockNode) -> SlateElement {
    let (kind, level) = match block.format.kind {
        BlockKind::Paragraph => ("paragraph".to_string(), None),
        BlockKind::Heading { level } => ("heading".to_string(), Some(level)),
        BlockKind::Quote => ("paragraph".to_string(), None),
        BlockKind::UnorderedListItem => ("list-item".to_string(), None),
        BlockKind::OrderedListItem => ("list-item".to_string(), None),
        BlockKind::Todo { .. } => ("todo-list-item".to_string(), None),
        BlockKind::Divider => ("divider".to_string(), None),
    };
    let checked = match block.format.kind {
        BlockKind::Todo { checked } => Some(checked),
        _ => None,
    };

    let text_size = block_text_size_to_string(block.format.size);
    let list_type = match block.format.kind {
        BlockKind::UnorderedListItem => Some("unordered".to_string()),
        BlockKind::OrderedListItem => Some("ordered".to_string()),
        _ => None,
    };

    SlateElement {
        kind,
        level,
        checked,
        text_size,
        list_type,
        children: if matches!(block.format.kind, BlockKind::Divider) {
            Vec::new()
        } else {
            inlines_to_slate_children(&block.inlines)
        },
    }
}

fn block_to_quote_paragraph(block: &BlockNode) -> SlateElement {
    SlateElement {
        kind: "paragraph".to_string(),
        text_size: block_text_size_to_string(block.format.size),
        children: inlines_to_slate_children(&block.inlines),
        ..Default::default()
    }
}

fn block_to_list_item(block: &BlockNode) -> SlateElement {
    SlateElement {
        kind: "list-item".to_string(),
        text_size: block_text_size_to_string(block.format.size),
        children: inlines_to_slate_children(&block.inlines),
        ..Default::default()
    }
}

fn inlines_to_slate_children(inlines: &[InlineNode]) -> Vec<SlateNode> {
    let mut children = Vec::new();
    for node in inlines {
        match node {
            InlineNode::Text(text) => children.push(SlateNode::Text(text_node_to_slate(text))),
        }
    }
    if children.is_empty() {
        children.push(SlateNode::Text(SlateText {
            text: String::new(),
            ..Default::default()
        }));
    }
    children
}

fn text_node_to_slate(text: &TextNode) -> SlateText {
    SlateText {
        text: text.text.clone(),
        bold: text.style.bold.then_some(true),
        italic: text.style.italic.then_some(true),
        underline: text.style.underline.then_some(true),
        strikethrough: text.style.strikethrough.then_some(true),
        color: text.style.fg.map(|hsla| SlateColor::Rgba(Rgba::from(hsla))),
        background_color: text.style.bg.map(|hsla| SlateColor::Rgba(Rgba::from(hsla))),
    }
}

fn slate_document_to_rich_text_document(nodes: &[SlateNode]) -> RichTextDocument {
    let mut blocks: Vec<BlockNode> = Vec::new();
    let mut pending_inlines: Vec<InlineNode> = Vec::new();

    let flush_pending = |blocks: &mut Vec<BlockNode>, pending: &mut Vec<InlineNode>| {
        if pending.is_empty() {
            return;
        }
        let mut block = BlockNode {
            format: BlockFormat::default(),
            inlines: std::mem::take(pending),
        };
        block.normalize();
        blocks.push(block);
    };

    for node in nodes {
        match node {
            SlateNode::Text(text) => {
                pending_inlines.push(InlineNode::Text(slate_text_to_text_node(text)));
            }
            SlateNode::Element(element) => {
                flush_pending(&mut blocks, &mut pending_inlines);
                blocks.extend(blocks_from_element(element));
            }
        }
    }

    flush_pending(&mut blocks, &mut pending_inlines);

    if blocks.is_empty() {
        blocks.push(BlockNode::default());
    }

    RichTextDocument { blocks }
}

fn blocks_from_element(element: &SlateElement) -> Vec<BlockNode> {
    match element.kind.as_str() {
        "bulleted-list" | "unordered-list" | "ul" => {
            list_container_to_blocks(element, BlockKind::UnorderedListItem)
        }
        "numbered-list" | "ordered-list" | "ol" => {
            list_container_to_blocks(element, BlockKind::OrderedListItem)
        }
        "blockquote" | "quote" => quote_container_to_blocks(element),
        "divider" | "hr" => vec![element_to_divider_block()],
        _ => vec![element_to_block(element, None)],
    }
}

fn quote_container_to_blocks(element: &SlateElement) -> Vec<BlockNode> {
    let mut blocks = Vec::new();

    // Slate-style: blockquote contains block children (paragraphs, headings, etc).
    for child in &element.children {
        match child {
            SlateNode::Element(child_el) => {
                blocks.push(element_to_block(child_el, Some(BlockKind::Quote)))
            }
            SlateNode::Text(text) => {
                let mut block = BlockNode {
                    format: BlockFormat {
                        kind: BlockKind::Quote,
                        size: BlockTextSize::Normal,
                    },
                    inlines: vec![InlineNode::Text(slate_text_to_text_node(text))],
                };
                block.normalize();
                blocks.push(block);
            }
        }
    }

    if blocks.is_empty() {
        blocks.push(BlockNode {
            format: BlockFormat {
                kind: BlockKind::Quote,
                size: BlockTextSize::Normal,
            },
            ..BlockNode::default()
        });
    }

    blocks
}

fn element_to_divider_block() -> BlockNode {
    let mut block = BlockNode::default();
    block.format.kind = BlockKind::Divider;
    block.normalize();
    block
}

fn list_container_to_blocks(element: &SlateElement, kind: BlockKind) -> Vec<BlockNode> {
    let mut blocks = Vec::new();

    for child in &element.children {
        match child {
            SlateNode::Element(item) => {
                if is_list_item(item) {
                    blocks.push(element_to_block(item, Some(kind)));
                } else {
                    blocks.push(element_to_block(item, Some(kind)));
                }
            }
            SlateNode::Text(text) => {
                let mut block = BlockNode {
                    format: BlockFormat {
                        kind,
                        size: BlockTextSize::Normal,
                    },
                    inlines: vec![InlineNode::Text(slate_text_to_text_node(text))],
                };
                block.normalize();
                blocks.push(block);
            }
        }
    }

    if blocks.is_empty() {
        let mut block = BlockNode {
            format: BlockFormat {
                kind,
                size: BlockTextSize::Normal,
            },
            inlines: vec![InlineNode::Text(TextNode {
                text: String::new(),
                style: InlineStyle::default(),
            })],
        };
        block.normalize();
        blocks.push(block);
    }

    blocks
}

fn is_list_item(element: &SlateElement) -> bool {
    matches!(element.kind.as_str(), "list-item" | "li")
}

fn element_to_block(element: &SlateElement, force_kind: Option<BlockKind>) -> BlockNode {
    let kind = force_kind.unwrap_or_else(|| parse_block_kind(element));
    let size = parse_text_size(element.text_size.as_deref());
    let format = BlockFormat { kind, size };

    let mut inlines = Vec::new();
    collect_text_leaves(&element.children, &mut inlines);
    if inlines.is_empty() {
        inlines.push(InlineNode::Text(TextNode {
            text: String::new(),
            style: InlineStyle::default(),
        }));
    }

    let mut block = BlockNode { format, inlines };
    block.normalize();
    block
}

fn collect_text_leaves(nodes: &[SlateNode], out: &mut Vec<InlineNode>) {
    for node in nodes {
        match node {
            SlateNode::Text(text) => out.push(InlineNode::Text(slate_text_to_text_node(text))),
            SlateNode::Element(element) => collect_text_leaves(&element.children, out),
        }
    }
}

fn parse_block_kind(element: &SlateElement) -> BlockKind {
    match element.kind.as_str() {
        "paragraph" | "p" => BlockKind::Paragraph,
        "heading" => BlockKind::Heading {
            level: element.level.unwrap_or(1).max(1),
        },
        "blockquote" | "quote" => BlockKind::Quote,
        "h1" => BlockKind::Heading { level: 1 },
        "h2" => BlockKind::Heading { level: 2 },
        "h3" => BlockKind::Heading { level: 3 },
        "h4" => BlockKind::Heading { level: 4 },
        "h5" => BlockKind::Heading { level: 5 },
        "h6" => BlockKind::Heading { level: 6 },
        "divider" | "hr" => BlockKind::Divider,
        "todo-list-item" | "task-list-item" => BlockKind::Todo {
            checked: element.checked.unwrap_or(false),
        },
        "list-item" | "li" => match element.list_type.as_deref() {
            Some("ordered") => BlockKind::OrderedListItem,
            Some("unordered") => BlockKind::UnorderedListItem,
            _ => BlockKind::UnorderedListItem,
        },
        _ => BlockKind::Paragraph,
    }
}

fn block_text_size_to_string(size: BlockTextSize) -> Option<String> {
    match size {
        BlockTextSize::Small => Some("small".to_string()),
        BlockTextSize::Normal => None,
        BlockTextSize::Large => Some("large".to_string()),
    }
}

fn parse_text_size(size: Option<&str>) -> BlockTextSize {
    let Some(size) = size else {
        return BlockTextSize::Normal;
    };
    match size.to_ascii_lowercase().as_str() {
        "small" | "sm" => BlockTextSize::Small,
        "large" | "lg" => BlockTextSize::Large,
        _ => BlockTextSize::Normal,
    }
}

fn slate_text_to_text_node(text: &SlateText) -> TextNode {
    let fg = match text.color.as_ref() {
        Some(SlateColor::Rgba(rgba)) => Some(Hsla::from(*rgba)),
        _ => None,
    };
    let bg = match text.background_color.as_ref() {
        Some(SlateColor::Rgba(rgba)) => Some(Hsla::from(*rgba)),
        _ => None,
    };

    TextNode {
        text: text.text.clone(),
        style: InlineStyle {
            bold: text.bold.unwrap_or(false),
            italic: text.italic.unwrap_or(false),
            underline: text.underline.unwrap_or(false),
            strikethrough: text.strikethrough.unwrap_or(false),
            fg,
            bg,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_plain_text() {
        let source = "Hello\nRich Text";
        let document = RichTextDocument::from_plain_text(source);
        let value = RichTextValue::from_document(&document);
        let json = serde_json::to_string(&value).expect("serialize");
        let parsed: RichTextValue = serde_json::from_str(&json).expect("deserialize");
        let roundtrip = parsed.into_document();

        assert_eq!(roundtrip.to_plain_text(), source);
    }
}
