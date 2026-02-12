//! Plugin modules for plate-core.
//!
//! This module contains the individual plugin implementations for the rich text editor.
//! Each plugin provides specific functionality:
//!
//! - [`BlockSelectionPlugin`] - Block-level selection and manipulation
//! - [`DndPlugin`] - Drag and drop support for blocks
//! - [`FindReplacePlugin`] - Find and replace text functionality
//! - [`MarkdownPlugin`] - Markdown import/export support
//! - [`MathPlugin`] - Mathematical formula support (LaTeX)
//! - [`SlashCommandPlugin`] - Slash command menu support

mod block_selection;
mod dnd;
mod find_replace;
mod markdown;
mod math;
mod slash_command;

pub use block_selection::BlockSelectionPlugin;
pub use dnd::{CanDropArgs, CanDropResult, DndPlugin, MoveNodeArgs};
pub use find_replace::FindReplacePlugin;
pub use markdown::MarkdownPlugin;
pub use math::MathPlugin;
pub use slash_command::SlashCommandPlugin;

use crate::core::{Document, ElementNode, Node};
use crate::ops::Path;
use crate::plugin::{NodeRole, PluginRegistry};

/// Get a node at the specified path in the document.
pub(super) fn node_at_path<'a>(doc: &'a Document, path: &[usize]) -> Option<&'a Node> {
    if path.is_empty() {
        return None;
    }
    let mut node = doc.children.get(path[0])?;
    for &ix in path.iter().skip(1) {
        node = match node {
            Node::Element(el) => el.children.get(ix)?,
            Node::Void(_) | Node::Text(_) => return None,
        };
    }
    Some(node)
}

/// Clamp an index to a valid char boundary in a string.
pub(super) fn clamp_to_char_boundary(s: &str, mut ix: usize) -> usize {
    if ix >= s.len() {
        return s.len();
    }
    while !s.is_char_boundary(ix) && ix > 0 {
        ix -= 1;
    }
    ix
}

/// A text block with its path and element reference.
pub(super) struct TextBlock<'a> {
    pub path: Path,
    pub el: &'a ElementNode,
}

/// Get all text blocks in document order.
pub(super) fn text_blocks_in_order<'a>(doc: &'a Document, registry: &PluginRegistry) -> Vec<TextBlock<'a>> {
    fn walk<'a>(
        nodes: &'a [Node],
        path: &mut Vec<usize>,
        out: &mut Vec<TextBlock<'a>>,
        registry: &PluginRegistry,
    ) {
        for (ix, node) in nodes.iter().enumerate() {
            path.push(ix);

            if let Node::Element(el) = node {
                let is_block = registry
                    .node_specs()
                    .get(&el.kind)
                    .map(|s| s.role == NodeRole::Block)
                    .unwrap_or(true);

                if is_block {
                    // Check if this block has inline children (text block)
                    let has_inline = el.children.iter().any(|c| {
                        matches!(c, Node::Text(_) | Node::Void(_))
                    });
                    if has_inline {
                        out.push(TextBlock {
                            path: path.clone(),
                            el,
                        });
                    }
                }

                walk(&el.children, path, out, registry);
            }

            path.pop();
        }
    }

    let mut out = Vec::new();
    walk(&doc.children, &mut Vec::new(), &mut out, registry);
    out
}
