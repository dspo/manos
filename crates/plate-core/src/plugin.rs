use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::core::{Document, Node, Point, Selection};
use crate::ops::{Op, Transaction};

#[derive(Debug, Clone)]
pub struct CommandError {
    message: String,
}

impl CommandError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Clone)]
pub struct CommandSpec {
    pub id: String,
    pub label: String,
    pub handler: std::sync::Arc<
        dyn Fn(&mut crate::core::Editor, Option<serde_json::Value>) -> Result<(), CommandError>
            + Send
            + Sync,
    >,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    Block,
    Inline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChildConstraint {
    None,
    BlockOnly,
    InlineOnly,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeSpec {
    pub kind: String,
    pub role: NodeRole,
    pub is_void: bool,
    pub children: ChildConstraint,
}

pub trait NormalizePass: Send + Sync {
    fn id(&self) -> &'static str;
    fn run(&self, doc: &Document, registry: &PluginRegistry) -> Vec<Op>;
}

pub trait PlatePlugin: Send + Sync {
    fn id(&self) -> &'static str;
    fn node_specs(&self) -> Vec<NodeSpec> {
        Vec::new()
    }
    fn normalize_passes(&self) -> Vec<Box<dyn NormalizePass>> {
        Vec::new()
    }
    fn commands(&self) -> Vec<CommandSpec> {
        Vec::new()
    }
}

#[derive(Default)]
pub struct PluginRegistry {
    node_specs: HashMap<String, NodeSpec>,
    normalize_passes: Vec<Box<dyn NormalizePass>>,
    commands: HashMap<String, CommandSpec>,
}

impl PluginRegistry {
    pub fn new(plugins: impl IntoIterator<Item = Box<dyn PlatePlugin>>) -> Result<Self, String> {
        let mut registry = Self::default();
        for plugin in plugins {
            registry.register_plugin(plugin)?;
        }
        Ok(registry)
    }

    pub fn core() -> Self {
        let plugins: Vec<Box<dyn PlatePlugin>> = vec![
            Box::new(CoreParagraphPlugin),
            Box::new(CoreDividerPlugin),
            Box::new(CoreNormalizePlugin),
            Box::new(CoreCommandsPlugin),
        ];
        Self::new(plugins).expect("core registry must be valid")
    }

    pub fn register_plugin(&mut self, plugin: Box<dyn PlatePlugin>) -> Result<(), String> {
        for spec in plugin.node_specs() {
            if self.node_specs.contains_key(&spec.kind) {
                return Err(format!("Duplicate node spec kind: {}", spec.kind));
            }
            self.node_specs.insert(spec.kind.clone(), spec);
        }

        self.normalize_passes.extend(plugin.normalize_passes());

        for cmd in plugin.commands() {
            if self.commands.contains_key(&cmd.id) {
                return Err(format!("Duplicate command id: {}", cmd.id));
            }
            self.commands.insert(cmd.id.clone(), cmd);
        }

        Ok(())
    }

    pub fn node_specs(&self) -> &HashMap<String, NodeSpec> {
        &self.node_specs
    }

    pub fn normalize_passes(&self) -> &[Box<dyn NormalizePass>] {
        &self.normalize_passes
    }

    pub fn commands(&self) -> &HashMap<String, CommandSpec> {
        &self.commands
    }

    pub fn command(&self, id: &str) -> Option<CommandSpec> {
        self.commands.get(id).cloned()
    }

    pub fn normalize(&self, doc: &Document) -> Vec<Op> {
        let mut ops: Vec<Op> = Vec::new();
        for pass in &self.normalize_passes {
            ops.extend(pass.run(doc, self));
        }
        ops
    }

    pub fn normalize_selection(&self, doc: &Document, selection: &Selection) -> Selection {
        let fallback = first_text_point(doc).unwrap_or(Point {
            path: vec![0, 0],
            offset: 0,
        });

        let anchor =
            normalize_point_to_existing_text(doc, &selection.anchor).unwrap_or_else(|| {
                normalize_point_to_existing_text(doc, &selection.focus)
                    .unwrap_or_else(|| fallback.clone())
            });
        let focus = normalize_point_to_existing_text(doc, &selection.focus)
            .unwrap_or_else(|| anchor.clone());

        Selection { anchor, focus }
    }

    pub fn is_known_kind(&self, kind: &str) -> bool {
        self.node_specs.contains_key(kind)
    }
}

fn first_text_point(doc: &Document) -> Option<Point> {
    for (block_ix, node) in doc.children.iter().enumerate() {
        if let Node::Element(el) = node {
            for (child_ix, child) in el.children.iter().enumerate() {
                if matches!(child, Node::Text(_)) {
                    return Some(Point {
                        path: vec![block_ix, child_ix],
                        offset: 0,
                    });
                }
            }
        }
    }
    None
}

fn normalize_point_to_existing_text(doc: &Document, point: &Point) -> Option<Point> {
    let mut path = point.path.clone();
    if path.is_empty() {
        return None;
    }

    // Clamp to root child.
    if path[0] >= doc.children.len() {
        if doc.children.is_empty() {
            return None;
        }
        path[0] = doc.children.len() - 1;
        path.truncate(1);
    }

    let Some(root) = doc.children.get(path[0]) else {
        return None;
    };

    // Prefer text inside an element block.
    if let Node::Element(el) = root {
        let child_ix = path
            .get(1)
            .copied()
            .unwrap_or(0)
            .min(el.children.len().saturating_sub(1));
        if let Some(Node::Text(t)) = el.children.get(child_ix) {
            let offset = point.offset.min(t.text.len());
            return Some(Point {
                path: vec![path[0], child_ix],
                offset,
            });
        }

        // Otherwise find first text child.
        for (ix, child) in el.children.iter().enumerate() {
            if let Node::Text(_) = child {
                return Some(Point {
                    path: vec![path[0], ix],
                    offset: 0,
                });
            }
        }
    }

    None
}

struct CoreParagraphPlugin;

impl PlatePlugin for CoreParagraphPlugin {
    fn id(&self) -> &'static str {
        "core.paragraph"
    }

    fn node_specs(&self) -> Vec<NodeSpec> {
        vec![NodeSpec {
            kind: "paragraph".to_string(),
            role: NodeRole::Block,
            is_void: false,
            children: ChildConstraint::InlineOnly,
        }]
    }
}

struct CoreDividerPlugin;

impl PlatePlugin for CoreDividerPlugin {
    fn id(&self) -> &'static str {
        "core.divider"
    }

    fn node_specs(&self) -> Vec<NodeSpec> {
        vec![NodeSpec {
            kind: "divider".to_string(),
            role: NodeRole::Block,
            is_void: true,
            children: ChildConstraint::None,
        }]
    }
}

struct CoreNormalizePlugin;

impl PlatePlugin for CoreNormalizePlugin {
    fn id(&self) -> &'static str {
        "core.normalize"
    }

    fn normalize_passes(&self) -> Vec<Box<dyn NormalizePass>> {
        vec![
            Box::new(EnsureNonEmptyDocument),
            Box::new(EnsureParagraphHasTextLeaf),
        ]
    }
}

struct EnsureNonEmptyDocument;

impl NormalizePass for EnsureNonEmptyDocument {
    fn id(&self) -> &'static str {
        "core.ensure_non_empty_document"
    }

    fn run(&self, doc: &Document, _registry: &PluginRegistry) -> Vec<Op> {
        if doc.children.is_empty() {
            return vec![Op::InsertNode {
                path: vec![0],
                node: Node::paragraph(""),
            }];
        }
        Vec::new()
    }
}

struct EnsureParagraphHasTextLeaf;

impl NormalizePass for EnsureParagraphHasTextLeaf {
    fn id(&self) -> &'static str {
        "core.ensure_paragraph_has_text_leaf"
    }

    fn run(&self, doc: &Document, _registry: &PluginRegistry) -> Vec<Op> {
        let mut ops = Vec::new();
        for (block_ix, node) in doc.children.iter().enumerate() {
            let Node::Element(el) = node else { continue };
            if el.kind != "paragraph" {
                continue;
            }
            let has_text = el.children.iter().any(|n| matches!(n, Node::Text(_)));
            if !has_text {
                ops.push(Op::InsertNode {
                    path: vec![block_ix, 0],
                    node: Node::Text(crate::core::TextNode {
                        text: String::new(),
                        marks: crate::core::Marks::default(),
                    }),
                });
            }
        }
        ops
    }
}

struct CoreCommandsPlugin;

impl PlatePlugin for CoreCommandsPlugin {
    fn id(&self) -> &'static str {
        "core.commands"
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![CommandSpec {
            id: "core.insert_divider".to_string(),
            label: "Insert divider".to_string(),
            handler: std::sync::Arc::new(|editor, _args| {
                let block_ix = editor.selection().focus.path.first().copied().unwrap_or(0);
                let insert_at = block_ix.saturating_add(1).min(editor.doc().children.len());

                let divider_path = vec![insert_at];
                let paragraph_path = vec![insert_at + 1];

                let tx = Transaction::new(vec![
                    Op::InsertNode {
                        path: divider_path,
                        node: Node::divider(),
                    },
                    Op::InsertNode {
                        path: paragraph_path.clone(),
                        node: Node::paragraph(""),
                    },
                ])
                .selection_after(Selection::collapsed(Point::new(vec![insert_at + 1, 0], 0)))
                .source("command:core.insert_divider");

                editor
                    .apply(tx)
                    .map_err(|e| CommandError::new(format!("Failed to insert divider: {e:?}")))
            }),
        }]
    }
}
