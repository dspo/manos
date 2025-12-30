use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::{Attrs, Document, ElementNode, Marks, Node, Point, Selection, TextNode};
use crate::ops::{Op, Path, Transaction};

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

#[derive(Debug, Clone)]
pub struct QueryError {
    message: String,
}

impl QueryError {
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
    pub description: Option<String>,
    pub keywords: Vec<String>,
    pub args_example: Option<serde_json::Value>,
    pub hidden: bool,
    pub handler: std::sync::Arc<
        dyn Fn(&mut crate::core::Editor, Option<serde_json::Value>) -> Result<(), CommandError>
            + Send
            + Sync,
    >,
}

impl CommandSpec {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        handler: impl Fn(
            &mut crate::core::Editor,
            Option<serde_json::Value>,
        ) -> Result<(), CommandError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
            keywords: Vec::new(),
            args_example: None,
            hidden: false,
            handler: std::sync::Arc::new(handler),
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn keywords<I, S>(mut self, keywords: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.keywords = keywords.into_iter().map(Into::into).collect();
        self
    }

    pub fn args_example(mut self, args_example: serde_json::Value) -> Self {
        self.args_example = Some(args_example);
        self
    }

    pub fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }
}

#[derive(Clone)]
pub struct QuerySpec {
    pub id: String,
    pub handler: std::sync::Arc<
        dyn Fn(
                &crate::core::Editor,
                Option<serde_json::Value>,
            ) -> Result<serde_json::Value, QueryError>
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

#[derive(Debug, Clone)]
pub struct TransactionPreview {
    pub doc: Document,
    pub selection: Selection,
}

pub trait TransactionTransform: Send + Sync {
    fn id(&self) -> &'static str;
    fn transform(&self, editor: &crate::core::Editor, tx: &Transaction) -> Option<Transaction>;
}

pub trait PlatePlugin: Send + Sync {
    fn id(&self) -> &'static str;
    fn node_specs(&self) -> Vec<NodeSpec> {
        Vec::new()
    }
    fn transaction_transforms(&self) -> Vec<Box<dyn TransactionTransform>> {
        Vec::new()
    }
    fn normalize_passes(&self) -> Vec<Box<dyn NormalizePass>> {
        Vec::new()
    }
    fn commands(&self) -> Vec<CommandSpec> {
        Vec::new()
    }
    fn queries(&self) -> Vec<QuerySpec> {
        Vec::new()
    }
}

#[derive(Default)]
pub struct PluginRegistry {
    node_specs: HashMap<String, NodeSpec>,
    transaction_transforms: Vec<Box<dyn TransactionTransform>>,
    normalize_passes: Vec<Box<dyn NormalizePass>>,
    commands: HashMap<String, CommandSpec>,
    queries: HashMap<String, QuerySpec>,
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

    pub fn richtext() -> Self {
        let plugins: Vec<Box<dyn PlatePlugin>> = vec![
            Box::new(CoreParagraphPlugin),
            Box::new(CoreDividerPlugin),
            Box::new(CoreNormalizePlugin),
            Box::new(CoreCommandsPlugin),
            Box::new(MarksCommandsPlugin),
            Box::new(AutoformatPlugin),
            Box::new(HeadingPlugin),
            Box::new(CodeBlockPlugin),
            Box::new(BlockquotePlugin),
            Box::new(TogglePlugin),
            Box::new(ColumnsPlugin),
            Box::new(TodoPlugin),
            Box::new(IndentPlugin),
            Box::new(AlignPlugin),
            Box::new(FontSizePlugin),
            Box::new(ListPlugin),
            Box::new(TablePlugin),
            Box::new(MentionPlugin),
            Box::new(EmojiPlugin),
            Box::new(ImagePlugin),
        ];
        Self::new(plugins).expect("richtext registry must be valid")
    }

    pub fn register_plugin(&mut self, plugin: Box<dyn PlatePlugin>) -> Result<(), String> {
        for spec in plugin.node_specs() {
            if self.node_specs.contains_key(&spec.kind) {
                return Err(format!("Duplicate node spec kind: {}", spec.kind));
            }
            self.node_specs.insert(spec.kind.clone(), spec);
        }

        self.transaction_transforms
            .extend(plugin.transaction_transforms());

        self.normalize_passes.extend(plugin.normalize_passes());

        for cmd in plugin.commands() {
            if self.commands.contains_key(&cmd.id) {
                return Err(format!("Duplicate command id: {}", cmd.id));
            }
            self.commands.insert(cmd.id.clone(), cmd);
        }

        for query in plugin.queries() {
            if self.queries.contains_key(&query.id) {
                return Err(format!("Duplicate query id: {}", query.id));
            }
            self.queries.insert(query.id.clone(), query);
        }

        Ok(())
    }

    pub fn node_specs(&self) -> &HashMap<String, NodeSpec> {
        &self.node_specs
    }

    pub fn transaction_transforms(&self) -> &[Box<dyn TransactionTransform>] {
        &self.transaction_transforms
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

    pub fn queries(&self) -> &HashMap<String, QuerySpec> {
        &self.queries
    }

    pub fn query(&self, id: &str) -> Option<QuerySpec> {
        self.queries.get(id).cloned()
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
            path: vec![0],
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
    fn walk(children: &[Node], path: &mut Vec<usize>) -> Option<Point> {
        for (ix, node) in children.iter().enumerate() {
            path.push(ix);
            match node {
                Node::Text(t) => {
                    let point = Point {
                        path: path.clone(),
                        offset: 0.min(t.text.len()),
                    };
                    path.pop();
                    return Some(point);
                }
                Node::Element(el) => {
                    if let Some(point) = walk(&el.children, path) {
                        path.pop();
                        return Some(point);
                    }
                }
                Node::Void(_) => {}
            }
            path.pop();
        }
        None
    }

    walk(&doc.children, &mut Vec::new())
}

fn normalize_point_to_existing_text(doc: &Document, point: &Point) -> Option<Point> {
    if point.path.is_empty() || doc.children.is_empty() {
        return None;
    }

    fn first_text_descendant(children: &[Node], path: &mut Vec<usize>) -> Option<Point> {
        for (ix, node) in children.iter().enumerate() {
            path.push(ix);
            match node {
                Node::Text(_) => {
                    let point = Point {
                        path: path.clone(),
                        offset: 0,
                    };
                    path.pop();
                    return Some(point);
                }
                Node::Element(el) => {
                    if let Some(point) = first_text_descendant(&el.children, path) {
                        path.pop();
                        return Some(point);
                    }
                }
                Node::Void(_) => {}
            }
            path.pop();
        }
        None
    }

    let mut resolved_path: Vec<usize> = Vec::new();
    let mut children: &[Node] = &doc.children;

    for &wanted in &point.path {
        if children.is_empty() {
            break;
        }
        let ix = wanted.min(children.len() - 1);
        resolved_path.push(ix);
        let node = &children[ix];
        match node {
            Node::Text(t) => {
                return Some(Point {
                    path: resolved_path,
                    offset: point.offset.min(t.text.len()),
                });
            }
            Node::Element(el) => {
                children = &el.children;
            }
            Node::Void(_) => {
                break;
            }
        }
    }

    let node = node_at_path(doc, &resolved_path)?;
    match node {
        Node::Text(t) => Some(Point {
            path: resolved_path,
            offset: point.offset.min(t.text.len()),
        }),
        Node::Element(el) => first_text_descendant(&el.children, &mut resolved_path),
        Node::Void(_) => None,
    }
}

fn node_at_path<'a>(doc: &'a Document, path: &[usize]) -> Option<&'a Node> {
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
            Box::new(MergeAdjacentTextLeaves),
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
        "core.ensure_inline_only_blocks_have_text_leaf"
    }

    fn run(&self, doc: &Document, registry: &PluginRegistry) -> Vec<Op> {
        let mut ops = Vec::new();

        fn walk(
            children: &[Node],
            path: &mut Vec<usize>,
            registry: &PluginRegistry,
            ops: &mut Vec<Op>,
        ) {
            for (ix, node) in children.iter().enumerate() {
                let Node::Element(el) = node else {
                    continue;
                };

                path.push(ix);

                let spec_children = registry
                    .node_specs
                    .get(&el.kind)
                    .map(|s| s.children.clone())
                    .unwrap_or(ChildConstraint::Any);

                if spec_children == ChildConstraint::InlineOnly {
                    let has_text = el.children.iter().any(|n| matches!(n, Node::Text(_)));
                    if !has_text {
                        let mut insert_path = path.clone();
                        insert_path.push(0);
                        ops.push(Op::InsertNode {
                            path: insert_path,
                            node: Node::Text(TextNode {
                                text: String::new(),
                                marks: Marks::default(),
                            }),
                        });
                    }
                } else {
                    walk(&el.children, path, registry, ops);
                }

                path.pop();
            }
        }

        walk(&doc.children, &mut Vec::new(), registry, &mut ops);
        ops
    }
}

struct MergeAdjacentTextLeaves;

impl NormalizePass for MergeAdjacentTextLeaves {
    fn id(&self) -> &'static str {
        "core.merge_adjacent_text_leaves"
    }

    fn run(&self, doc: &Document, registry: &PluginRegistry) -> Vec<Op> {
        let mut ops = Vec::new();

        fn walk(
            children: &[Node],
            path: &mut Vec<usize>,
            registry: &PluginRegistry,
            ops: &mut Vec<Op>,
        ) {
            for (ix, node) in children.iter().enumerate() {
                let Node::Element(el) = node else {
                    continue;
                };

                path.push(ix);

                let spec_children = registry
                    .node_specs
                    .get(&el.kind)
                    .map(|s| s.children.clone())
                    .unwrap_or_else(|| {
                        if el.children.iter().any(|n| matches!(n, Node::Text(_))) {
                            ChildConstraint::InlineOnly
                        } else {
                            ChildConstraint::Any
                        }
                    });

                if spec_children == ChildConstraint::InlineOnly {
                    if el.children.len() >= 2 {
                        let mut ix = el.children.len();
                        while ix > 0 {
                            ix -= 1;
                            let Node::Text(right) = &el.children[ix] else {
                                continue;
                            };

                            let mut start = ix;
                            while start > 0 {
                                let Some(Node::Text(left)) = el.children.get(start - 1) else {
                                    break;
                                };
                                if left.marks != right.marks {
                                    break;
                                }
                                start -= 1;
                            }

                            if start == ix {
                                continue;
                            }

                            let Some(Node::Text(first)) = el.children.get(start) else {
                                continue;
                            };
                            let mut appended = String::new();
                            for node in el.children.iter().take(ix + 1).skip(start + 1) {
                                if let Node::Text(t) = node {
                                    appended.push_str(&t.text);
                                }
                            }

                            if !appended.is_empty() {
                                let mut insert_text_path = path.clone();
                                insert_text_path.push(start);
                                ops.push(Op::InsertText {
                                    path: insert_text_path,
                                    offset: first.text.len(),
                                    text: appended,
                                });
                            }

                            for remove_ix in (start + 1..=ix).rev() {
                                let mut remove_path = path.clone();
                                remove_path.push(remove_ix);
                                ops.push(Op::RemoveNode { path: remove_path });
                            }

                            ix = start;
                        }
                    }
                } else {
                    walk(&el.children, path, registry, ops);
                }

                path.pop();
            }
        }

        walk(&doc.children, &mut Vec::new(), registry, &mut ops);

        ops
    }
}

struct CoreCommandsPlugin;

impl PlatePlugin for CoreCommandsPlugin {
    fn id(&self) -> &'static str {
        "core.commands"
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new("core.insert_divider", "Insert divider", |editor, _args| {
                let focus = editor.selection().focus.clone();
                let block_path = focus.path.split_last().map(|(_, p)| p).unwrap_or(&[]);

                let (parent_path, insert_at) = if block_path.is_empty() {
                    (Vec::new(), editor.doc().children.len())
                } else {
                    let (block_ix, parent) = block_path.split_last().unwrap();
                    (parent.to_vec(), block_ix + 1)
                };

                let divider_path = {
                    let mut path = parent_path.clone();
                    path.push(insert_at);
                    path
                };
                let paragraph_element_path = {
                    let mut path = parent_path.clone();
                    path.push(insert_at + 1);
                    path
                };

                let paragraph_path = {
                    let mut path = paragraph_element_path.clone();
                    path.push(0);
                    path
                };

                let tx = Transaction::new(vec![
                    Op::InsertNode {
                        path: divider_path,
                        node: Node::divider(),
                    },
                    Op::InsertNode {
                        path: paragraph_element_path.clone(),
                        node: Node::paragraph(""),
                    },
                ])
                .selection_after(Selection::collapsed(Point::new(paragraph_path, 0)))
                .source("command:core.insert_divider");

                editor
                    .apply(tx)
                    .map_err(|e| CommandError::new(format!("Failed to insert divider: {e:?}")))
            })
            .description("Insert a divider block and a trailing paragraph.")
            .keywords(["divider", "separator", "hr", "horizontal rule"]),
        ]
    }
}

struct MarksCommandsPlugin;

impl PlatePlugin for MarksCommandsPlugin {
    fn id(&self) -> &'static str {
        "marks.commands"
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new("marks.toggle_bold", "Toggle bold", |editor, _args| {
                toggle_bold(editor)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor
                            .apply(tx)
                            .map_err(|e| CommandError::new(format!("Failed to toggle bold: {e:?}")))
                    })
            })
            .description("Toggle bold on the current selection or caret.")
            .keywords(["bold", "strong", "mark"]),
            CommandSpec::new("marks.toggle_italic", "Toggle italic", |editor, _args| {
                toggle_italic(editor)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("Failed to toggle italic: {e:?}"))
                        })
                    })
            })
            .description("Toggle italic on the current selection or caret.")
            .keywords(["italic", "emphasis", "mark"]),
            CommandSpec::new(
                "marks.toggle_underline",
                "Toggle underline",
                |editor, _args| {
                    toggle_underline(editor)
                        .map_err(CommandError::new)
                        .and_then(|tx| {
                            editor.apply(tx).map_err(|e| {
                                CommandError::new(format!("Failed to toggle underline: {e:?}"))
                            })
                        })
                },
            )
            .description("Toggle underline on the current selection or caret.")
            .keywords(["underline", "mark"]),
            CommandSpec::new(
                "marks.toggle_strikethrough",
                "Toggle strikethrough",
                |editor, _args| {
                    toggle_strikethrough(editor)
                        .map_err(CommandError::new)
                        .and_then(|tx| {
                            editor.apply(tx).map_err(|e| {
                                CommandError::new(format!("Failed to toggle strikethrough: {e:?}"))
                            })
                        })
                },
            )
            .description("Toggle strikethrough on the current selection or caret.")
            .keywords(["strikethrough", "strike", "mark"]),
            CommandSpec::new("marks.toggle_code", "Toggle code", |editor, _args| {
                toggle_code(editor)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor
                            .apply(tx)
                            .map_err(|e| CommandError::new(format!("Failed to toggle code: {e:?}")))
                    })
            })
            .description("Toggle code mark on the current selection or caret.")
            .keywords(["code", "monospace", "mark"]),
            CommandSpec::new("marks.set_link", "Set link", |editor, args| {
                let url = args
                    .as_ref()
                    .and_then(|v| v.get("url"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| CommandError::new("Missing args.url"))?
                    .to_string();
                set_link(editor, url)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor
                            .apply(tx)
                            .map_err(|e| CommandError::new(format!("Failed to set link: {e:?}")))
                    })
            })
            .description("Set link mark on the current selection or caret.")
            .keywords(["link", "url", "hyperlink"])
            .args_example(serde_json::json!({ "url": "https://example.com" })),
            CommandSpec::new("marks.unset_link", "Unset link", |editor, _args| {
                unset_link(editor)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor
                            .apply(tx)
                            .map_err(|e| CommandError::new(format!("Failed to unset link: {e:?}")))
                    })
            })
            .description("Remove link mark from the current selection or caret.")
            .keywords(["link", "unlink", "url", "hyperlink"]),
            CommandSpec::new("marks.set_text_color", "Set text color", |editor, args| {
                let color = args
                    .as_ref()
                    .and_then(|v| v.get("color"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| CommandError::new("Missing args.color"))?
                    .to_string();
                set_text_color(editor, color)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("Failed to set text color: {e:?}"))
                        })
                    })
            })
            .description("Set text color mark (expects args.color as a hex string).")
            .keywords(["color", "text color", "foreground", "fg"])
            .args_example(serde_json::json!({ "color": "#ff0000" })),
            CommandSpec::new(
                "marks.unset_text_color",
                "Unset text color",
                |editor, _args| {
                    unset_text_color(editor)
                        .map_err(CommandError::new)
                        .and_then(|tx| {
                            editor.apply(tx).map_err(|e| {
                                CommandError::new(format!("Failed to unset text color: {e:?}"))
                            })
                        })
                },
            )
            .description("Remove text color mark from the current selection or caret.")
            .keywords(["color", "text color", "foreground", "fg", "reset"]),
            CommandSpec::new(
                "marks.set_highlight_color",
                "Set highlight color",
                |editor, args| {
                    let color = args
                        .as_ref()
                        .and_then(|v| v.get("color"))
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| CommandError::new("Missing args.color"))?
                        .to_string();
                    set_highlight_color(editor, color)
                        .map_err(CommandError::new)
                        .and_then(|tx| {
                            editor.apply(tx).map_err(|e| {
                                CommandError::new(format!("Failed to set highlight color: {e:?}"))
                            })
                        })
                },
            )
            .description("Set highlight color mark (expects args.color as a hex string).")
            .keywords(["highlight", "background", "bg", "color"])
            .args_example(serde_json::json!({ "color": "#fff59d" })),
            CommandSpec::new(
                "marks.unset_highlight_color",
                "Unset highlight color",
                |editor, _args| {
                    unset_highlight_color(editor)
                        .map_err(CommandError::new)
                        .and_then(|tx| {
                            editor.apply(tx).map_err(|e| {
                                CommandError::new(format!("Failed to unset highlight color: {e:?}"))
                            })
                        })
                },
            )
            .description("Remove highlight color mark from the current selection or caret.")
            .keywords(["highlight", "background", "bg", "color", "reset"]),
        ]
    }

    fn queries(&self) -> Vec<QuerySpec> {
        vec![
            QuerySpec {
                id: "marks.get_active".to_string(),
                handler: std::sync::Arc::new(|editor, _args| {
                    serde_json::to_value(active_marks(editor))
                        .map_err(|err| QueryError::new(format!("Failed to encode marks: {err}")))
                }),
            },
            QuerySpec {
                id: "marks.is_bold_active".to_string(),
                handler: std::sync::Arc::new(|editor, _args| {
                    Ok(Value::Bool(active_marks(editor).bold))
                }),
            },
            QuerySpec {
                id: "marks.is_italic_active".to_string(),
                handler: std::sync::Arc::new(|editor, _args| {
                    Ok(Value::Bool(active_marks(editor).italic))
                }),
            },
            QuerySpec {
                id: "marks.is_underline_active".to_string(),
                handler: std::sync::Arc::new(|editor, _args| {
                    Ok(Value::Bool(active_marks(editor).underline))
                }),
            },
            QuerySpec {
                id: "marks.is_strikethrough_active".to_string(),
                handler: std::sync::Arc::new(|editor, _args| {
                    Ok(Value::Bool(active_marks(editor).strikethrough))
                }),
            },
            QuerySpec {
                id: "marks.is_code_active".to_string(),
                handler: std::sync::Arc::new(|editor, _args| {
                    Ok(Value::Bool(active_marks(editor).code))
                }),
            },
            QuerySpec {
                id: "marks.has_link_active".to_string(),
                handler: std::sync::Arc::new(|editor, _args| {
                    Ok(Value::Bool(active_marks(editor).link.is_some()))
                }),
            },
        ]
    }
}

struct AutoformatPlugin;

impl PlatePlugin for AutoformatPlugin {
    fn id(&self) -> &'static str {
        "autoformat"
    }

    fn transaction_transforms(&self) -> Vec<Box<dyn TransactionTransform>> {
        vec![Box::new(AutoformatOnSpace)]
    }
}

struct AutoformatOnSpace;

impl TransactionTransform for AutoformatOnSpace {
    fn id(&self) -> &'static str {
        "autoformat.on_space"
    }

    fn transform(&self, editor: &crate::core::Editor, tx: &Transaction) -> Option<Transaction> {
        let source = tx.meta.source.as_deref()?;
        if source != "ime:replace_text" {
            return None;
        }

        fn replace_node_ops(path: &[usize], node: Node) -> Vec<Op> {
            vec![
                Op::RemoveNode {
                    path: path.to_vec(),
                },
                Op::InsertNode {
                    path: path.to_vec(),
                    node,
                },
            ]
        }

        let mut extra_ops: Vec<Op> = Vec::new();
        let mut selection_after: Option<Selection> = None;

        let Some((block_path, caret_global, text, marks)) = (|| {
            let (doc_after, selection_after) = if let Some(sel) = &tx.selection_after {
                (None, sel.clone())
            } else {
                let preview = editor.preview_transaction(tx).ok()?;
                (Some(preview.doc), preview.selection)
            };

            if !selection_after.is_collapsed() {
                return None;
            }

            let focus = &selection_after.focus;
            let (focus_child_ix, block_path) = focus.path.split_last()?;
            let block_path = block_path.to_vec();
            let focus_child_ix = *focus_child_ix;

            let children = if let Some(children) = children_after_replace_ops(tx, &block_path) {
                children
            } else if let Some(doc) = doc_after {
                let Node::Element(el) = node_at_path(&doc, &block_path)? else {
                    return None;
                };
                el.children.clone()
            } else {
                return None;
            };

            let mut out_text = String::new();
            let mut out_marks = Marks::default();
            let mut have_marks = false;
            for child in &children {
                match child {
                    Node::Text(t) => {
                        if !have_marks {
                            out_marks = t.marks.clone();
                            have_marks = true;
                        }
                        out_text.push_str(&t.text);
                    }
                    Node::Void(_) => return None,
                    Node::Element(_) => return None,
                }
            }

            let caret_global = point_global_offset(&children, focus_child_ix, focus.offset);

            Some((block_path, caret_global, out_text, out_marks))
        })() else {
            return None;
        };

        fn children_after_replace_ops(tx: &Transaction, block_path: &[usize]) -> Option<Vec<Node>> {
            let mut saw_remove = false;
            let mut inserted: Vec<(usize, Node)> = Vec::new();

            for op in &tx.ops {
                match op {
                    Op::RemoveNode { path }
                        if path.len() == block_path.len() + 1 && path.starts_with(block_path) =>
                    {
                        saw_remove = true;
                    }
                    Op::InsertNode { path, node }
                        if path.len() == block_path.len() + 1 && path.starts_with(block_path) =>
                    {
                        let child_ix = *path.last()?;
                        inserted.push((child_ix, node.clone()));
                    }
                    _ => {}
                }
            }

            if !saw_remove || inserted.is_empty() {
                return None;
            }

            inserted.sort_by_key(|(ix, _)| *ix);
            for (expected, (ix, _)) in inserted.iter().enumerate() {
                if *ix != expected {
                    return None;
                }
            }

            Some(inserted.into_iter().map(|(_, node)| node).collect())
        }

        let Node::Element(block_el) = node_at_path(editor.doc(), &block_path)? else {
            return None;
        };
        if block_el.kind != "paragraph" {
            return None;
        }
        if !element_is_text_block(block_el, editor.registry()) {
            return None;
        }

        let empty_text_children = || -> Vec<Node> {
            vec![Node::Text(TextNode {
                text: String::new(),
                marks: marks.clone(),
            })]
        };

        match text.as_str() {
            "- " | "* " if caret_global == 2 && text.len() == 2 => {
                let indent = block_el
                    .attrs
                    .get("indent")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let mut attrs = block_el.attrs.clone();
                attrs.remove("indent");
                attrs.remove("list_index");
                attrs.insert(
                    "list_type".to_string(),
                    Value::String("bulleted".to_string()),
                );
                if indent > 0 {
                    attrs.insert(
                        "list_level".to_string(),
                        Value::Number(serde_json::Number::from(indent.min(MAX_INDENT_LEVEL))),
                    );
                }
                let children = empty_text_children();
                let node = Node::Element(ElementNode {
                    kind: "list_item".to_string(),
                    attrs,
                    children: children.clone(),
                });
                extra_ops.extend(replace_node_ops(&block_path, node));
                let caret = point_for_global_offset(&block_path, &children, 0);
                selection_after = Some(Selection::collapsed(caret));
            }
            "1. " if caret_global == 3 && text.len() == 3 => {
                let indent = block_el
                    .attrs
                    .get("indent")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let mut attrs = block_el.attrs.clone();
                attrs.remove("indent");
                attrs.remove("list_index");
                attrs.insert(
                    "list_type".to_string(),
                    Value::String("ordered".to_string()),
                );
                if indent > 0 {
                    attrs.insert(
                        "list_level".to_string(),
                        Value::Number(serde_json::Number::from(indent.min(MAX_INDENT_LEVEL))),
                    );
                }
                let children = empty_text_children();
                let node = Node::Element(ElementNode {
                    kind: "list_item".to_string(),
                    attrs,
                    children: children.clone(),
                });
                extra_ops.extend(replace_node_ops(&block_path, node));
                let caret = point_for_global_offset(&block_path, &children, 0);
                selection_after = Some(Selection::collapsed(caret));
            }
            "> " if caret_global == 2 && text.len() == 2 => {
                let children = empty_text_children();
                let para = Node::Element(ElementNode {
                    kind: "paragraph".to_string(),
                    attrs: block_el.attrs.clone(),
                    children: children.clone(),
                });
                let quote = Node::Element(ElementNode {
                    kind: "blockquote".to_string(),
                    attrs: Attrs::default(),
                    children: vec![para],
                });
                extra_ops.extend(replace_node_ops(&block_path, quote));
                let mut inner_path = block_path.clone();
                inner_path.push(0);
                let caret = point_for_global_offset(&inner_path, &children, 0);
                selection_after = Some(Selection::collapsed(caret));
            }
            "[ ] " | "[x] " | "[X] " if caret_global == 4 && text.len() == 4 => {
                let checked = matches!(text.as_str(), "[x] " | "[X] ");
                let mut attrs = block_el.attrs.clone();
                attrs.insert("checked".to_string(), Value::Bool(checked));
                let children = empty_text_children();
                let node = Node::Element(ElementNode {
                    kind: "todo_item".to_string(),
                    attrs,
                    children: children.clone(),
                });
                extra_ops.extend(replace_node_ops(&block_path, node));
                let caret = point_for_global_offset(&block_path, &children, 0);
                selection_after = Some(Selection::collapsed(caret));
            }
            marker if marker.ends_with(' ') && marker.chars().all(|ch| ch == '#' || ch == ' ') => {
                let hashes = marker.chars().take_while(|ch| *ch == '#').count();
                if hashes >= 1
                    && hashes <= 6
                    && marker == format!("{} ", "#".repeat(hashes))
                    && caret_global == marker.len()
                    && text.len() == marker.len()
                {
                    let mut attrs = block_el.attrs.clone();
                    attrs.insert(
                        "level".to_string(),
                        Value::Number(serde_json::Number::from(hashes as u64)),
                    );
                    let children = empty_text_children();
                    let node = Node::Element(ElementNode {
                        kind: "heading".to_string(),
                        attrs,
                        children: children.clone(),
                    });
                    extra_ops.extend(replace_node_ops(&block_path, node));
                    let caret = point_for_global_offset(&block_path, &children, 0);
                    selection_after = Some(Selection::collapsed(caret));
                }
            }
            _ => {}
        }

        if extra_ops.is_empty() {
            return None;
        }

        let mut ops = tx.ops.clone();
        ops.extend(extra_ops);

        let mut out = Transaction::new(ops);
        out.meta = tx.meta.clone();
        if let Some(sel) = selection_after {
            out.selection_after = Some(sel);
        }
        Some(out)
    }
}

struct HeadingPlugin;

impl PlatePlugin for HeadingPlugin {
    fn id(&self) -> &'static str {
        "heading"
    }

    fn node_specs(&self) -> Vec<NodeSpec> {
        vec![NodeSpec {
            kind: "heading".to_string(),
            role: NodeRole::Block,
            is_void: false,
            children: ChildConstraint::InlineOnly,
        }]
    }

    fn normalize_passes(&self) -> Vec<Box<dyn NormalizePass>> {
        vec![Box::new(NormalizeHeadingLevels)]
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new("block.set_heading", "Set heading", |editor, args| {
                let level = args
                    .as_ref()
                    .and_then(|v| v.get("level"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1)
                    .clamp(1, 6);
                set_heading(editor, level)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        if tx.ops.is_empty() {
                            return Ok(());
                        }
                        editor
                            .apply(tx)
                            .map_err(|e| CommandError::new(format!("Failed to set heading: {e:?}")))
                    })
            })
            .description("Convert the active text block(s) into a heading.")
            .keywords(["heading", "title", "h1", "h2", "h3", "h4", "h5", "h6"])
            .args_example(serde_json::json!({ "level": 2 })),
            CommandSpec::new("block.unset_heading", "Unset heading", |editor, _args| {
                unset_heading(editor)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        if tx.ops.is_empty() {
                            return Ok(());
                        }
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("Failed to unset heading: {e:?}"))
                        })
                    })
            })
            .description("Convert heading back to a paragraph.")
            .keywords(["heading", "paragraph", "title", "reset"]),
        ]
    }

    fn queries(&self) -> Vec<QuerySpec> {
        vec![QuerySpec {
            id: "block.heading_level".to_string(),
            handler: std::sync::Arc::new(|editor, _args| Ok(active_heading_level(editor))),
        }]
    }
}

struct NormalizeHeadingLevels;

impl NormalizePass for NormalizeHeadingLevels {
    fn id(&self) -> &'static str {
        "heading.normalize_levels"
    }

    fn run(&self, doc: &Document, registry: &PluginRegistry) -> Vec<Op> {
        let mut ops = Vec::new();

        fn normalize_container(
            children: &[Node],
            parent_path: &mut Vec<usize>,
            registry: &PluginRegistry,
            ops: &mut Vec<Op>,
        ) {
            for (ix, node) in children.iter().enumerate() {
                let Node::Element(el) = node else {
                    continue;
                };

                if el.kind == "heading" {
                    let level = el
                        .attrs
                        .get("level")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(1)
                        .clamp(1, 6);
                    let current = el.attrs.get("level").and_then(|v| v.as_u64());
                    if current != Some(level) {
                        let mut set = Attrs::default();
                        set.insert(
                            "level".to_string(),
                            Value::Number(serde_json::Number::from(level)),
                        );
                        let mut path = parent_path.clone();
                        path.push(ix);
                        ops.push(Op::SetNodeAttrs {
                            path,
                            patch: crate::core::AttrPatch {
                                set,
                                remove: Vec::new(),
                            },
                        });
                    }
                }
            }

            for (ix, node) in children.iter().enumerate() {
                let Node::Element(el) = node else {
                    continue;
                };

                let spec_children = registry
                    .node_specs
                    .get(&el.kind)
                    .map(|s| s.children.clone())
                    .unwrap_or(ChildConstraint::Any);
                if spec_children == ChildConstraint::InlineOnly || el.children.is_empty() {
                    continue;
                }

                parent_path.push(ix);
                normalize_container(&el.children, parent_path, registry, ops);
                parent_path.pop();
            }
        }

        normalize_container(&doc.children, &mut Vec::new(), registry, &mut ops);

        ops
    }
}

struct CodeBlockPlugin;

impl PlatePlugin for CodeBlockPlugin {
    fn id(&self) -> &'static str {
        "code_block"
    }

    fn node_specs(&self) -> Vec<NodeSpec> {
        vec![NodeSpec {
            kind: "code_block".to_string(),
            role: NodeRole::Block,
            is_void: false,
            children: ChildConstraint::InlineOnly,
        }]
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new("code_block.toggle", "Toggle code block", |editor, _args| {
                toggle_code_block(editor)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        if tx.ops.is_empty() {
                            return Ok(());
                        }
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("Failed to toggle code block: {e:?}"))
                        })
                    })
            })
            .description("Toggle code block for the active text block(s).")
            .keywords(["code block", "code", "pre", "monospace"]),
        ]
    }

    fn queries(&self) -> Vec<QuerySpec> {
        vec![QuerySpec {
            id: "code_block.is_active".to_string(),
            handler: std::sync::Arc::new(|editor, _args| Ok(active_is_code_block(editor))),
        }]
    }
}

struct BlockquotePlugin;

impl PlatePlugin for BlockquotePlugin {
    fn id(&self) -> &'static str {
        "blockquote"
    }

    fn node_specs(&self) -> Vec<NodeSpec> {
        vec![NodeSpec {
            kind: "blockquote".to_string(),
            role: NodeRole::Block,
            is_void: false,
            children: ChildConstraint::BlockOnly,
        }]
    }

    fn normalize_passes(&self) -> Vec<Box<dyn NormalizePass>> {
        vec![Box::new(NormalizeBlockquoteChildren)]
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new(
                "blockquote.wrap_selection",
                "Wrap selection in blockquote",
                |editor, _args| {
                    wrap_selection_in_blockquote(editor)
                        .map_err(CommandError::new)
                        .and_then(|tx| {
                            if tx.ops.is_empty() {
                                return Ok(());
                            }
                            editor.apply(tx).map_err(|e| {
                                CommandError::new(format!("Failed to wrap blockquote: {e:?}"))
                            })
                        })
                },
            )
            .description("Wrap selected blocks in a blockquote container.")
            .keywords(["blockquote", "quote", "wrap"]),
            CommandSpec::new("blockquote.unwrap", "Unwrap blockquote", |editor, _args| {
                unwrap_nearest_blockquote(editor)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        if tx.ops.is_empty() {
                            return Ok(());
                        }
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("Failed to unwrap blockquote: {e:?}"))
                        })
                    })
            })
            .description("Unwrap the nearest blockquote container.")
            .keywords(["blockquote", "quote", "unwrap"]),
        ]
    }

    fn queries(&self) -> Vec<QuerySpec> {
        vec![QuerySpec {
            id: "blockquote.is_active".to_string(),
            handler: std::sync::Arc::new(|editor, _args| Ok(Value::Bool(is_in_blockquote(editor)))),
        }]
    }
}

struct NormalizeBlockquoteChildren;

impl NormalizePass for NormalizeBlockquoteChildren {
    fn id(&self) -> &'static str {
        "blockquote.ensure_non_empty"
    }

    fn run(&self, doc: &Document, registry: &PluginRegistry) -> Vec<Op> {
        let mut ops = Vec::new();

        fn normalize_container(
            children: &[Node],
            parent_path: &mut Vec<usize>,
            registry: &PluginRegistry,
            ops: &mut Vec<Op>,
        ) {
            for (ix, node) in children.iter().enumerate() {
                let Node::Element(el) = node else {
                    continue;
                };

                if el.kind == "blockquote" && el.children.is_empty() {
                    let mut path = parent_path.clone();
                    path.push(ix);
                    path.push(0);
                    ops.push(Op::InsertNode {
                        path,
                        node: Node::paragraph(""),
                    });
                }
            }

            for (ix, node) in children.iter().enumerate() {
                let Node::Element(el) = node else {
                    continue;
                };

                let spec_children = registry
                    .node_specs
                    .get(&el.kind)
                    .map(|s| s.children.clone())
                    .unwrap_or(ChildConstraint::Any);
                if spec_children == ChildConstraint::InlineOnly || el.children.is_empty() {
                    continue;
                }

                parent_path.push(ix);
                normalize_container(&el.children, parent_path, registry, ops);
                parent_path.pop();
            }
        }

        normalize_container(&doc.children, &mut Vec::new(), registry, &mut ops);
        ops
    }
}

struct TogglePlugin;

impl PlatePlugin for TogglePlugin {
    fn id(&self) -> &'static str {
        "toggle"
    }

    fn node_specs(&self) -> Vec<NodeSpec> {
        vec![NodeSpec {
            kind: "toggle".to_string(),
            role: NodeRole::Block,
            is_void: false,
            children: ChildConstraint::BlockOnly,
        }]
    }

    fn normalize_passes(&self) -> Vec<Box<dyn NormalizePass>> {
        vec![Box::new(NormalizeToggleStructure)]
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new(
                "toggle.wrap_selection",
                "Wrap selection in toggle",
                |editor, _args| {
                    wrap_selection_in_toggle(editor)
                        .map_err(CommandError::new)
                        .and_then(|tx| {
                            if tx.ops.is_empty() {
                                return Ok(());
                            }
                            editor.apply(tx).map_err(|e| {
                                CommandError::new(format!("Failed to wrap toggle: {e:?}"))
                            })
                        })
                },
            )
            .description("Wrap selected blocks in a collapsible toggle container.")
            .keywords(["toggle", "collapsible", "wrap"]),
            CommandSpec::new("toggle.unwrap", "Unwrap toggle", |editor, _args| {
                unwrap_nearest_toggle(editor)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        if tx.ops.is_empty() {
                            return Ok(());
                        }
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("Failed to unwrap toggle: {e:?}"))
                        })
                    })
            })
            .description("Unwrap the nearest toggle container.")
            .keywords(["toggle", "collapsible", "unwrap"]),
            CommandSpec::new(
                "toggle.toggle_collapsed",
                "Toggle collapsed",
                |editor, args| {
                    toggle_toggle_collapsed(editor, args.as_ref())
                        .map_err(CommandError::new)
                        .and_then(|tx| {
                            if tx.ops.is_empty() {
                                return Ok(());
                            }
                            editor.apply(tx).map_err(|e| {
                                CommandError::new(format!("Failed to toggle collapsed: {e:?}"))
                            })
                        })
                },
            )
            .description("Toggle the collapsed state of the nearest toggle container.")
            .keywords(["toggle", "collapse", "expand"]),
        ]
    }

    fn queries(&self) -> Vec<QuerySpec> {
        vec![
            QuerySpec {
                id: "toggle.is_active".to_string(),
                handler: std::sync::Arc::new(|editor, _args| Ok(Value::Bool(is_in_toggle(editor)))),
            },
            QuerySpec {
                id: "toggle.is_collapsed".to_string(),
                handler: std::sync::Arc::new(|editor, _args| {
                    Ok(Value::Bool(active_toggle_collapsed(editor)))
                }),
            },
        ]
    }
}

struct NormalizeToggleStructure;

impl NormalizePass for NormalizeToggleStructure {
    fn id(&self) -> &'static str {
        "toggle.normalize_structure"
    }

    fn run(&self, doc: &Document, registry: &PluginRegistry) -> Vec<Op> {
        let mut ops = Vec::new();

        fn normalize_container(
            children: &[Node],
            parent_path: &mut Vec<usize>,
            registry: &PluginRegistry,
            ops: &mut Vec<Op>,
        ) {
            for (ix, node) in children.iter().enumerate() {
                let Node::Element(el) = node else {
                    continue;
                };

                if el.kind == "toggle" {
                    let mut toggle_path = parent_path.clone();
                    toggle_path.push(ix);

                    if el
                        .attrs
                        .get("collapsed")
                        .and_then(|v| v.as_bool())
                        .is_none()
                    {
                        let mut set = Attrs::default();
                        set.insert("collapsed".to_string(), Value::Bool(false));
                        ops.push(Op::SetNodeAttrs {
                            path: toggle_path.clone(),
                            patch: crate::core::AttrPatch {
                                set,
                                remove: Vec::new(),
                            },
                        });
                    }

                    if el.children.is_empty() {
                        let mut insert_path = toggle_path.clone();
                        insert_path.push(0);
                        ops.push(Op::InsertNode {
                            path: insert_path,
                            node: Node::paragraph(""),
                        });
                    } else {
                        let title_ok = el
                            .children
                            .first()
                            .and_then(|n| match n {
                                Node::Element(el) => Some(el),
                                Node::Text(_) | Node::Void(_) => None,
                            })
                            .is_some_and(|el| matches!(el.kind.as_str(), "paragraph" | "heading"));
                        if !title_ok {
                            let mut insert_path = toggle_path.clone();
                            insert_path.push(0);
                            ops.push(Op::InsertNode {
                                path: insert_path,
                                node: Node::paragraph(""),
                            });
                        }
                    }
                }
            }

            for (ix, node) in children.iter().enumerate() {
                let Node::Element(el) = node else {
                    continue;
                };

                let spec_children = registry
                    .node_specs
                    .get(&el.kind)
                    .map(|s| s.children.clone())
                    .unwrap_or(ChildConstraint::Any);
                if spec_children == ChildConstraint::InlineOnly || el.children.is_empty() {
                    continue;
                }

                parent_path.push(ix);
                normalize_container(&el.children, parent_path, registry, ops);
                parent_path.pop();
            }
        }

        normalize_container(&doc.children, &mut Vec::new(), registry, &mut ops);
        ops
    }
}

struct ColumnsPlugin;

impl PlatePlugin for ColumnsPlugin {
    fn id(&self) -> &'static str {
        "columns"
    }

    fn node_specs(&self) -> Vec<NodeSpec> {
        vec![
            NodeSpec {
                kind: "columns".to_string(),
                role: NodeRole::Block,
                is_void: false,
                children: ChildConstraint::BlockOnly,
            },
            NodeSpec {
                kind: "column".to_string(),
                role: NodeRole::Block,
                is_void: false,
                children: ChildConstraint::BlockOnly,
            },
        ]
    }

    fn normalize_passes(&self) -> Vec<Box<dyn NormalizePass>> {
        vec![Box::new(NormalizeColumnsStructure)]
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new("columns.insert", "Insert columns", |editor, args| {
                let cols = args
                    .as_ref()
                    .and_then(|v| v.get("columns"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(2)
                    .clamp(2, 6) as usize;

                insert_columns(editor, cols)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("Failed to insert columns: {e:?}"))
                        })
                    })
            })
            .description("Insert a multi-column layout container.")
            .keywords(["columns", "layout", "grid", "multi-column"])
            .args_example(serde_json::json!({ "columns": 2 })),
            CommandSpec::new("columns.unwrap", "Unwrap columns", |editor, _args| {
                unwrap_nearest_columns(editor)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        if tx.ops.is_empty() {
                            return Ok(());
                        }
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("Failed to unwrap columns: {e:?}"))
                        })
                    })
            })
            .description("Unwrap the nearest columns container.")
            .keywords(["columns", "layout", "unwrap"]),
            CommandSpec::new(
                "columns.set_widths",
                "Set columns widths",
                |editor, args| {
                    let args = args
                        .as_ref()
                        .ok_or_else(|| CommandError::new("Missing args"))?;

                    let path = args
                        .get("path")
                        .and_then(|v| v.as_array())
                        .ok_or_else(|| CommandError::new("Missing args.path"))?
                        .iter()
                        .filter_map(|v| v.as_u64().map(|n| n as usize))
                        .collect::<Vec<usize>>();
                    if path.is_empty() {
                        return Err(CommandError::new("Invalid args.path"));
                    }

                    let widths = args
                        .get("widths")
                        .and_then(|v| v.as_array())
                        .ok_or_else(|| CommandError::new("Missing args.widths"))?
                        .iter()
                        .filter_map(|v| v.as_f64())
                        .collect::<Vec<f64>>();
                    if widths.len() < 2 {
                        return Err(CommandError::new("Invalid args.widths"));
                    }

                    set_columns_widths(editor, &path, widths)
                        .map_err(CommandError::new)
                        .and_then(|tx| {
                            if tx.ops.is_empty() {
                                return Ok(());
                            }
                            editor.apply(tx).map_err(|e| {
                                CommandError::new(format!("Failed to set columns widths: {e:?}"))
                            })
                        })
                },
            )
            .hidden(true),
        ]
    }

    fn queries(&self) -> Vec<QuerySpec> {
        vec![QuerySpec {
            id: "columns.is_active".to_string(),
            handler: std::sync::Arc::new(|editor, _args| {
                let is_active =
                    ancestor_element_path(editor.doc(), &editor.selection().focus.path, "columns")
                        .is_some();
                Ok(Value::Bool(is_active))
            }),
        }]
    }
}

struct NormalizeColumnsStructure;

impl NormalizePass for NormalizeColumnsStructure {
    fn id(&self) -> &'static str {
        "columns.normalize_structure"
    }

    fn run(&self, doc: &Document, registry: &PluginRegistry) -> Vec<Op> {
        let mut ops = Vec::new();

        fn normalize_container(
            children: &[Node],
            parent_path: &mut Vec<usize>,
            registry: &PluginRegistry,
            ops: &mut Vec<Op>,
        ) {
            for (ix, node) in children.iter().enumerate() {
                let Node::Element(el) = node else {
                    continue;
                };

                if el.kind == "columns" {
                    let mut columns_path = parent_path.clone();
                    columns_path.push(ix);

                    let target_cols = el.children.len().max(2);

                    // Ensure the wrapper structure: columns -> column -> blocks.
                    for (child_ix, child) in el.children.iter().enumerate() {
                        let is_column = matches!(child, Node::Element(col) if col.kind == "column");
                        if is_column {
                            continue;
                        }

                        let mut child_path = columns_path.clone();
                        child_path.push(child_ix);

                        ops.push(Op::RemoveNode {
                            path: child_path.clone(),
                        });
                        ops.push(Op::InsertNode {
                            path: child_path,
                            node: Node::Element(ElementNode {
                                kind: "column".to_string(),
                                attrs: Attrs::default(),
                                children: vec![child.clone()],
                            }),
                        });
                    }

                    if el.children.len() < 2 {
                        for insert_ix in el.children.len()..2 {
                            let mut insert_path = columns_path.clone();
                            insert_path.push(insert_ix);
                            ops.push(Op::InsertNode {
                                path: insert_path,
                                node: column_node(vec![Node::paragraph("")]),
                            });
                        }
                    }

                    for (col_ix, col_node) in el.children.iter().enumerate() {
                        let Node::Element(col_el) = col_node else {
                            continue;
                        };
                        if col_el.kind != "column" {
                            continue;
                        }
                        if col_el.children.is_empty() {
                            let mut insert_path = columns_path.clone();
                            insert_path.push(col_ix);
                            insert_path.push(0);
                            ops.push(Op::InsertNode {
                                path: insert_path,
                                node: Node::paragraph(""),
                            });
                        }
                    }

                    if !columns_widths_valid(el, target_cols) {
                        let widths = equal_widths(target_cols);
                        let mut set = Attrs::default();
                        set.insert(
                            "widths".to_string(),
                            Value::Array(
                                widths
                                    .into_iter()
                                    .map(|w| {
                                        Value::Number(
                                            serde_json::Number::from_f64(w)
                                                .unwrap_or_else(|| serde_json::Number::from(0)),
                                        )
                                    })
                                    .collect(),
                            ),
                        );
                        ops.push(Op::SetNodeAttrs {
                            path: columns_path,
                            patch: crate::core::AttrPatch {
                                set,
                                remove: Vec::new(),
                            },
                        });
                    }
                }
            }

            for (ix, node) in children.iter().enumerate() {
                let Node::Element(el) = node else {
                    continue;
                };

                let spec_children = registry
                    .node_specs
                    .get(&el.kind)
                    .map(|s| s.children.clone())
                    .unwrap_or(ChildConstraint::Any);
                if spec_children == ChildConstraint::InlineOnly || el.children.is_empty() {
                    continue;
                }

                parent_path.push(ix);
                normalize_container(&el.children, parent_path, registry, ops);
                parent_path.pop();
            }
        }

        normalize_container(&doc.children, &mut Vec::new(), registry, &mut ops);
        ops
    }
}

struct TodoPlugin;

impl PlatePlugin for TodoPlugin {
    fn id(&self) -> &'static str {
        "todo"
    }

    fn node_specs(&self) -> Vec<NodeSpec> {
        vec![NodeSpec {
            kind: "todo_item".to_string(),
            role: NodeRole::Block,
            is_void: false,
            children: ChildConstraint::InlineOnly,
        }]
    }

    fn normalize_passes(&self) -> Vec<Box<dyn NormalizePass>> {
        vec![Box::new(NormalizeTodoCheckedAttr)]
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new("todo.toggle", "Toggle todo item", |editor, _args| {
                toggle_todo(editor)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        if tx.ops.is_empty() {
                            return Ok(());
                        }
                        editor
                            .apply(tx)
                            .map_err(|e| CommandError::new(format!("Failed to toggle todo: {e:?}")))
                    })
            })
            .description("Toggle todo item block type for the active text block(s).")
            .keywords(["todo", "task", "checkbox"]),
            CommandSpec::new(
                "todo.toggle_checked",
                "Toggle todo checked",
                |editor, args| {
                    toggle_todo_checked(editor, args.as_ref())
                        .map_err(CommandError::new)
                        .and_then(|tx| {
                            if tx.ops.is_empty() {
                                return Ok(());
                            }
                            editor.apply(tx).map_err(|e| {
                                CommandError::new(format!("Failed to toggle todo checked: {e:?}"))
                            })
                        })
                },
            )
            .description("Toggle checked state for the nearest todo item.")
            .keywords(["todo", "task", "checkbox", "checked"]),
        ]
    }

    fn queries(&self) -> Vec<QuerySpec> {
        vec![
            QuerySpec {
                id: "todo.is_active".to_string(),
                handler: std::sync::Arc::new(|editor, _args| Ok(Value::Bool(is_in_todo(editor)))),
            },
            QuerySpec {
                id: "todo.is_checked".to_string(),
                handler: std::sync::Arc::new(|editor, _args| {
                    Ok(Value::Bool(active_todo_checked(editor)))
                }),
            },
        ]
    }
}

struct NormalizeTodoCheckedAttr;

impl NormalizePass for NormalizeTodoCheckedAttr {
    fn id(&self) -> &'static str {
        "todo.normalize_checked_attr"
    }

    fn run(&self, doc: &Document, registry: &PluginRegistry) -> Vec<Op> {
        let mut ops = Vec::new();

        fn normalize_container(
            children: &[Node],
            parent_path: &mut Vec<usize>,
            registry: &PluginRegistry,
            ops: &mut Vec<Op>,
        ) {
            for (ix, node) in children.iter().enumerate() {
                let Node::Element(el) = node else {
                    continue;
                };

                if el.kind == "todo_item" {
                    let checked = el.attrs.get("checked").and_then(|v| v.as_bool());
                    if checked.is_none() {
                        let mut set = Attrs::default();
                        set.insert("checked".to_string(), Value::Bool(false));
                        let mut path = parent_path.clone();
                        path.push(ix);
                        ops.push(Op::SetNodeAttrs {
                            path,
                            patch: crate::core::AttrPatch {
                                set,
                                remove: Vec::new(),
                            },
                        });
                    }
                }
            }

            for (ix, node) in children.iter().enumerate() {
                let Node::Element(el) = node else {
                    continue;
                };

                let spec_children = registry
                    .node_specs
                    .get(&el.kind)
                    .map(|s| s.children.clone())
                    .unwrap_or(ChildConstraint::Any);
                if spec_children == ChildConstraint::InlineOnly || el.children.is_empty() {
                    continue;
                }

                parent_path.push(ix);
                normalize_container(&el.children, parent_path, registry, ops);
                parent_path.pop();
            }
        }

        normalize_container(&doc.children, &mut Vec::new(), registry, &mut ops);
        ops
    }
}

const MAX_INDENT_LEVEL: u64 = 8;
const MIN_FONT_SIZE: u64 = 8;
const MAX_FONT_SIZE: u64 = 72;

struct IndentPlugin;

impl PlatePlugin for IndentPlugin {
    fn id(&self) -> &'static str {
        "indent"
    }

    fn normalize_passes(&self) -> Vec<Box<dyn NormalizePass>> {
        vec![Box::new(NormalizeIndentAttrs)]
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new(
                "block.indent_increase",
                "Increase indent",
                |editor, _args| {
                    adjust_indent(editor, IndentDirection::Increase)
                        .map_err(CommandError::new)
                        .and_then(|tx| {
                            if tx.ops.is_empty() {
                                return Ok(());
                            }
                            editor.apply(tx).map_err(|e| {
                                CommandError::new(format!("Failed to increase indent: {e:?}"))
                            })
                        })
                },
            )
            .description("Increase indent level for the active block(s).")
            .keywords(["indent", "tab", "increase"]),
            CommandSpec::new(
                "block.indent_decrease",
                "Decrease indent",
                |editor, _args| {
                    adjust_indent(editor, IndentDirection::Decrease)
                        .map_err(CommandError::new)
                        .and_then(|tx| {
                            if tx.ops.is_empty() {
                                return Ok(());
                            }
                            editor.apply(tx).map_err(|e| {
                                CommandError::new(format!("Failed to decrease indent: {e:?}"))
                            })
                        })
                },
            )
            .description("Decrease indent level for the active block(s).")
            .keywords(["indent", "outdent", "decrease"]),
        ]
    }

    fn queries(&self) -> Vec<QuerySpec> {
        vec![QuerySpec {
            id: "block.indent_level".to_string(),
            handler: std::sync::Arc::new(|editor, _args| Ok(active_indent_level(editor))),
        }]
    }
}

struct NormalizeIndentAttrs;

impl NormalizePass for NormalizeIndentAttrs {
    fn id(&self) -> &'static str {
        "block.normalize_indent_attrs"
    }

    fn run(&self, doc: &Document, registry: &PluginRegistry) -> Vec<Op> {
        let mut ops = Vec::new();

        fn normalize_level_attr(
            el: &ElementNode,
            path: &[usize],
            key: &str,
            max: u64,
            ops: &mut Vec<Op>,
        ) {
            let Some(value) = el.attrs.get(key) else {
                return;
            };

            let Some(level) = value.as_u64() else {
                ops.push(Op::SetNodeAttrs {
                    path: path.to_vec(),
                    patch: crate::core::AttrPatch {
                        set: Attrs::default(),
                        remove: vec![key.to_string()],
                    },
                });
                return;
            };

            let clamped = level.min(max);
            if clamped == 0 {
                ops.push(Op::SetNodeAttrs {
                    path: path.to_vec(),
                    patch: crate::core::AttrPatch {
                        set: Attrs::default(),
                        remove: vec![key.to_string()],
                    },
                });
                return;
            }

            if clamped != level {
                let mut set = Attrs::default();
                set.insert(key.to_string(), Value::Number(clamped.into()));
                ops.push(Op::SetNodeAttrs {
                    path: path.to_vec(),
                    patch: crate::core::AttrPatch {
                        set,
                        remove: Vec::new(),
                    },
                });
            }
        }

        fn walk(
            nodes: &[Node],
            path: &mut Vec<usize>,
            registry: &PluginRegistry,
            ops: &mut Vec<Op>,
        ) {
            for (ix, node) in nodes.iter().enumerate() {
                let Node::Element(el) = node else {
                    continue;
                };

                path.push(ix);

                if element_is_text_block(el, registry) {
                    normalize_level_attr(el, path, "indent", MAX_INDENT_LEVEL, ops);
                    if el.kind == "list_item" {
                        normalize_level_attr(el, path, "list_level", MAX_INDENT_LEVEL, ops);
                    }
                } else {
                    walk(&el.children, path, registry, ops);
                }

                path.pop();
            }
        }

        walk(&doc.children, &mut Vec::new(), registry, &mut ops);
        ops
    }
}

struct AlignPlugin;

impl PlatePlugin for AlignPlugin {
    fn id(&self) -> &'static str {
        "align"
    }

    fn normalize_passes(&self) -> Vec<Box<dyn NormalizePass>> {
        vec![Box::new(NormalizeAlignAttrs)]
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new("block.set_align", "Set block alignment", |editor, args| {
                let align = args
                    .as_ref()
                    .and_then(|v| v.get("align"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| CommandError::new("Missing args.align"))?
                    .to_string();

                set_block_align(editor, align)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        if tx.ops.is_empty() {
                            return Ok(());
                        }
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("Failed to set alignment: {e:?}"))
                        })
                    })
            })
            .description("Set text alignment for the active block(s).")
            .keywords(["align", "alignment", "left", "center", "right"])
            .args_example(serde_json::json!({ "align": "center" })),
        ]
    }

    fn queries(&self) -> Vec<QuerySpec> {
        vec![QuerySpec {
            id: "block.align".to_string(),
            handler: std::sync::Arc::new(|editor, _args| Ok(active_block_align(editor))),
        }]
    }
}

struct NormalizeAlignAttrs;

impl NormalizePass for NormalizeAlignAttrs {
    fn id(&self) -> &'static str {
        "block.normalize_align_attrs"
    }

    fn run(&self, doc: &Document, registry: &PluginRegistry) -> Vec<Op> {
        let mut ops = Vec::new();

        fn normalize_align(
            el: &ElementNode,
            path: &[usize],
            registry: &PluginRegistry,
            ops: &mut Vec<Op>,
        ) {
            let Some(value) = el.attrs.get("align") else {
                return;
            };

            let should_remove = match value.as_str() {
                Some("center") | Some("right") => !element_is_text_block(el, registry),
                Some("left") => true,
                _ => true,
            };

            if should_remove {
                ops.push(Op::SetNodeAttrs {
                    path: path.to_vec(),
                    patch: crate::core::AttrPatch {
                        set: Attrs::default(),
                        remove: vec!["align".to_string()],
                    },
                });
            }
        }

        fn walk(
            nodes: &[Node],
            path: &mut Vec<usize>,
            registry: &PluginRegistry,
            ops: &mut Vec<Op>,
        ) {
            for (ix, node) in nodes.iter().enumerate() {
                let Node::Element(el) = node else {
                    continue;
                };

                path.push(ix);
                normalize_align(el, path, registry, ops);

                if !element_is_text_block(el, registry) {
                    walk(&el.children, path, registry, ops);
                }

                path.pop();
            }
        }

        walk(&doc.children, &mut Vec::new(), registry, &mut ops);
        ops
    }
}

struct FontSizePlugin;

impl PlatePlugin for FontSizePlugin {
    fn id(&self) -> &'static str {
        "font_size"
    }

    fn normalize_passes(&self) -> Vec<Box<dyn NormalizePass>> {
        vec![Box::new(NormalizeFontSizeAttrs)]
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new("block.set_font_size", "Set font size", |editor, args| {
                let size = args
                    .as_ref()
                    .and_then(|v| v.get("size"))
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| CommandError::new("Missing args.size"))?
                    .clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);

                set_block_font_size(editor, Some(size))
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        if tx.ops.is_empty() {
                            return Ok(());
                        }
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("Failed to set font size: {e:?}"))
                        })
                    })
            })
            .description("Set explicit font size for the active block(s).")
            .keywords(["font size", "text size", "typography"])
            .args_example(serde_json::json!({ "size": 16 })),
            CommandSpec::new(
                "block.unset_font_size",
                "Unset font size",
                |editor, _args| {
                    set_block_font_size(editor, None)
                        .map_err(CommandError::new)
                        .and_then(|tx| {
                            if tx.ops.is_empty() {
                                return Ok(());
                            }
                            editor.apply(tx).map_err(|e| {
                                CommandError::new(format!("Failed to unset font size: {e:?}"))
                            })
                        })
                },
            )
            .description("Remove explicit font size from the active block(s).")
            .keywords(["font size", "text size", "typography", "reset"]),
        ]
    }

    fn queries(&self) -> Vec<QuerySpec> {
        vec![QuerySpec {
            id: "block.font_size".to_string(),
            handler: std::sync::Arc::new(|editor, _args| Ok(active_block_font_size(editor))),
        }]
    }
}

struct NormalizeFontSizeAttrs;

impl NormalizePass for NormalizeFontSizeAttrs {
    fn id(&self) -> &'static str {
        "block.normalize_font_size_attrs"
    }

    fn run(&self, doc: &Document, registry: &PluginRegistry) -> Vec<Op> {
        let mut ops = Vec::new();

        fn normalize_font_size(
            el: &ElementNode,
            path: &[usize],
            registry: &PluginRegistry,
            ops: &mut Vec<Op>,
        ) {
            let Some(value) = el.attrs.get("font_size") else {
                return;
            };

            let Some(size) = value.as_u64() else {
                ops.push(Op::SetNodeAttrs {
                    path: path.to_vec(),
                    patch: crate::core::AttrPatch {
                        set: Attrs::default(),
                        remove: vec!["font_size".to_string()],
                    },
                });
                return;
            };

            if !element_is_text_block(el, registry) {
                ops.push(Op::SetNodeAttrs {
                    path: path.to_vec(),
                    patch: crate::core::AttrPatch {
                        set: Attrs::default(),
                        remove: vec!["font_size".to_string()],
                    },
                });
                return;
            }

            let clamped = size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
            if clamped != size {
                let mut set = Attrs::default();
                set.insert("font_size".to_string(), Value::Number(clamped.into()));
                ops.push(Op::SetNodeAttrs {
                    path: path.to_vec(),
                    patch: crate::core::AttrPatch {
                        set,
                        remove: Vec::new(),
                    },
                });
            }
        }

        fn walk(
            nodes: &[Node],
            path: &mut Vec<usize>,
            registry: &PluginRegistry,
            ops: &mut Vec<Op>,
        ) {
            for (ix, node) in nodes.iter().enumerate() {
                let Node::Element(el) = node else {
                    continue;
                };

                path.push(ix);
                normalize_font_size(el, path, registry, ops);

                if !element_is_text_block(el, registry) {
                    walk(&el.children, path, registry, ops);
                }

                path.pop();
            }
        }

        walk(&doc.children, &mut Vec::new(), registry, &mut ops);
        ops
    }
}

struct ListPlugin;

impl PlatePlugin for ListPlugin {
    fn id(&self) -> &'static str {
        "list"
    }

    fn node_specs(&self) -> Vec<NodeSpec> {
        vec![NodeSpec {
            kind: "list_item".to_string(),
            role: NodeRole::Block,
            is_void: false,
            children: ChildConstraint::InlineOnly,
        }]
    }

    fn normalize_passes(&self) -> Vec<Box<dyn NormalizePass>> {
        vec![Box::new(NormalizeOrderedListIndices)]
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new(
                "list.toggle_bulleted",
                "Toggle bulleted list",
                |editor, _args| toggle_list(editor, "bulleted").map_err(CommandError::new),
            )
            .description("Toggle bulleted list for the active block(s).")
            .keywords(["list", "bulleted", "unordered", "ul"]),
            CommandSpec::new(
                "list.toggle_ordered",
                "Toggle ordered list",
                |editor, _args| toggle_list(editor, "ordered").map_err(CommandError::new),
            )
            .description("Toggle ordered list for the active block(s).")
            .keywords(["list", "ordered", "numbered", "ol"]),
            CommandSpec::new("list.unwrap", "Unwrap list item", |editor, _args| {
                unwrap_list_item(editor).map_err(CommandError::new)
            })
            .description("Convert list item(s) back to paragraphs.")
            .keywords(["list", "unwrap", "paragraph"]),
        ]
    }

    fn queries(&self) -> Vec<QuerySpec> {
        vec![
            QuerySpec {
                id: "list.active_type".to_string(),
                handler: std::sync::Arc::new(|editor, _args| {
                    Ok(active_list_type(editor)
                        .map(Value::String)
                        .unwrap_or(Value::Null))
                }),
            },
            QuerySpec {
                id: "list.is_active".to_string(),
                handler: std::sync::Arc::new(|editor, args| {
                    let expected = args
                        .as_ref()
                        .and_then(|v| v.get("type"))
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| QueryError::new("Missing args.type"))?;
                    Ok(Value::Bool(
                        active_list_type(editor).as_deref() == Some(expected),
                    ))
                }),
            },
        ]
    }
}

struct MentionPlugin;

impl PlatePlugin for MentionPlugin {
    fn id(&self) -> &'static str {
        "mention"
    }

    fn node_specs(&self) -> Vec<NodeSpec> {
        vec![NodeSpec {
            kind: "mention".to_string(),
            role: NodeRole::Inline,
            is_void: true,
            children: ChildConstraint::None,
        }]
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new("mention.insert", "Insert mention", |editor, args| {
                let label = args
                    .as_ref()
                    .and_then(|v| v.get("label"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("mention")
                    .to_string();

                insert_mention(editor, label)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("Failed to insert mention: {e:?}"))
                        })
                    })
            })
            .description("Insert an inline mention node at the caret/selection.")
            .keywords(["mention", "@", "inline", "void"])
            .args_example(serde_json::json!({ "label": "@alice" })),
        ]
    }
}

struct EmojiPlugin;

impl PlatePlugin for EmojiPlugin {
    fn id(&self) -> &'static str {
        "emoji"
    }

    fn node_specs(&self) -> Vec<NodeSpec> {
        vec![NodeSpec {
            kind: "emoji".to_string(),
            role: NodeRole::Inline,
            is_void: true,
            children: ChildConstraint::None,
        }]
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new("emoji.insert", "Insert emoji", |editor, args| {
                let emoji = args
                    .as_ref()
                    .and_then(|v| v.get("emoji"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("😀")
                    .to_string();

                insert_emoji(editor, emoji)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("Failed to insert emoji: {e:?}"))
                        })
                    })
            })
            .description("Insert an inline emoji node at the caret/selection.")
            .keywords(["emoji", "emote", "inline", "void"])
            .args_example(serde_json::json!({ "emoji": "😀" })),
        ]
    }
}

struct ImagePlugin;

impl PlatePlugin for ImagePlugin {
    fn id(&self) -> &'static str {
        "image"
    }

    fn node_specs(&self) -> Vec<NodeSpec> {
        vec![NodeSpec {
            kind: "image".to_string(),
            role: NodeRole::Block,
            is_void: true,
            children: ChildConstraint::None,
        }]
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new("image.insert", "Insert image", |editor, args| {
                let src = args
                    .as_ref()
                    .and_then(|v| v.get("src"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| CommandError::new("Missing args.src"))?
                    .to_string();
                let alt = args
                    .as_ref()
                    .and_then(|v| v.get("alt"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                insert_image(editor, src, alt)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("Failed to insert image: {e:?}"))
                        })
                    })
            })
            .description("Insert a block image node (void).")
            .keywords(["image", "img", "media", "void"])
            .args_example(
                serde_json::json!({ "src": "https://example.com/image.png", "alt": "Alt text" }),
            ),
            CommandSpec::new("image.insert_many", "Insert images", |editor, args| {
                let srcs = args
                    .as_ref()
                    .and_then(|v| v.get("srcs"))
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| CommandError::new("Missing args.srcs (array)"))?;

                let mut items: Vec<(String, Option<String>)> = Vec::new();
                for src in srcs {
                    let Some(src) = src.as_str().map(|s| s.trim()).filter(|s| !s.is_empty()) else {
                        continue;
                    };
                    items.push((src.to_string(), None));
                }

                if items.is_empty() {
                    return Err(CommandError::new("No valid srcs"));
                }

                insert_images(editor, items)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("Failed to insert images: {e:?}"))
                        })
                    })
            })
            .description("Insert multiple block image nodes (void).")
            .keywords(["image", "img", "media", "void", "multiple"])
            .args_example(serde_json::json!({
                "srcs": [
                    "https://example.com/a.png",
                    "https://example.com/b.png"
                ]
            })),
        ]
    }
}

struct TablePlugin;

impl PlatePlugin for TablePlugin {
    fn id(&self) -> &'static str {
        "table"
    }

    fn node_specs(&self) -> Vec<NodeSpec> {
        vec![
            NodeSpec {
                kind: "table".to_string(),
                role: NodeRole::Block,
                is_void: false,
                children: ChildConstraint::BlockOnly,
            },
            NodeSpec {
                kind: "table_row".to_string(),
                role: NodeRole::Block,
                is_void: false,
                children: ChildConstraint::BlockOnly,
            },
            NodeSpec {
                kind: "table_cell".to_string(),
                role: NodeRole::Block,
                is_void: false,
                children: ChildConstraint::BlockOnly,
            },
        ]
    }

    fn normalize_passes(&self) -> Vec<Box<dyn NormalizePass>> {
        vec![Box::new(NormalizeTableStructure)]
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new("table.insert", "Insert table", |editor, args| {
                let rows = args
                    .as_ref()
                    .and_then(|v| v.get("rows"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(2)
                    .clamp(1, 32) as usize;
                let cols = args
                    .as_ref()
                    .and_then(|v| v.get("cols"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(2)
                    .clamp(1, 32) as usize;

                insert_table(editor, rows, cols)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("Failed to insert table: {e:?}"))
                        })
                    })
            })
            .description("Insert a table at the current selection.")
            .keywords(["table", "grid"])
            .args_example(serde_json::json!({ "rows": 2, "cols": 3 })),
            CommandSpec::new(
                "table.insert_row_above",
                "Insert row above",
                |editor, _args| {
                    insert_table_row_above(editor)
                        .map_err(CommandError::new)
                        .and_then(|tx| {
                            editor.apply(tx).map_err(|e| {
                                CommandError::new(format!("Failed to insert row above: {e:?}"))
                            })
                        })
                },
            )
            .description("Insert a new row above the current row.")
            .keywords(["table", "row", "insert"]),
            CommandSpec::new(
                "table.insert_row_below",
                "Insert row below",
                |editor, _args| {
                    insert_table_row_below(editor)
                        .map_err(CommandError::new)
                        .and_then(|tx| {
                            editor.apply(tx).map_err(|e| {
                                CommandError::new(format!("Failed to insert row below: {e:?}"))
                            })
                        })
                },
            )
            .description("Insert a new row below the current row.")
            .keywords(["table", "row", "insert"]),
            CommandSpec::new(
                "table.insert_col_left",
                "Insert column left",
                |editor, _args| {
                    insert_table_col_left(editor)
                        .map_err(CommandError::new)
                        .and_then(|tx| {
                            editor.apply(tx).map_err(|e| {
                                CommandError::new(format!("Failed to insert column left: {e:?}"))
                            })
                        })
                },
            )
            .description("Insert a new column to the left of the current column.")
            .keywords(["table", "column", "insert"]),
            CommandSpec::new(
                "table.insert_col_right",
                "Insert column right",
                |editor, _args| {
                    insert_table_col_right(editor)
                        .map_err(CommandError::new)
                        .and_then(|tx| {
                            editor.apply(tx).map_err(|e| {
                                CommandError::new(format!("Failed to insert column right: {e:?}"))
                            })
                        })
                },
            )
            .description("Insert a new column to the right of the current column.")
            .keywords(["table", "column", "insert"]),
            CommandSpec::new("table.delete_row", "Delete row", |editor, _args| {
                delete_table_row(editor)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor
                            .apply(tx)
                            .map_err(|e| CommandError::new(format!("Failed to delete row: {e:?}")))
                    })
            })
            .description("Delete the current table row.")
            .keywords(["table", "row", "delete", "remove"]),
            CommandSpec::new("table.delete_col", "Delete column", |editor, _args| {
                delete_table_col(editor)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("Failed to delete column: {e:?}"))
                        })
                    })
            })
            .description("Delete the current table column.")
            .keywords(["table", "column", "delete", "remove"]),
            CommandSpec::new("table.delete_table", "Delete table", |editor, _args| {
                delete_table(editor)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("Failed to delete table: {e:?}"))
                        })
                    })
            })
            .description("Delete the current table and replace it with a paragraph.")
            .keywords(["table", "delete", "remove"]),
        ]
    }

    fn queries(&self) -> Vec<QuerySpec> {
        vec![QuerySpec {
            id: "table.is_active".to_string(),
            handler: std::sync::Arc::new(|editor, _args| {
                let is_active =
                    ancestor_element_path(editor.doc(), &editor.selection().focus.path, "table")
                        .is_some();
                Ok(Value::Bool(is_active))
            }),
        }]
    }
}

struct NormalizeTableStructure;

impl NormalizePass for NormalizeTableStructure {
    fn id(&self) -> &'static str {
        "table.normalize_structure"
    }

    fn run(&self, doc: &Document, _registry: &PluginRegistry) -> Vec<Op> {
        let mut ops = Vec::new();

        fn normalize_table(table: &ElementNode, path: &[usize], ops: &mut Vec<Op>) {
            if table.children.is_empty() {
                let mut insert_path = path.to_vec();
                insert_path.push(0);
                ops.push(Op::InsertNode {
                    path: insert_path,
                    node: table_row_node(1),
                });
                return;
            }

            let mut max_cols = 1usize;
            for child in &table.children {
                let Node::Element(row) = child else {
                    continue;
                };
                if row.kind != "table_row" {
                    continue;
                }
                max_cols = max_cols.max(row.children.len().max(1));
            }

            for (row_ix, row_node) in table.children.iter().enumerate() {
                let Node::Element(row) = row_node else {
                    continue;
                };
                if row.kind != "table_row" {
                    continue;
                }

                if row.children.is_empty() {
                    let mut insert_cell_path = path.to_vec();
                    insert_cell_path.push(row_ix);
                    insert_cell_path.push(0);
                    ops.push(Op::InsertNode {
                        path: insert_cell_path,
                        node: table_cell_node(),
                    });
                    continue;
                }

                if row.children.len() < max_cols {
                    for col_ix in row.children.len()..max_cols {
                        let mut insert_cell_path = path.to_vec();
                        insert_cell_path.push(row_ix);
                        insert_cell_path.push(col_ix);
                        ops.push(Op::InsertNode {
                            path: insert_cell_path,
                            node: table_cell_node(),
                        });
                    }
                }

                for (cell_ix, cell_node) in row.children.iter().enumerate() {
                    let Node::Element(cell) = cell_node else {
                        continue;
                    };
                    if cell.kind != "table_cell" {
                        continue;
                    }
                    if cell.children.is_empty() {
                        let mut insert_para_path = path.to_vec();
                        insert_para_path.push(row_ix);
                        insert_para_path.push(cell_ix);
                        insert_para_path.push(0);
                        ops.push(Op::InsertNode {
                            path: insert_para_path,
                            node: Node::paragraph(""),
                        });
                    }
                }
            }
        }

        fn walk(nodes: &[Node], path: &mut Vec<usize>, ops: &mut Vec<Op>) {
            for (ix, node) in nodes.iter().enumerate() {
                let Node::Element(el) = node else {
                    continue;
                };
                path.push(ix);

                if el.kind == "table" {
                    normalize_table(el, path, ops);
                }

                walk(&el.children, path, ops);
                path.pop();
            }
        }

        walk(&doc.children, &mut Vec::new(), &mut ops);

        ops
    }
}

struct NormalizeOrderedListIndices;

impl NormalizePass for NormalizeOrderedListIndices {
    fn id(&self) -> &'static str {
        "list.normalize_ordered_indices"
    }

    fn run(&self, doc: &Document, registry: &PluginRegistry) -> Vec<Op> {
        let mut ops = Vec::new();

        fn normalize_container(
            children: &[Node],
            parent_path: &mut Vec<usize>,
            registry: &PluginRegistry,
            ops: &mut Vec<Op>,
        ) {
            let mut counter: u64 = 0;

            for (ix, node) in children.iter().enumerate() {
                let Node::Element(el) = node else {
                    counter = 0;
                    continue;
                };

                if el.kind != "list_item" {
                    counter = 0;
                    continue;
                }

                let list_type = el
                    .attrs
                    .get("list_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if list_type != "ordered" {
                    counter = 0;
                    if el.attrs.contains_key("list_index") {
                        let mut path = parent_path.clone();
                        path.push(ix);
                        ops.push(Op::SetNodeAttrs {
                            path,
                            patch: crate::core::AttrPatch {
                                set: Attrs::default(),
                                remove: vec!["list_index".to_string()],
                            },
                        });
                    }
                    continue;
                }

                counter += 1;
                let desired = Value::Number(serde_json::Number::from(counter));
                let current = el.attrs.get("list_index");
                if current != Some(&desired) {
                    let mut set = Attrs::default();
                    set.insert("list_index".to_string(), desired);
                    let mut path = parent_path.clone();
                    path.push(ix);
                    ops.push(Op::SetNodeAttrs {
                        path,
                        patch: crate::core::AttrPatch {
                            set,
                            remove: Vec::new(),
                        },
                    });
                }
            }

            for (ix, node) in children.iter().enumerate() {
                let Node::Element(el) = node else {
                    continue;
                };
                let spec_children = registry
                    .node_specs
                    .get(&el.kind)
                    .map(|s| s.children.clone())
                    .unwrap_or(ChildConstraint::Any);
                if spec_children == ChildConstraint::InlineOnly || el.children.is_empty() {
                    continue;
                }

                parent_path.push(ix);
                normalize_container(&el.children, parent_path, registry, ops);
                parent_path.pop();
            }
        }

        normalize_container(&doc.children, &mut Vec::new(), registry, &mut ops);

        ops
    }
}

fn active_list_type(editor: &crate::core::Editor) -> Option<String> {
    let focus = &editor.selection().focus;
    let block_path = focus.path.split_last().map(|(_, p)| p)?;
    let Some(Node::Element(el)) = node_at_path(editor.doc(), block_path) else {
        return None;
    };
    if el.kind != "list_item" {
        return None;
    }
    el.attrs
        .get("list_type")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn active_heading_level(editor: &crate::core::Editor) -> Value {
    let focus = &editor.selection().focus;
    let Some(block_path) = focus.path.split_last().map(|(_, p)| p) else {
        return Value::Null;
    };
    let Some(Node::Element(el)) = node_at_path(editor.doc(), block_path) else {
        return Value::Null;
    };
    if el.kind != "heading" {
        return Value::Null;
    }
    let level = el
        .attrs
        .get("level")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .clamp(1, 6);
    Value::Number(serde_json::Number::from(level))
}

fn active_is_code_block(editor: &crate::core::Editor) -> Value {
    let focus = &editor.selection().focus;
    let Some(block_path) = focus.path.split_last().map(|(_, p)| p) else {
        return Value::Bool(false);
    };
    let Some(Node::Element(el)) = node_at_path(editor.doc(), block_path) else {
        return Value::Bool(false);
    };
    Value::Bool(el.kind == "code_block")
}

fn active_indent_level(editor: &crate::core::Editor) -> Value {
    let focus = &editor.selection().focus;
    let Some(block_path) = focus.path.split_last().map(|(_, p)| p) else {
        return Value::Number(serde_json::Number::from(0u64));
    };
    let Some(Node::Element(el)) = node_at_path(editor.doc(), block_path) else {
        return Value::Number(serde_json::Number::from(0u64));
    };

    let level = if el.kind == "list_item" {
        el.attrs
            .get("list_level")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    } else {
        el.attrs.get("indent").and_then(|v| v.as_u64()).unwrap_or(0)
    }
    .min(MAX_INDENT_LEVEL);

    Value::Number(serde_json::Number::from(level))
}

fn active_block_align(editor: &crate::core::Editor) -> Value {
    let focus = &editor.selection().focus;
    let Some(block_path) = focus.path.split_last().map(|(_, p)| p) else {
        return Value::Null;
    };
    let Some(Node::Element(el)) = node_at_path(editor.doc(), block_path) else {
        return Value::Null;
    };
    if !element_is_text_block(&el, editor.registry()) {
        return Value::Null;
    }

    match el.attrs.get("align").and_then(|v| v.as_str()) {
        Some("center") => Value::String("center".to_string()),
        Some("right") => Value::String("right".to_string()),
        _ => Value::Null,
    }
}

fn active_block_font_size(editor: &crate::core::Editor) -> Value {
    let focus = &editor.selection().focus;
    let Some(block_path) = focus.path.split_last().map(|(_, p)| p) else {
        return Value::Null;
    };
    let Some(Node::Element(el)) = node_at_path(editor.doc(), block_path) else {
        return Value::Null;
    };
    if !element_is_text_block(&el, editor.registry()) {
        return Value::Null;
    }

    match el.attrs.get("font_size").and_then(|v| v.as_u64()) {
        Some(size) => Value::Number(serde_json::Number::from(
            size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE),
        )),
        None => Value::Null,
    }
}

fn set_heading(editor: &mut crate::core::Editor, level: u64) -> Result<Transaction, String> {
    let level = level.clamp(1, 6);
    let focus = editor.selection().focus.clone();
    let block_path = focus.path.split_last().map(|(_, p)| p).unwrap_or(&[]);
    if block_path.is_empty() {
        return Err("No active block".into());
    }
    let Some(node) = node_at_path(editor.doc(), block_path).cloned() else {
        return Err("No active block".into());
    };
    let selection_after = editor.selection().clone();

    let Node::Element(el) = node else {
        return Err("Active block is not a text block".into());
    };
    let spec_children = editor
        .registry()
        .node_specs()
        .get(&el.kind)
        .map(|s| s.children.clone())
        .unwrap_or(ChildConstraint::Any);
    if spec_children != ChildConstraint::InlineOnly {
        return Err("Active block is not a text block".into());
    }

    let current_level = (el.kind == "heading")
        .then(|| el.attrs.get("level").and_then(|v| v.as_u64()))
        .flatten()
        .unwrap_or(1)
        .clamp(1, 6);
    if el.kind == "heading" && current_level == level {
        return Ok(Transaction::new(Vec::new()).source("command:block.set_heading"));
    }

    let mut attrs = el.attrs.clone();
    attrs.insert(
        "level".to_string(),
        Value::Number(serde_json::Number::from(level)),
    );
    let next = Node::Element(ElementNode {
        kind: "heading".to_string(),
        attrs,
        children: el.children,
    });

    Ok(Transaction::new(vec![
        Op::RemoveNode {
            path: block_path.to_vec(),
        },
        Op::InsertNode {
            path: block_path.to_vec(),
            node: next,
        },
    ])
    .selection_after(selection_after)
    .source("command:block.set_heading"))
}

fn unset_heading(editor: &mut crate::core::Editor) -> Result<Transaction, String> {
    let focus = editor.selection().focus.clone();
    let block_path = focus.path.split_last().map(|(_, p)| p).unwrap_or(&[]);
    if block_path.is_empty() {
        return Err("No active block".into());
    }
    let Some(node) = node_at_path(editor.doc(), block_path).cloned() else {
        return Err("No active block".into());
    };
    let selection_after = editor.selection().clone();

    let Node::Element(el) = node else {
        return Err("Active block is not a text block".into());
    };
    if el.kind != "heading" {
        return Ok(Transaction::new(Vec::new()).source("command:block.unset_heading"));
    }

    let next = Node::Element(ElementNode {
        kind: "paragraph".to_string(),
        attrs: {
            let mut attrs = el.attrs.clone();
            attrs.remove("level");
            attrs
        },
        children: el.children,
    });

    Ok(Transaction::new(vec![
        Op::RemoveNode {
            path: block_path.to_vec(),
        },
        Op::InsertNode {
            path: block_path.to_vec(),
            node: next,
        },
    ])
    .selection_after(selection_after)
    .source("command:block.unset_heading"))
}

fn set_block_align(editor: &mut crate::core::Editor, align: String) -> Result<Transaction, String> {
    let align = match align.as_str() {
        "left" | "center" | "right" => align,
        _ => return Err("Invalid align value".into()),
    };

    let sel = editor.selection().clone();
    let selection_after = sel.clone();
    let (start, end) = ordered_selection_points(&sel);
    let Some(start_block_path) = start.path.split_last().map(|(_, p)| p.to_vec()) else {
        return Err("Selection start is not in a text block".into());
    };
    let Some(end_block_path) = end.path.split_last().map(|(_, p)| p.to_vec()) else {
        return Err("Selection end is not in a text block".into());
    };

    let blocks = text_blocks_in_order(editor.doc(), editor.registry());
    let start_index = blocks
        .iter()
        .position(|b| b.path == start_block_path)
        .ok_or_else(|| "Selection start is not in a text block".to_string())?;
    let end_index = blocks
        .iter()
        .position(|b| b.path == end_block_path)
        .ok_or_else(|| "Selection end is not in a text block".to_string())?;

    let (a, b) = if start_index <= end_index {
        (start_index, end_index)
    } else {
        (end_index, start_index)
    };

    let mut ops: Vec<Op> = Vec::new();
    for block in blocks.iter().take(b + 1).skip(a) {
        let el = block.el;

        if align == "left" {
            if el.attrs.get("align").is_some() {
                ops.push(Op::SetNodeAttrs {
                    path: block.path.clone(),
                    patch: crate::core::AttrPatch {
                        set: Attrs::default(),
                        remove: vec!["align".to_string()],
                    },
                });
            }
            continue;
        }

        if el.attrs.get("align").and_then(|v| v.as_str()) == Some(align.as_str()) {
            continue;
        }

        let mut set = Attrs::default();
        set.insert("align".to_string(), Value::String(align.clone()));
        ops.push(Op::SetNodeAttrs {
            path: block.path.clone(),
            patch: crate::core::AttrPatch {
                set,
                remove: Vec::new(),
            },
        });
    }

    Ok(Transaction::new(ops)
        .selection_after(selection_after)
        .source("command:block.set_align"))
}

fn set_block_font_size(
    editor: &mut crate::core::Editor,
    size: Option<u64>,
) -> Result<Transaction, String> {
    let size = size.map(|v| v.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE));

    let sel = editor.selection().clone();
    let selection_after = sel.clone();
    let (start, end) = ordered_selection_points(&sel);
    let Some(start_block_path) = start.path.split_last().map(|(_, p)| p.to_vec()) else {
        return Err("Selection start is not in a text block".into());
    };
    let Some(end_block_path) = end.path.split_last().map(|(_, p)| p.to_vec()) else {
        return Err("Selection end is not in a text block".into());
    };

    let blocks = text_blocks_in_order(editor.doc(), editor.registry());
    let start_index = blocks
        .iter()
        .position(|b| b.path == start_block_path)
        .ok_or_else(|| "Selection start is not in a text block".to_string())?;
    let end_index = blocks
        .iter()
        .position(|b| b.path == end_block_path)
        .ok_or_else(|| "Selection end is not in a text block".to_string())?;

    let (a, b) = if start_index <= end_index {
        (start_index, end_index)
    } else {
        (end_index, start_index)
    };

    let mut ops: Vec<Op> = Vec::new();
    for block in blocks.iter().take(b + 1).skip(a) {
        let el = block.el;

        match size {
            None => {
                if el.attrs.get("font_size").is_none() {
                    continue;
                }
                ops.push(Op::SetNodeAttrs {
                    path: block.path.clone(),
                    patch: crate::core::AttrPatch {
                        set: Attrs::default(),
                        remove: vec!["font_size".to_string()],
                    },
                });
            }
            Some(size) => {
                if el.attrs.get("font_size").and_then(|v| v.as_u64()) == Some(size) {
                    continue;
                }
                let mut set = Attrs::default();
                set.insert("font_size".to_string(), Value::Number(size.into()));
                ops.push(Op::SetNodeAttrs {
                    path: block.path.clone(),
                    patch: crate::core::AttrPatch {
                        set,
                        remove: Vec::new(),
                    },
                });
            }
        }
    }

    Ok(Transaction::new(ops)
        .selection_after(selection_after)
        .source(match size {
            Some(_) => "command:block.set_font_size",
            None => "command:block.unset_font_size",
        }))
}

fn toggle_code_block(editor: &mut crate::core::Editor) -> Result<Transaction, String> {
    let focus = editor.selection().focus.clone();
    let block_path = focus.path.split_last().map(|(_, p)| p).unwrap_or(&[]);
    if block_path.is_empty() {
        return Err("No active block".into());
    }
    let Some(node) = node_at_path(editor.doc(), block_path).cloned() else {
        return Err("No active block".into());
    };
    let selection_after = editor.selection().clone();

    let Node::Element(el) = node else {
        return Err("Active block is not a text block".into());
    };
    if !element_is_text_block(&el, editor.registry()) {
        return Err("Active block is not a text block".into());
    }

    let (next_kind, mut attrs) = if el.kind == "code_block" {
        ("paragraph", el.attrs.clone())
    } else {
        ("code_block", el.attrs.clone())
    };

    if next_kind == "code_block" {
        attrs.remove("level");
        attrs.remove("checked");
        attrs.remove("list_index");

        if let Some(level) = attrs.remove("list_level") {
            if !attrs.contains_key("indent") {
                attrs.insert("indent".to_string(), level);
            }
        }
        attrs.remove("list_type");
    } else {
        attrs.remove("language");
    }

    let next = Node::Element(ElementNode {
        kind: next_kind.to_string(),
        attrs,
        children: el.children,
    });

    Ok(Transaction::new(vec![
        Op::RemoveNode {
            path: block_path.to_vec(),
        },
        Op::InsertNode {
            path: block_path.to_vec(),
            node: next,
        },
    ])
    .selection_after(selection_after)
    .source("command:code_block.toggle"))
}

fn is_in_blockquote(editor: &crate::core::Editor) -> bool {
    nearest_blockquote_path(editor.doc(), &editor.selection().focus.path).is_some()
}

fn nearest_blockquote_path(doc: &Document, point_path: &[usize]) -> Option<Path> {
    let mut path: Path = point_path.to_vec();
    while !path.is_empty() {
        if let Some(Node::Element(el)) = node_at_path(doc, &path) {
            if el.kind == "blockquote" {
                return Some(path);
            }
        }
        path.pop();
    }
    None
}

fn children_at_path<'a>(doc: &'a Document, parent_path: &[usize]) -> Option<&'a [Node]> {
    if parent_path.is_empty() {
        return Some(&doc.children);
    }
    match node_at_path(doc, parent_path)? {
        Node::Element(el) => Some(&el.children),
        Node::Void(_) | Node::Text(_) => None,
    }
}

fn wrap_selection_in_blockquote(editor: &mut crate::core::Editor) -> Result<Transaction, String> {
    let sel = editor.selection().clone();
    let (start, end) = ordered_selection_points(&sel);
    let start_block_path = start
        .path
        .split_last()
        .map(|(_, p)| p.to_vec())
        .ok_or_else(|| "Selection start is not in a text block".to_string())?;
    let end_block_path = end
        .path
        .split_last()
        .map(|(_, p)| p.to_vec())
        .ok_or_else(|| "Selection end is not in a text block".to_string())?;

    let (start_ix, start_parent) = start_block_path
        .split_last()
        .ok_or_else(|| "Selection start is not a block node".to_string())?;
    let (end_ix, end_parent) = end_block_path
        .split_last()
        .ok_or_else(|| "Selection end is not a block node".to_string())?;

    if start_parent != end_parent {
        return Err("Selection must be within a single block container".into());
    }

    let (start_ix, end_ix) = if start_ix <= end_ix {
        (*start_ix, *end_ix)
    } else {
        (*end_ix, *start_ix)
    };

    let Some(parent_children) = children_at_path(editor.doc(), start_parent) else {
        return Err("Selection parent is not a container".into());
    };
    if start_ix >= parent_children.len() || end_ix >= parent_children.len() {
        return Err("Selection block range is out of bounds".into());
    }

    let selected: Vec<Node> = parent_children
        .iter()
        .cloned()
        .take(end_ix + 1)
        .skip(start_ix)
        .collect();

    let quote = Node::Element(ElementNode {
        kind: "blockquote".to_string(),
        attrs: Attrs::default(),
        children: selected,
    });

    let mut ops: Vec<Op> = Vec::new();
    for ix in (start_ix..=end_ix).rev() {
        let mut path = start_parent.to_vec();
        path.push(ix);
        ops.push(Op::RemoveNode { path });
    }
    let mut insert_path = start_parent.to_vec();
    insert_path.push(start_ix);
    ops.push(Op::InsertNode {
        path: insert_path.clone(),
        node: quote,
    });

    let remap_point = |point: &Point| -> Point {
        if !point.path.starts_with(start_parent) || point.path.len() < start_parent.len() + 2 {
            return point.clone();
        }
        let block_ix = point.path[start_parent.len()];
        if block_ix < start_ix || block_ix > end_ix {
            return point.clone();
        }
        let mut new_path = start_parent.to_vec();
        new_path.push(start_ix);
        new_path.push(block_ix - start_ix);
        new_path.extend_from_slice(&point.path[start_parent.len() + 1..]);
        Point {
            path: new_path,
            offset: point.offset,
        }
    };

    let selection_after = Selection {
        anchor: remap_point(&sel.anchor),
        focus: remap_point(&sel.focus),
    };

    Ok(Transaction::new(ops)
        .selection_after(selection_after)
        .source("command:blockquote.wrap_selection"))
}

fn unwrap_nearest_blockquote(editor: &mut crate::core::Editor) -> Result<Transaction, String> {
    let sel = editor.selection().clone();
    let Some(quote_path) = nearest_blockquote_path(editor.doc(), &sel.focus.path) else {
        return Ok(Transaction::new(Vec::new()).source("command:blockquote.unwrap"));
    };
    let (quote_ix, parent_path) = quote_path
        .split_last()
        .ok_or_else(|| "Invalid blockquote path".to_string())?;
    let quote_ix = *quote_ix;
    let parent_path = parent_path.to_vec();
    let Some(Node::Element(quote_el)) = node_at_path(editor.doc(), &quote_path).cloned() else {
        return Err("Blockquote node not found".into());
    };
    if quote_el.kind != "blockquote" {
        return Ok(Transaction::new(Vec::new()).source("command:blockquote.unwrap"));
    }

    let children = quote_el.children;

    let mut ops: Vec<Op> = Vec::new();
    ops.push(Op::RemoveNode {
        path: quote_path.clone(),
    });

    for (i, node) in children.into_iter().enumerate() {
        let mut path = parent_path.clone();
        path.push(quote_ix + i);
        ops.push(Op::InsertNode { path, node });
    }

    let remap_point = |point: &Point| -> Point {
        if point.path.len() < quote_path.len() + 1 {
            return point.clone();
        }
        if !point.path.starts_with(&quote_path) {
            return point.clone();
        }
        let inner_ix = point.path[quote_path.len()];
        let mut new_path = parent_path.clone();
        new_path.push(quote_ix + inner_ix);
        new_path.extend_from_slice(&point.path[quote_path.len() + 1..]);
        Point {
            path: new_path,
            offset: point.offset,
        }
    };

    let selection_after = Selection {
        anchor: remap_point(&sel.anchor),
        focus: remap_point(&sel.focus),
    };

    Ok(Transaction::new(ops)
        .selection_after(selection_after)
        .source("command:blockquote.unwrap"))
}

fn is_in_toggle(editor: &crate::core::Editor) -> bool {
    nearest_toggle_path(editor.doc(), &editor.selection().focus.path).is_some()
}

fn active_toggle_collapsed(editor: &crate::core::Editor) -> bool {
    let Some(toggle_path) = nearest_toggle_path(editor.doc(), &editor.selection().focus.path)
    else {
        return false;
    };
    let Some(Node::Element(el)) = node_at_path(editor.doc(), &toggle_path) else {
        return false;
    };
    if el.kind != "toggle" {
        return false;
    }
    el.attrs
        .get("collapsed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn nearest_toggle_path(doc: &Document, point_path: &[usize]) -> Option<Path> {
    let mut path: Path = point_path.to_vec();
    while !path.is_empty() {
        if let Some(Node::Element(el)) = node_at_path(doc, &path) {
            if el.kind == "toggle" {
                return Some(path);
            }
        }
        path.pop();
    }
    None
}

fn wrap_selection_in_toggle(editor: &mut crate::core::Editor) -> Result<Transaction, String> {
    let sel = editor.selection().clone();
    let (start, end) = ordered_selection_points(&sel);
    let start_block_path = start
        .path
        .split_last()
        .map(|(_, p)| p.to_vec())
        .ok_or_else(|| "Selection start is not in a text block".to_string())?;
    let end_block_path = end
        .path
        .split_last()
        .map(|(_, p)| p.to_vec())
        .ok_or_else(|| "Selection end is not in a text block".to_string())?;

    let (start_ix, start_parent) = start_block_path
        .split_last()
        .ok_or_else(|| "Selection start is not a block node".to_string())?;
    let (end_ix, end_parent) = end_block_path
        .split_last()
        .ok_or_else(|| "Selection end is not a block node".to_string())?;

    if start_parent != end_parent {
        return Err("Selection must be within a single block container".into());
    }

    let (start_ix, end_ix) = if start_ix <= end_ix {
        (*start_ix, *end_ix)
    } else {
        (*end_ix, *start_ix)
    };

    let Some(parent_children) = children_at_path(editor.doc(), start_parent) else {
        return Err("Selection parent is not a container".into());
    };
    if start_ix >= parent_children.len() || end_ix >= parent_children.len() {
        return Err("Selection block range is out of bounds".into());
    }

    let selected: Vec<Node> = parent_children
        .iter()
        .cloned()
        .take(end_ix + 1)
        .skip(start_ix)
        .collect();

    let title_offset = selected
        .first()
        .and_then(|n| match n {
            Node::Element(el) => Some(el.kind.as_str()),
            _ => None,
        })
        .is_some_and(|kind| matches!(kind, "paragraph" | "heading"));

    let mut children = Vec::new();
    if !title_offset {
        children.push(Node::paragraph(""));
    }
    children.extend(selected);
    let title_shift = if title_offset { 0 } else { 1 };

    let mut attrs = Attrs::default();
    attrs.insert("collapsed".to_string(), Value::Bool(false));
    let toggle = Node::Element(ElementNode {
        kind: "toggle".to_string(),
        attrs,
        children,
    });

    let mut ops: Vec<Op> = Vec::new();
    for ix in (start_ix..=end_ix).rev() {
        let mut path = start_parent.to_vec();
        path.push(ix);
        ops.push(Op::RemoveNode { path });
    }
    let mut insert_path = start_parent.to_vec();
    insert_path.push(start_ix);
    ops.push(Op::InsertNode {
        path: insert_path.clone(),
        node: toggle,
    });

    let remap_point = |point: &Point| -> Point {
        if !point.path.starts_with(start_parent) || point.path.len() < start_parent.len() + 2 {
            return point.clone();
        }
        let block_ix = point.path[start_parent.len()];
        if block_ix < start_ix || block_ix > end_ix {
            return point.clone();
        }
        let mut new_path = start_parent.to_vec();
        new_path.push(start_ix);
        new_path.push(title_shift + (block_ix - start_ix));
        new_path.extend_from_slice(&point.path[start_parent.len() + 1..]);
        Point {
            path: new_path,
            offset: point.offset,
        }
    };

    let selection_after = Selection {
        anchor: remap_point(&sel.anchor),
        focus: remap_point(&sel.focus),
    };

    Ok(Transaction::new(ops)
        .selection_after(selection_after)
        .source("command:toggle.wrap_selection"))
}

fn unwrap_nearest_toggle(editor: &mut crate::core::Editor) -> Result<Transaction, String> {
    let sel = editor.selection().clone();
    let Some(toggle_path) = nearest_toggle_path(editor.doc(), &sel.focus.path) else {
        return Ok(Transaction::new(Vec::new()).source("command:toggle.unwrap"));
    };
    let (toggle_ix, parent_path) = toggle_path
        .split_last()
        .ok_or_else(|| "Invalid toggle path".to_string())?;
    let toggle_ix = *toggle_ix;
    let parent_path = parent_path.to_vec();
    let Some(Node::Element(toggle_el)) = node_at_path(editor.doc(), &toggle_path).cloned() else {
        return Err("Toggle node not found".into());
    };
    if toggle_el.kind != "toggle" {
        return Ok(Transaction::new(Vec::new()).source("command:toggle.unwrap"));
    }

    let children = toggle_el.children;

    let mut ops: Vec<Op> = Vec::new();
    ops.push(Op::RemoveNode {
        path: toggle_path.clone(),
    });

    for (i, node) in children.into_iter().enumerate() {
        let mut path = parent_path.clone();
        path.push(toggle_ix + i);
        ops.push(Op::InsertNode { path, node });
    }

    let remap_point = |point: &Point| -> Point {
        if point.path.len() < toggle_path.len() + 1 {
            return point.clone();
        }
        if !point.path.starts_with(&toggle_path) {
            return point.clone();
        }
        let inner_ix = point.path[toggle_path.len()];
        let mut new_path = parent_path.clone();
        new_path.push(toggle_ix + inner_ix);
        new_path.extend_from_slice(&point.path[toggle_path.len() + 1..]);
        Point {
            path: new_path,
            offset: point.offset,
        }
    };

    let selection_after = Selection {
        anchor: remap_point(&sel.anchor),
        focus: remap_point(&sel.focus),
    };

    Ok(Transaction::new(ops)
        .selection_after(selection_after)
        .source("command:toggle.unwrap"))
}

fn toggle_toggle_collapsed(
    editor: &mut crate::core::Editor,
    args: Option<&Value>,
) -> Result<Transaction, String> {
    let sel = editor.selection().clone();
    let toggle_path = parse_path_arg(args)
        .filter(|path| {
            matches!(
                node_at_path(editor.doc(), path),
                Some(Node::Element(el)) if el.kind == "toggle"
            )
        })
        .or_else(|| nearest_toggle_path(editor.doc(), &sel.focus.path));

    let Some(toggle_path) = toggle_path else {
        return Ok(Transaction::new(Vec::new()).source("command:toggle.toggle_collapsed"));
    };

    let Some(Node::Element(el)) = node_at_path(editor.doc(), &toggle_path) else {
        return Err("Toggle node not found".into());
    };

    let current = el
        .attrs
        .get("collapsed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let next = !current;

    let mut set = Attrs::default();
    set.insert("collapsed".to_string(), Value::Bool(next));

    let mut selection_after = sel.clone();
    if next {
        let mut title_block_path = toggle_path.clone();
        title_block_path.push(0);

        let title_point = node_at_path(editor.doc(), &title_block_path)
            .and_then(|node| match node {
                Node::Element(el) => Some(el),
                _ => None,
            })
            .map(|title_el| {
                let global_len: usize = title_el
                    .children
                    .iter()
                    .map(|n| match n {
                        Node::Text(t) => t.text.len(),
                        Node::Void(v) => v.inline_text_len(),
                        Node::Element(_) => 0,
                    })
                    .sum();
                point_for_global_offset(&title_block_path, &title_el.children, global_len)
            })
            .or_else(|| {
                normalize_point_to_existing_text(
                    editor.doc(),
                    &Point::new(title_block_path.clone(), 0),
                )
            })
            .unwrap_or_else(|| Point::new(title_block_path.clone(), 0));

        let remap_point = |point: &Point| -> Point {
            if point.path.len() < toggle_path.len() + 1 {
                return point.clone();
            }
            if !point.path.starts_with(&toggle_path) {
                return point.clone();
            }
            let child_ix = point.path[toggle_path.len()];
            if child_ix == 0 {
                return point.clone();
            }
            title_point.clone()
        };

        selection_after = Selection {
            anchor: remap_point(&sel.anchor),
            focus: remap_point(&sel.focus),
        };
    }

    Ok(Transaction::new(vec![Op::SetNodeAttrs {
        path: toggle_path,
        patch: crate::core::AttrPatch {
            set,
            remove: Vec::new(),
        },
    }])
    .selection_after(selection_after)
    .source("command:toggle.toggle_collapsed"))
}

fn is_in_todo(editor: &crate::core::Editor) -> bool {
    let focus = &editor.selection().focus;
    let Some(block_path) = focus.path.split_last().map(|(_, p)| p) else {
        return false;
    };
    match node_at_path(editor.doc(), block_path) {
        Some(Node::Element(el)) if el.kind == "todo_item" => true,
        _ => false,
    }
}

fn active_todo_checked(editor: &crate::core::Editor) -> bool {
    let focus = &editor.selection().focus;
    let Some(block_path) = focus.path.split_last().map(|(_, p)| p) else {
        return false;
    };
    let Some(Node::Element(el)) = node_at_path(editor.doc(), block_path) else {
        return false;
    };
    if el.kind != "todo_item" {
        return false;
    }
    el.attrs
        .get("checked")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn toggle_todo_in_block_path(block_node: Node, checked: bool) -> Result<Node, String> {
    let Node::Element(el) = block_node else {
        return Err("Active block is not a block element".into());
    };

    match el.kind.as_str() {
        "paragraph" => {
            let mut attrs = el.attrs.clone();
            attrs.insert("checked".to_string(), Value::Bool(checked));
            Ok(Node::Element(ElementNode {
                kind: "todo_item".to_string(),
                attrs,
                children: el.children,
            }))
        }
        "todo_item" => Ok(Node::Element(ElementNode {
            kind: "paragraph".to_string(),
            attrs: {
                let mut attrs = el.attrs.clone();
                attrs.remove("checked");
                attrs
            },
            children: el.children,
        })),
        _ => Ok(Node::Element(el)),
    }
}

fn toggle_todo(editor: &mut crate::core::Editor) -> Result<Transaction, String> {
    let selection_after = editor.selection().clone();
    let sel = editor.selection().clone();
    let (start, end) = ordered_selection_points(&sel);
    let Some(start_block_path) = start.path.split_last().map(|(_, p)| p.to_vec()) else {
        return Err("Selection start is not in a text block".into());
    };
    let Some(end_block_path) = end.path.split_last().map(|(_, p)| p.to_vec()) else {
        return Err("Selection end is not in a text block".into());
    };

    let Some((focus_ix, focus_parent)) = selection_after
        .focus
        .path
        .split_last()
        .and_then(|(_, p)| p.split_last())
        .map(|(ix, parent)| (*ix, parent.to_vec()))
    else {
        return Err("No active block".into());
    };

    let (start_ix, start_parent) = start_block_path
        .split_last()
        .ok_or_else(|| "Selection start is not a block node".to_string())?;
    let (end_ix, end_parent) = end_block_path
        .split_last()
        .ok_or_else(|| "Selection end is not a block node".to_string())?;

    let (parent_path, a, b) = if start_parent == end_parent {
        let (a, b) = if start_ix <= end_ix {
            (*start_ix, *end_ix)
        } else {
            (*end_ix, *start_ix)
        };
        (start_parent.to_vec(), a, b)
    } else {
        (focus_parent, focus_ix, focus_ix)
    };

    let Some(parent_children) = children_at_path(editor.doc(), &parent_path) else {
        return Err("Selection parent is not a container".into());
    };
    if a >= parent_children.len() || b >= parent_children.len() {
        return Err("Selection block range is out of bounds".into());
    }

    let mut ops: Vec<Op> = Vec::new();
    for ix in a..=b {
        let Some(node) = parent_children.get(ix).cloned() else {
            continue;
        };
        let Node::Element(el) = &node else {
            continue;
        };
        if el.kind != "paragraph" && el.kind != "todo_item" {
            continue;
        }

        let next = toggle_todo_in_block_path(node, false)?;
        let mut path = parent_path.clone();
        path.push(ix);
        ops.push(Op::RemoveNode { path: path.clone() });
        ops.push(Op::InsertNode { path, node: next });
    }

    Ok(Transaction::new(ops)
        .selection_after(selection_after)
        .source("command:todo.toggle"))
}

fn parse_path_arg(args: Option<&Value>) -> Option<Vec<usize>> {
    let path = args.as_ref()?.get("path")?.as_array()?;
    let mut out = Vec::with_capacity(path.len());
    for v in path {
        out.push(v.as_u64()? as usize);
    }
    Some(out)
}

fn toggle_todo_checked(
    editor: &mut crate::core::Editor,
    args: Option<&Value>,
) -> Result<Transaction, String> {
    let selection_after = editor.selection().clone();

    let block_path = parse_path_arg(args).or_else(|| {
        selection_after
            .focus
            .path
            .split_last()
            .map(|(_, p)| p.to_vec())
    });
    let Some(block_path) = block_path else {
        return Err("No active block".into());
    };

    let Some(Node::Element(el)) = node_at_path(editor.doc(), &block_path) else {
        return Ok(Transaction::new(Vec::new()).source("command:todo.toggle_checked"));
    };
    if el.kind != "todo_item" {
        return Ok(Transaction::new(Vec::new()).source("command:todo.toggle_checked"));
    }

    let checked = el
        .attrs
        .get("checked")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let next = Value::Bool(!checked);
    let mut set = Attrs::default();
    set.insert("checked".to_string(), next);

    Ok(Transaction::new(vec![Op::SetNodeAttrs {
        path: block_path,
        patch: crate::core::AttrPatch {
            set,
            remove: Vec::new(),
        },
    }])
    .selection_after(selection_after)
    .source("command:todo.toggle_checked"))
}

#[derive(Clone, Copy)]
enum IndentDirection {
    Increase,
    Decrease,
}

fn adjust_indent(
    editor: &mut crate::core::Editor,
    direction: IndentDirection,
) -> Result<Transaction, String> {
    let sel = editor.selection().clone();
    let selection_after = sel.clone();
    let (start, end) = ordered_selection_points(&sel);
    let Some(start_block_path) = start.path.split_last().map(|(_, p)| p.to_vec()) else {
        return Err("Selection start is not in a text block".into());
    };
    let Some(end_block_path) = end.path.split_last().map(|(_, p)| p.to_vec()) else {
        return Err("Selection end is not in a text block".into());
    };

    let blocks = text_blocks_in_order(editor.doc(), editor.registry());
    let start_index = blocks
        .iter()
        .position(|b| b.path == start_block_path)
        .ok_or_else(|| "Selection start is not in a text block".to_string())?;
    let end_index = blocks
        .iter()
        .position(|b| b.path == end_block_path)
        .ok_or_else(|| "Selection end is not in a text block".to_string())?;

    let (a, b) = if start_index <= end_index {
        (start_index, end_index)
    } else {
        (end_index, start_index)
    };

    let mut ops: Vec<Op> = Vec::new();
    for block in blocks.iter().take(b + 1).skip(a) {
        let el = block.el;

        let (key, current) = if el.kind == "list_item" {
            (
                "list_level",
                el.attrs
                    .get("list_level")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
            )
        } else {
            (
                "indent",
                el.attrs.get("indent").and_then(|v| v.as_u64()).unwrap_or(0),
            )
        };

        let next = match direction {
            IndentDirection::Increase => current.saturating_add(1).min(MAX_INDENT_LEVEL),
            IndentDirection::Decrease => current.saturating_sub(1),
        };
        if next == current {
            continue;
        }

        if next == 0 {
            ops.push(Op::SetNodeAttrs {
                path: block.path.clone(),
                patch: crate::core::AttrPatch {
                    set: Attrs::default(),
                    remove: vec![key.to_string()],
                },
            });
            continue;
        }

        let mut set = Attrs::default();
        set.insert(
            key.to_string(),
            Value::Number(serde_json::Number::from(next)),
        );
        ops.push(Op::SetNodeAttrs {
            path: block.path.clone(),
            patch: crate::core::AttrPatch {
                set,
                remove: Vec::new(),
            },
        });
    }

    let source = match direction {
        IndentDirection::Increase => "command:block.indent_increase",
        IndentDirection::Decrease => "command:block.indent_decrease",
    };

    Ok(Transaction::new(ops)
        .selection_after(selection_after)
        .source(source))
}

fn toggle_list(editor: &mut crate::core::Editor, list_type: &str) -> Result<(), String> {
    let focus = editor.selection().focus.clone();
    let block_path = focus.path.split_last().map(|(_, p)| p).unwrap_or(&[]);
    let Some(node) = node_at_path(editor.doc(), block_path).cloned() else {
        return Err("No active block".into());
    };
    let selection_after = editor.selection().clone();

    let next = match node {
        Node::Element(el) if el.kind == "paragraph" => {
            let indent = el.attrs.get("indent").and_then(|v| v.as_u64()).unwrap_or(0);
            let mut attrs = el.attrs.clone();
            attrs.remove("indent");
            attrs.insert(
                "list_type".to_string(),
                Value::String(list_type.to_string()),
            );
            if indent > 0 {
                attrs.insert(
                    "list_level".to_string(),
                    Value::Number(serde_json::Number::from(indent.min(MAX_INDENT_LEVEL))),
                );
            }
            Node::Element(ElementNode {
                kind: "list_item".to_string(),
                attrs,
                children: el.children,
            })
        }
        Node::Element(el) if el.kind == "list_item" => {
            let current = el
                .attrs
                .get("list_type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if current == list_type {
                let level = el
                    .attrs
                    .get("list_level")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let mut attrs = el.attrs.clone();
                attrs.remove("list_type");
                attrs.remove("list_index");
                attrs.remove("list_level");
                if level > 0 {
                    attrs.insert(
                        "indent".to_string(),
                        Value::Number(serde_json::Number::from(level.min(MAX_INDENT_LEVEL))),
                    );
                }
                Node::Element(ElementNode {
                    kind: "paragraph".to_string(),
                    attrs,
                    children: el.children,
                })
            } else {
                let mut attrs = el.attrs.clone();
                attrs.insert(
                    "list_type".to_string(),
                    Value::String(list_type.to_string()),
                );
                attrs.remove("list_index");
                Node::Element(ElementNode {
                    kind: "list_item".to_string(),
                    attrs,
                    children: el.children,
                })
            }
        }
        _ => return Err("Active block is not a text block".into()),
    };

    let tx = Transaction::new(vec![
        Op::RemoveNode {
            path: block_path.to_vec(),
        },
        Op::InsertNode {
            path: block_path.to_vec(),
            node: next,
        },
    ])
    .selection_after(selection_after)
    .source(format!("command:list.toggle_{list_type}"));

    editor
        .apply(tx)
        .map_err(|e| format!("Failed to toggle list: {e:?}"))
}

fn unwrap_list_item(editor: &mut crate::core::Editor) -> Result<(), String> {
    let focus = editor.selection().focus.clone();
    let block_path = focus.path.split_last().map(|(_, p)| p).unwrap_or(&[]);
    let Some(node) = node_at_path(editor.doc(), block_path).cloned() else {
        return Err("No active block".into());
    };
    let selection_after = editor.selection().clone();

    let Node::Element(el) = node else {
        return Err("Active block is not a text block".into());
    };
    if el.kind != "list_item" {
        return Ok(());
    }

    let level = el
        .attrs
        .get("list_level")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if level > 0 {
        let next_level = level.saturating_sub(1);
        let patch = if next_level == 0 {
            crate::core::AttrPatch {
                set: Attrs::default(),
                remove: vec!["list_level".to_string()],
            }
        } else {
            let mut set = Attrs::default();
            set.insert(
                "list_level".to_string(),
                Value::Number(serde_json::Number::from(next_level)),
            );
            crate::core::AttrPatch {
                set,
                remove: Vec::new(),
            }
        };

        let tx = Transaction::new(vec![Op::SetNodeAttrs {
            path: block_path.to_vec(),
            patch,
        }])
        .selection_after(selection_after)
        .source("command:list.unwrap:outdent");

        return editor
            .apply(tx)
            .map_err(|e| format!("Failed to outdent list: {e:?}"));
    }

    let mut attrs = el.attrs.clone();
    attrs.remove("list_type");
    attrs.remove("list_index");
    attrs.remove("list_level");

    let next = Node::Element(ElementNode {
        kind: "paragraph".to_string(),
        attrs,
        children: el.children,
    });

    let tx = Transaction::new(vec![
        Op::RemoveNode {
            path: block_path.to_vec(),
        },
        Op::InsertNode {
            path: block_path.to_vec(),
            node: next,
        },
    ])
    .selection_after(selection_after)
    .source("command:list.unwrap");

    editor
        .apply(tx)
        .map_err(|e| format!("Failed to unwrap list: {e:?}"))
}

fn table_cell_node() -> Node {
    Node::Element(ElementNode {
        kind: "table_cell".to_string(),
        attrs: Attrs::default(),
        children: vec![Node::paragraph("")],
    })
}

fn table_row_node(cols: usize) -> Node {
    let cols = cols.max(1);
    Node::Element(ElementNode {
        kind: "table_row".to_string(),
        attrs: Attrs::default(),
        children: (0..cols).map(|_| table_cell_node()).collect(),
    })
}

fn table_node(rows: usize, cols: usize) -> Node {
    let rows = rows.max(1);
    let cols = cols.max(1);
    Node::Element(ElementNode {
        kind: "table".to_string(),
        attrs: Attrs::default(),
        children: (0..rows).map(|_| table_row_node(cols)).collect(),
    })
}

fn ancestor_element_path(doc: &Document, path: &[usize], kind: &str) -> Option<Path> {
    if path.is_empty() {
        return None;
    }

    for len in (1..=path.len()).rev() {
        let candidate = &path[..len];
        if let Some(Node::Element(el)) = node_at_path(doc, candidate) {
            if el.kind == kind {
                return Some(candidate.to_vec());
            }
        }
    }
    None
}

fn equal_widths(cols: usize) -> Vec<f64> {
    let cols = cols.max(2);
    let w = 1.0 / cols as f64;
    vec![w; cols]
}

fn columns_widths_valid(columns: &ElementNode, expected_len: usize) -> bool {
    if expected_len < 2 {
        return false;
    }
    let Some(widths) = columns.attrs.get("widths").and_then(|v| v.as_array()) else {
        return false;
    };
    if widths.len() != expected_len {
        return false;
    }

    let mut sum = 0.0f64;
    for v in widths {
        let Some(w) = v.as_f64() else {
            return false;
        };
        if !w.is_finite() || w <= 0.0 {
            return false;
        }
        sum += w;
    }

    (sum - 1.0).abs() <= 0.01
}

fn column_node(children: Vec<Node>) -> Node {
    Node::Element(ElementNode {
        kind: "column".to_string(),
        attrs: Attrs::default(),
        children,
    })
}

fn columns_node(cols: usize) -> Node {
    let cols = cols.clamp(2, 6);
    let widths = equal_widths(cols);
    let widths_json: Vec<Value> = widths
        .iter()
        .filter_map(|w| serde_json::Number::from_f64(*w).map(Value::Number))
        .collect();

    let mut attrs = Attrs::default();
    attrs.insert("widths".to_string(), Value::Array(widths_json));

    Node::Element(ElementNode {
        kind: "columns".to_string(),
        attrs,
        children: (0..cols)
            .map(|_| column_node(vec![Node::paragraph("")]))
            .collect(),
    })
}

fn insert_columns(editor: &crate::core::Editor, cols: usize) -> Result<Transaction, String> {
    let cols = cols.clamp(2, 6);
    let focus = editor.selection().focus.clone();
    let block_path = focus.path.split_last().map(|(_, p)| p).unwrap_or(&[]);

    let (parent_path, insert_at) = if block_path.is_empty() {
        (Vec::new(), editor.doc().children.len())
    } else {
        let (block_ix, parent) = block_path.split_last().unwrap();
        (parent.to_vec(), block_ix + 1)
    };

    let columns_path = {
        let mut path = parent_path.clone();
        path.push(insert_at);
        path
    };
    let paragraph_path = {
        let mut path = parent_path.clone();
        path.push(insert_at + 1);
        path
    };

    let mut selection_path = columns_path.clone();
    selection_path.extend([0, 0, 0]);

    Ok(Transaction::new(vec![
        Op::InsertNode {
            path: columns_path,
            node: columns_node(cols),
        },
        Op::InsertNode {
            path: paragraph_path,
            node: Node::paragraph(""),
        },
    ])
    .selection_after(Selection::collapsed(Point::new(selection_path, 0)))
    .source("command:columns.insert"))
}

fn unwrap_nearest_columns(editor: &mut crate::core::Editor) -> Result<Transaction, String> {
    let sel = editor.selection().clone();
    let Some(columns_path) = ancestor_element_path(editor.doc(), &sel.focus.path, "columns") else {
        return Ok(Transaction::new(Vec::new()).source("command:columns.unwrap"));
    };
    let (columns_ix, parent_path) = columns_path
        .split_last()
        .ok_or_else(|| "Invalid columns path".to_string())?;
    let columns_ix = *columns_ix;
    let parent_path = parent_path.to_vec();

    let Some(Node::Element(columns_el)) = node_at_path(editor.doc(), &columns_path).cloned() else {
        return Err("Columns node not found".into());
    };
    if columns_el.kind != "columns" {
        return Ok(Transaction::new(Vec::new()).source("command:columns.unwrap"));
    }

    let mut flat_blocks: Vec<Node> = Vec::new();
    let mut counts: Vec<usize> = Vec::new();

    for child in columns_el.children.into_iter() {
        match child {
            Node::Element(col_el) if col_el.kind == "column" => {
                let count = col_el.children.len().max(1);
                counts.push(count);
                if col_el.children.is_empty() {
                    flat_blocks.push(Node::paragraph(""));
                } else {
                    flat_blocks.extend(col_el.children);
                }
            }
            other => {
                counts.push(1);
                flat_blocks.push(other);
            }
        }
    }

    if flat_blocks.is_empty() {
        counts = vec![1];
        flat_blocks.push(Node::paragraph(""));
    }

    let mut ops: Vec<Op> = Vec::new();
    ops.push(Op::RemoveNode {
        path: columns_path.clone(),
    });
    for (i, node) in flat_blocks.into_iter().enumerate() {
        let mut path = parent_path.clone();
        path.push(columns_ix + i);
        ops.push(Op::InsertNode { path, node });
    }

    let remap_point = |point: &Point| -> Point {
        if point.path.len() < columns_path.len() + 2 {
            return point.clone();
        }
        if !point.path.starts_with(&columns_path) {
            return point.clone();
        }
        let col_ix = point.path[columns_path.len()];
        let block_ix = point.path[columns_path.len() + 1];

        let mut flat_ix = 0usize;
        for i in 0..col_ix.min(counts.len()) {
            flat_ix += counts.get(i).copied().unwrap_or(0);
        }

        let within = counts.get(col_ix).copied().unwrap_or(1);
        flat_ix += block_ix.min(within.saturating_sub(1));

        let mut new_path = parent_path.clone();
        new_path.push(columns_ix + flat_ix);
        new_path.extend_from_slice(&point.path[columns_path.len() + 2..]);
        Point {
            path: new_path,
            offset: point.offset,
        }
    };

    let selection_after = Selection {
        anchor: remap_point(&sel.anchor),
        focus: remap_point(&sel.focus),
    };

    Ok(Transaction::new(ops)
        .selection_after(selection_after)
        .source("command:columns.unwrap"))
}

fn set_columns_widths(
    editor: &crate::core::Editor,
    columns_path: &[usize],
    widths: Vec<f64>,
) -> Result<Transaction, String> {
    let Some(Node::Element(columns_el)) = node_at_path(editor.doc(), columns_path) else {
        return Err("Columns node not found".into());
    };
    if columns_el.kind != "columns" {
        return Err("Target is not columns".into());
    }

    let col_count = columns_el.children.len().max(2);
    if widths.len() != col_count {
        return Err(format!(
            "Width count mismatch: got {}, expected {col_count}",
            widths.len()
        ));
    }

    let mut widths = widths
        .into_iter()
        .map(|w| w.max(0.05))
        .collect::<Vec<f64>>();
    let sum: f64 = widths.iter().sum();
    if !sum.is_finite() || sum <= 0.0 {
        return Err("Invalid widths sum".into());
    }
    for w in &mut widths {
        *w /= sum;
    }

    let widths_json = widths
        .into_iter()
        .filter_map(|w| serde_json::Number::from_f64(w).map(Value::Number))
        .collect::<Vec<Value>>();

    let mut set = Attrs::default();
    set.insert("widths".to_string(), Value::Array(widths_json));
    let patch = crate::core::AttrPatch {
        set,
        remove: Vec::new(),
    };

    Ok(Transaction::new(vec![Op::SetNodeAttrs {
        path: columns_path.to_vec(),
        patch,
    }])
    .source("command:columns.set_widths"))
}

fn insert_table(
    editor: &crate::core::Editor,
    rows: usize,
    cols: usize,
) -> Result<Transaction, String> {
    let focus = editor.selection().focus.clone();
    let block_path = focus.path.split_last().map(|(_, p)| p).unwrap_or(&[]);

    let (parent_path, insert_at) = if block_path.is_empty() {
        (Vec::new(), editor.doc().children.len())
    } else {
        let (block_ix, parent) = block_path.split_last().unwrap();
        (parent.to_vec(), block_ix + 1)
    };

    let table_path = {
        let mut path = parent_path.clone();
        path.push(insert_at);
        path
    };
    let paragraph_path = {
        let mut path = parent_path.clone();
        path.push(insert_at + 1);
        path
    };

    let mut selection_path = table_path.clone();
    selection_path.extend([0, 0, 0, 0]);

    Ok(Transaction::new(vec![
        Op::InsertNode {
            path: table_path,
            node: table_node(rows, cols),
        },
        Op::InsertNode {
            path: paragraph_path,
            node: Node::paragraph(""),
        },
    ])
    .selection_after(Selection::collapsed(Point::new(selection_path, 0)))
    .source("command:table.insert"))
}

fn insert_table_row_above(editor: &crate::core::Editor) -> Result<Transaction, String> {
    let focus_path = &editor.selection().focus.path;
    let row_path =
        ancestor_element_path(editor.doc(), focus_path, "table_row").ok_or("Not in a table")?;
    let cell_path = ancestor_element_path(editor.doc(), focus_path, "table_cell");
    let (row_ix, table_path) = row_path
        .split_last()
        .ok_or_else(|| "Invalid table row path".to_string())?;

    let Some(Node::Element(row)) = node_at_path(editor.doc(), &row_path) else {
        return Err("Invalid table row".into());
    };
    let cols = row.children.len().max(1);
    let target_col_ix = cell_path
        .as_ref()
        .and_then(|p| p.split_last().map(|(ix, _)| *ix))
        .unwrap_or(0)
        .min(cols.saturating_sub(1));

    let mut insert_path = table_path.to_vec();
    insert_path.push(*row_ix);

    let mut selection_path = insert_path.clone();
    selection_path.push(target_col_ix);
    selection_path.extend([0, 0]);

    Ok(Transaction::new(vec![Op::InsertNode {
        path: insert_path,
        node: table_row_node(cols),
    }])
    .selection_after(Selection::collapsed(Point::new(selection_path, 0)))
    .source("command:table.insert_row_above"))
}

fn insert_table_row_below(editor: &crate::core::Editor) -> Result<Transaction, String> {
    let focus_path = &editor.selection().focus.path;
    let row_path =
        ancestor_element_path(editor.doc(), focus_path, "table_row").ok_or("Not in a table")?;
    let (row_ix, table_path) = row_path
        .split_last()
        .ok_or_else(|| "Invalid table row path".to_string())?;

    let Some(Node::Element(row)) = node_at_path(editor.doc(), &row_path) else {
        return Err("Invalid table row".into());
    };
    let cols = row.children.len().max(1);

    let mut insert_path = table_path.to_vec();
    insert_path.push(row_ix + 1);

    let mut selection_path = insert_path.clone();
    selection_path.extend([0, 0, 0]);

    Ok(Transaction::new(vec![Op::InsertNode {
        path: insert_path,
        node: table_row_node(cols),
    }])
    .selection_after(Selection::collapsed(Point::new(selection_path, 0)))
    .source("command:table.insert_row_below"))
}

fn insert_table_col_left(editor: &crate::core::Editor) -> Result<Transaction, String> {
    let focus_path = &editor.selection().focus.path;
    let cell_path =
        ancestor_element_path(editor.doc(), focus_path, "table_cell").ok_or("Not in a table")?;
    let (cell_ix, row_path) = cell_path
        .split_last()
        .ok_or_else(|| "Invalid table cell path".to_string())?;
    let cell_ix = *cell_ix;
    let (row_ix, table_path) = row_path
        .split_last()
        .ok_or_else(|| "Invalid table row path".to_string())?;

    let Some(Node::Element(table)) = node_at_path(editor.doc(), table_path) else {
        return Err("Invalid table".into());
    };
    if table.kind != "table" {
        return Err("Invalid table".into());
    }

    let insert_ix_in_current_row = {
        let Some(Node::Element(row)) = node_at_path(editor.doc(), row_path) else {
            return Err("Invalid table row".into());
        };
        cell_ix.min(row.children.len())
    };

    let mut ops: Vec<Op> = Vec::new();
    for (r_ix, row_node) in table.children.iter().enumerate() {
        let Node::Element(row) = row_node else {
            continue;
        };
        if row.kind != "table_row" {
            continue;
        }
        let insert_ix = cell_ix.min(row.children.len());
        let mut insert_path = table_path.to_vec();
        insert_path.push(r_ix);
        insert_path.push(insert_ix);
        ops.push(Op::InsertNode {
            path: insert_path,
            node: table_cell_node(),
        });
    }

    let mut selection_path = table_path.to_vec();
    selection_path.push(*row_ix);
    selection_path.push(insert_ix_in_current_row);
    selection_path.extend([0, 0]);

    Ok(Transaction::new(ops)
        .selection_after(Selection::collapsed(Point::new(selection_path, 0)))
        .source("command:table.insert_col_left"))
}

fn insert_table_col_right(editor: &crate::core::Editor) -> Result<Transaction, String> {
    let focus_path = &editor.selection().focus.path;
    let cell_path =
        ancestor_element_path(editor.doc(), focus_path, "table_cell").ok_or("Not in a table")?;
    let (cell_ix, row_path) = cell_path
        .split_last()
        .ok_or_else(|| "Invalid table cell path".to_string())?;
    let (row_ix, table_path) = row_path
        .split_last()
        .ok_or_else(|| "Invalid table row path".to_string())?;

    let Some(Node::Element(table)) = node_at_path(editor.doc(), table_path) else {
        return Err("Invalid table".into());
    };
    if table.kind != "table" {
        return Err("Invalid table".into());
    }

    let insert_ix_in_current_row = {
        let Some(Node::Element(row)) = node_at_path(editor.doc(), row_path) else {
            return Err("Invalid table row".into());
        };
        (cell_ix + 1).min(row.children.len())
    };

    let mut ops: Vec<Op> = Vec::new();
    for (r_ix, row_node) in table.children.iter().enumerate() {
        let Node::Element(row) = row_node else {
            continue;
        };
        if row.kind != "table_row" {
            continue;
        }
        let insert_ix = (cell_ix + 1).min(row.children.len());
        let mut insert_path = table_path.to_vec();
        insert_path.push(r_ix);
        insert_path.push(insert_ix);
        ops.push(Op::InsertNode {
            path: insert_path,
            node: table_cell_node(),
        });
    }

    let mut selection_path = table_path.to_vec();
    selection_path.push(*row_ix);
    selection_path.push(insert_ix_in_current_row);
    selection_path.extend([0, 0]);

    Ok(Transaction::new(ops)
        .selection_after(Selection::collapsed(Point::new(selection_path, 0)))
        .source("command:table.insert_col_right"))
}

fn delete_table(editor: &crate::core::Editor) -> Result<Transaction, String> {
    let focus_path = &editor.selection().focus.path;
    let table_path =
        ancestor_element_path(editor.doc(), focus_path, "table").ok_or("Not in a table")?;

    let mut paragraph_text_path = table_path.clone();
    paragraph_text_path.push(0);

    Ok(Transaction::new(vec![
        Op::RemoveNode {
            path: table_path.clone(),
        },
        Op::InsertNode {
            path: table_path,
            node: Node::paragraph(""),
        },
    ])
    .selection_after(Selection::collapsed(Point::new(paragraph_text_path, 0)))
    .source("command:table.delete_table"))
}

fn delete_table_row(editor: &crate::core::Editor) -> Result<Transaction, String> {
    let focus_path = &editor.selection().focus.path;
    let row_path =
        ancestor_element_path(editor.doc(), focus_path, "table_row").ok_or("Not in a table")?;
    let cell_path = ancestor_element_path(editor.doc(), focus_path, "table_cell");

    let (row_ix, table_path) = row_path
        .split_last()
        .ok_or_else(|| "Invalid table row path".to_string())?;
    let cell_ix = cell_path
        .as_ref()
        .and_then(|p| p.split_last().map(|(ix, _)| *ix))
        .unwrap_or(0);

    let Some(Node::Element(table)) = node_at_path(editor.doc(), table_path) else {
        return Err("Invalid table".into());
    };
    if table.kind != "table" {
        return Err("Invalid table".into());
    }

    let row_count = table
        .children
        .iter()
        .filter(|n| matches!(n, Node::Element(el) if el.kind == "table_row"))
        .count();

    if row_count <= 1 {
        let paragraph_text_path = {
            let mut path = row_path.clone();
            path.truncate(table_path.len());
            path.push(0);
            path
        };
        return Ok(Transaction::new(vec![
            Op::RemoveNode {
                path: table_path.to_vec(),
            },
            Op::InsertNode {
                path: table_path.to_vec(),
                node: Node::paragraph(""),
            },
        ])
        .selection_after(Selection::collapsed(Point::new(paragraph_text_path, 0)))
        .source("command:table.delete_row"));
    }

    let target_row_ix = if *row_ix < row_count.saturating_sub(1) {
        *row_ix
    } else {
        row_ix.saturating_sub(1)
    };
    let target_row_in_old_doc = if *row_ix < row_count.saturating_sub(1) {
        row_ix + 1
    } else {
        target_row_ix
    };

    let target_cols = node_at_path(
        editor.doc(),
        &[table_path, &[target_row_in_old_doc]].concat(),
    )
    .and_then(|n| match n {
        Node::Element(el) if el.kind == "table_row" => Some(el.children.len().max(1)),
        _ => None,
    })
    .unwrap_or(1);
    let target_col_ix = cell_ix.min(target_cols.saturating_sub(1));

    let mut selection_path = table_path.to_vec();
    selection_path.push(target_row_ix);
    selection_path.push(target_col_ix);
    selection_path.extend([0, 0]);

    Ok(Transaction::new(vec![Op::RemoveNode { path: row_path }])
        .selection_after(Selection::collapsed(Point::new(selection_path, 0)))
        .source("command:table.delete_row"))
}

fn delete_table_col(editor: &crate::core::Editor) -> Result<Transaction, String> {
    let focus_path = &editor.selection().focus.path;
    let cell_path =
        ancestor_element_path(editor.doc(), focus_path, "table_cell").ok_or("Not in a table")?;
    let (cell_ix, row_path) = cell_path
        .split_last()
        .ok_or_else(|| "Invalid table cell path".to_string())?;
    let (row_ix, table_path) = row_path
        .split_last()
        .ok_or_else(|| "Invalid table row path".to_string())?;

    let Some(Node::Element(table)) = node_at_path(editor.doc(), table_path) else {
        return Err("Invalid table".into());
    };
    if table.kind != "table" {
        return Err("Invalid table".into());
    }

    let first_row_cols = table
        .children
        .iter()
        .find_map(|n| match n {
            Node::Element(el) if el.kind == "table_row" => Some(el.children.len().max(1)),
            _ => None,
        })
        .unwrap_or(1);

    if first_row_cols <= 1 {
        let mut paragraph_text_path = table_path.to_vec();
        paragraph_text_path.push(0);
        return Ok(Transaction::new(vec![
            Op::RemoveNode {
                path: table_path.to_vec(),
            },
            Op::InsertNode {
                path: table_path.to_vec(),
                node: Node::paragraph(""),
            },
        ])
        .selection_after(Selection::collapsed(Point::new(paragraph_text_path, 0)))
        .source("command:table.delete_col"));
    }

    let target_col_ix = if *cell_ix < first_row_cols.saturating_sub(1) {
        *cell_ix
    } else {
        cell_ix.saturating_sub(1)
    };

    let mut ops: Vec<Op> = Vec::new();
    for (r_ix, row_node) in table.children.iter().enumerate() {
        let Node::Element(row) = row_node else {
            continue;
        };
        if row.kind != "table_row" {
            continue;
        }
        if *cell_ix >= row.children.len() {
            continue;
        }
        let mut remove_path = table_path.to_vec();
        remove_path.push(r_ix);
        remove_path.push(*cell_ix);
        ops.push(Op::RemoveNode { path: remove_path });
    }

    let mut selection_path = table_path.to_vec();
    selection_path.push(*row_ix);
    selection_path.push(target_col_ix);
    selection_path.extend([0, 0]);

    Ok(Transaction::new(ops)
        .selection_after(Selection::collapsed(Point::new(selection_path, 0)))
        .source("command:table.delete_col"))
}

fn clamp_to_char_boundary(s: &str, mut ix: usize) -> usize {
    ix = ix.min(s.len());
    while ix > 0 && !s.is_char_boundary(ix) {
        ix -= 1;
    }
    ix
}

fn point_global_offset(children: &[Node], child_ix: usize, offset: usize) -> usize {
    let mut global = 0usize;
    for (ix, node) in children.iter().enumerate() {
        match node {
            Node::Text(t) => {
                if ix < child_ix {
                    global += t.text.len();
                    continue;
                }
                if ix == child_ix {
                    let o = clamp_to_char_boundary(&t.text, offset);
                    global += o;
                }
                break;
            }
            Node::Void(v) => {
                if ix < child_ix {
                    global += v.inline_text_len();
                    continue;
                }
                if ix == child_ix {
                    global += offset.min(v.inline_text_len());
                }
                break;
            }
            Node::Element(_) => {}
        }
    }
    global
}

fn point_for_global_offset(block_path: &[usize], children: &[Node], global_offset: usize) -> Point {
    let mut remaining = global_offset;
    for (child_ix, node) in children.iter().enumerate() {
        match node {
            Node::Text(t) => {
                if remaining < t.text.len() {
                    let mut path = block_path.to_vec();
                    path.push(child_ix);
                    return Point::new(path, clamp_to_char_boundary(&t.text, remaining));
                }
                if remaining == t.text.len() {
                    if matches!(children.get(child_ix + 1), Some(Node::Text(_))) {
                        let mut path = block_path.to_vec();
                        path.push(child_ix + 1);
                        return Point::new(path, 0);
                    }
                    let mut path = block_path.to_vec();
                    path.push(child_ix);
                    return Point::new(path, t.text.len());
                }
                remaining = remaining.saturating_sub(t.text.len());
            }
            Node::Void(v) => {
                let len = v.inline_text_len();
                if remaining <= len {
                    let before = remaining;
                    let after = len - remaining;

                    if remaining == 0 || before <= after {
                        for (ix, prev) in children.iter().enumerate().take(child_ix).rev() {
                            if let Node::Text(t) = prev {
                                let mut path = block_path.to_vec();
                                path.push(ix);
                                return Point::new(path, t.text.len());
                            }
                        }
                    }

                    for (ix, next) in children.iter().enumerate().skip(child_ix + 1) {
                        if matches!(next, Node::Text(_)) {
                            let mut path = block_path.to_vec();
                            path.push(ix);
                            return Point::new(path, 0);
                        }
                    }
                    break;
                }
                remaining = remaining.saturating_sub(len);
            }
            Node::Element(_) => {}
        }
    }

    // Fallback to end of last text node.
    for (child_ix, node) in children.iter().enumerate().rev() {
        if let Node::Text(t) = node {
            let mut path = block_path.to_vec();
            path.push(child_ix);
            return Point::new(path, t.text.len());
        }
    }

    let mut path = block_path.to_vec();
    path.push(0);
    Point::new(path, 0)
}

fn is_point_in_block(point: &Point, block_path: &[usize]) -> bool {
    point.path.len() == block_path.len() + 1 && point.path.starts_with(block_path)
}

struct TextBlock<'a> {
    path: Path,
    el: &'a ElementNode,
}

fn element_is_text_block(el: &ElementNode, registry: &PluginRegistry) -> bool {
    match registry
        .node_specs
        .get(&el.kind)
        .map(|s| s.children.clone())
    {
        Some(ChildConstraint::InlineOnly) => true,
        Some(_) => false,
        None => el
            .children
            .iter()
            .any(|n| matches!(n, Node::Text(_) | Node::Void(_))),
    }
}

fn text_blocks_in_order<'a>(doc: &'a Document, registry: &PluginRegistry) -> Vec<TextBlock<'a>> {
    fn walk<'a>(
        nodes: &'a [Node],
        path: &mut Vec<usize>,
        registry: &PluginRegistry,
        out: &mut Vec<TextBlock<'a>>,
    ) {
        for (ix, node) in nodes.iter().enumerate() {
            let Node::Element(el) = node else {
                continue;
            };

            path.push(ix);

            if element_is_text_block(el, registry) {
                out.push(TextBlock {
                    path: path.clone(),
                    el,
                });
            } else {
                walk(&el.children, path, registry, out);
            }

            path.pop();
        }
    }

    let mut out = Vec::new();
    walk(&doc.children, &mut Vec::new(), registry, &mut out);
    out
}

fn total_inline_text_len(children: &[Node]) -> usize {
    children
        .iter()
        .map(|n| match n {
            Node::Text(t) => t.text.len(),
            Node::Void(v) => v.inline_text_len(),
            Node::Element(_) => 0,
        })
        .sum()
}

fn apply_marks_in_block(
    children: &[Node],
    start_global: usize,
    end_global: usize,
    apply: &dyn Fn(Marks) -> Marks,
) -> Vec<Node> {
    if start_global >= end_global {
        return children.to_vec();
    }

    let mut out: Vec<Node> = Vec::new();
    let mut cursor = 0usize;

    for node in children {
        let (node_start, node_end) = match node {
            Node::Text(t) => {
                let start = cursor;
                let end = cursor + t.text.len();
                cursor = end;
                (start, end)
            }
            Node::Void(v) => {
                cursor += v.inline_text_len();
                out.push(node.clone());
                continue;
            }
            Node::Element(_) => {
                out.push(node.clone());
                continue;
            }
        };

        if end_global <= node_start || start_global >= node_end {
            out.push(node.clone());
            continue;
        }

        let Node::Text(t) = node else {
            out.push(node.clone());
            continue;
        };

        let sel_start = (start_global.saturating_sub(node_start)).min(t.text.len());
        let sel_end = (end_global.saturating_sub(node_start)).min(t.text.len());

        let sel_start = clamp_to_char_boundary(&t.text, sel_start);
        let sel_end = clamp_to_char_boundary(&t.text, sel_end);

        if sel_start == 0 && sel_end == t.text.len() {
            let mut next = t.clone();
            next.marks = apply(next.marks);
            out.push(Node::Text(next));
            continue;
        }

        let prefix = t.text.get(..sel_start).unwrap_or("").to_string();
        let middle = t.text.get(sel_start..sel_end).unwrap_or("").to_string();
        let suffix = t.text.get(sel_end..).unwrap_or("").to_string();

        if !prefix.is_empty() {
            out.push(Node::Text(TextNode {
                text: prefix,
                marks: t.marks.clone(),
            }));
        }
        if !middle.is_empty() {
            out.push(Node::Text(TextNode {
                text: middle,
                marks: apply(t.marks.clone()),
            }));
        }
        if !suffix.is_empty() {
            out.push(Node::Text(TextNode {
                text: suffix,
                marks: t.marks.clone(),
            }));
        }
    }

    if out.is_empty() {
        out.push(Node::Text(TextNode {
            text: String::new(),
            marks: Marks::default(),
        }));
    }

    out
}

fn ordered_selection_points(sel: &Selection) -> (Point, Point) {
    let mut start = sel.anchor.clone();
    let mut end = sel.focus.clone();

    if start.path == end.path {
        if end.offset < start.offset {
            std::mem::swap(&mut start, &mut end);
        }
        return (start, end);
    }
    if end.path < start.path {
        std::mem::swap(&mut start, &mut end);
    }
    (start, end)
}

fn active_marks(editor: &crate::core::Editor) -> Marks {
    let focus = &editor.selection().focus;
    match node_at_path(editor.doc(), &focus.path) {
        Some(Node::Text(text)) => text.marks.clone(),
        _ => Marks::default(),
    }
}

fn all_selected_text_nodes_have_mark(
    editor: &crate::core::Editor,
    sel: &Selection,
    get: fn(&Marks) -> bool,
) -> Result<bool, String> {
    let (start, end) = ordered_selection_points(sel);
    let Some(start_block_path) = start.path.split_last().map(|(_, p)| p.to_vec()) else {
        return Err("Selection start is not in a text block".into());
    };
    let Some(end_block_path) = end.path.split_last().map(|(_, p)| p.to_vec()) else {
        return Err("Selection end is not in a text block".into());
    };

    let blocks = text_blocks_in_order(editor.doc(), editor.registry());
    let start_index = blocks
        .iter()
        .position(|b| b.path == start_block_path)
        .ok_or_else(|| "Selection start is not in a text block".to_string())?;
    let end_index = blocks
        .iter()
        .position(|b| b.path == end_block_path)
        .ok_or_else(|| "Selection end is not in a text block".to_string())?;

    let (start_index, end_index) = if start_index <= end_index {
        (start_index, end_index)
    } else {
        (end_index, start_index)
    };

    let start_inline_ix = start.path.last().copied().unwrap_or(0);
    let end_inline_ix = end.path.last().copied().unwrap_or(0);

    for (block_index, block) in blocks
        .iter()
        .enumerate()
        .take(end_index + 1)
        .skip(start_index)
    {
        let children = block.el.children.as_slice();
        let total_len = total_inline_text_len(children);
        if total_len == 0 {
            continue;
        }

        let start_global = if block_index == start_index {
            point_global_offset(children, start_inline_ix, start.offset)
        } else {
            0
        };
        let end_global = if block_index == end_index {
            point_global_offset(children, end_inline_ix, end.offset)
        } else {
            total_len
        };
        if start_global >= end_global {
            continue;
        }

        let mut cursor = 0usize;
        for node in children {
            let (node_start, node_end) = match node {
                Node::Text(t) => {
                    let start = cursor;
                    let end = cursor + t.text.len();
                    cursor = end;
                    (start, end)
                }
                Node::Void(v) => {
                    let start = cursor;
                    let end = cursor + v.inline_text_len();
                    cursor = end;
                    (start, end)
                }
                Node::Element(_) => {
                    continue;
                }
            };
            if end_global <= node_start || start_global >= node_end {
                continue;
            }
            if let Node::Text(t) = node {
                if !get(&t.marks) {
                    return Ok(false);
                }
            }
        }
    }

    Ok(true)
}

fn toggle_bool_mark(
    editor: &mut crate::core::Editor,
    get: fn(&Marks) -> bool,
    set: fn(&mut Marks, bool),
    source: &'static str,
) -> Result<Transaction, String> {
    let sel = editor.selection().clone();
    if sel.is_collapsed() {
        return toggle_mark_at_caret(editor, |mut marks| {
            let target = !get(&marks);
            set(&mut marks, target);
            marks
        })
        .map(|(ops, selection_after)| {
            Transaction::new(ops)
                .selection_after(selection_after)
                .source(source)
        });
    }

    let all_set = all_selected_text_nodes_have_mark(editor, &sel, get)?;
    let target = !all_set;
    apply_mark_range(editor, &sel, &|mut marks: Marks| {
        set(&mut marks, target);
        marks
    })
    .map(|(ops, selection_after)| {
        Transaction::new(ops)
            .selection_after(selection_after)
            .source(source)
    })
}

fn set_optional_string_mark(
    editor: &mut crate::core::Editor,
    set: fn(&mut Marks, Option<String>),
    value: Option<String>,
    source: &'static str,
) -> Result<Transaction, String> {
    let sel = editor.selection().clone();
    if sel.is_collapsed() {
        return toggle_mark_at_caret(editor, |mut marks| {
            set(&mut marks, value.clone());
            marks
        })
        .map(|(ops, selection_after)| {
            Transaction::new(ops)
                .selection_after(selection_after)
                .source(source)
        });
    }

    apply_mark_range(editor, &sel, &|mut marks: Marks| {
        set(&mut marks, value.clone());
        marks
    })
    .map(|(ops, selection_after)| {
        Transaction::new(ops)
            .selection_after(selection_after)
            .source(source)
    })
}

fn toggle_bold(editor: &mut crate::core::Editor) -> Result<Transaction, String> {
    toggle_bool_mark(
        editor,
        |m| m.bold,
        |m, v| m.bold = v,
        "command:marks.toggle_bold",
    )
}

fn toggle_italic(editor: &mut crate::core::Editor) -> Result<Transaction, String> {
    toggle_bool_mark(
        editor,
        |m| m.italic,
        |m, v| m.italic = v,
        "command:marks.toggle_italic",
    )
}

fn toggle_underline(editor: &mut crate::core::Editor) -> Result<Transaction, String> {
    toggle_bool_mark(
        editor,
        |m| m.underline,
        |m, v| m.underline = v,
        "command:marks.toggle_underline",
    )
}

fn toggle_strikethrough(editor: &mut crate::core::Editor) -> Result<Transaction, String> {
    toggle_bool_mark(
        editor,
        |m| m.strikethrough,
        |m, v| m.strikethrough = v,
        "command:marks.toggle_strikethrough",
    )
}

fn toggle_code(editor: &mut crate::core::Editor) -> Result<Transaction, String> {
    toggle_bool_mark(
        editor,
        |m| m.code,
        |m, v| m.code = v,
        "command:marks.toggle_code",
    )
}

fn set_text_color(editor: &mut crate::core::Editor, color: String) -> Result<Transaction, String> {
    set_optional_string_mark(
        editor,
        |m, v| m.text_color = v,
        Some(color),
        "command:marks.set_text_color",
    )
}

fn unset_text_color(editor: &mut crate::core::Editor) -> Result<Transaction, String> {
    set_optional_string_mark(
        editor,
        |m, v| m.text_color = v,
        None,
        "command:marks.unset_text_color",
    )
}

fn set_highlight_color(
    editor: &mut crate::core::Editor,
    color: String,
) -> Result<Transaction, String> {
    set_optional_string_mark(
        editor,
        |m, v| m.highlight_color = v,
        Some(color),
        "command:marks.set_highlight_color",
    )
}

fn unset_highlight_color(editor: &mut crate::core::Editor) -> Result<Transaction, String> {
    set_optional_string_mark(
        editor,
        |m, v| m.highlight_color = v,
        None,
        "command:marks.unset_highlight_color",
    )
}

fn set_link(editor: &mut crate::core::Editor, url: String) -> Result<Transaction, String> {
    let sel = editor.selection().clone();
    if sel.is_collapsed() {
        return toggle_mark_at_caret(editor, |mut marks| {
            marks.link = Some(url.clone());
            marks
        })
        .map(|(ops, selection_after)| {
            Transaction::new(ops)
                .selection_after(selection_after)
                .source("command:marks.set_link")
        });
    }

    apply_mark_range(editor, &sel, &|mut marks: Marks| {
        marks.link = Some(url.clone());
        marks
    })
    .map(|(ops, selection_after)| {
        Transaction::new(ops)
            .selection_after(selection_after)
            .source("command:marks.set_link")
    })
}

fn unset_link(editor: &mut crate::core::Editor) -> Result<Transaction, String> {
    let sel = editor.selection().clone();
    if sel.is_collapsed() {
        return toggle_mark_at_caret(editor, |mut marks| {
            marks.link = None;
            marks
        })
        .map(|(ops, selection_after)| {
            Transaction::new(ops)
                .selection_after(selection_after)
                .source("command:marks.unset_link")
        });
    }

    apply_mark_range(editor, &sel, &|mut marks: Marks| {
        marks.link = None;
        marks
    })
    .map(|(ops, selection_after)| {
        Transaction::new(ops)
            .selection_after(selection_after)
            .source("command:marks.unset_link")
    })
}

fn toggle_mark_at_caret(
    editor: &crate::core::Editor,
    apply: impl Fn(Marks) -> Marks,
) -> Result<(Vec<Op>, Selection), String> {
    let focus = editor.selection().focus.clone();
    if focus.path.is_empty() {
        return Err("Selection is not in a text node".into());
    }
    let (child_ix, block_path) = focus
        .path
        .split_last()
        .ok_or_else(|| "Selection is not in a text node".to_string())?;

    let Some(Node::Element(el)) = node_at_path(editor.doc(), block_path) else {
        return Err("Selection is not in a text block".into());
    };
    let Some(Node::Text(text)) = el.children.get(*child_ix) else {
        return Err("Selection is not in a text node".into());
    };

    let cursor = clamp_to_char_boundary(&text.text, focus.offset);
    let marks_before = text.marks.clone();
    let marks_after = apply(marks_before.clone());

    if text.text.is_empty() {
        let selection_after = Selection::collapsed(Point::new(focus.path.clone(), 0));
        return Ok((
            vec![Op::SetTextMarks {
                path: focus.path.clone(),
                marks: marks_after,
            }],
            selection_after,
        ));
    }

    let mut replacement: Vec<Node> = Vec::new();
    let base_child_ix = *child_ix;
    let mut caret_child_ix = base_child_ix;

    let left = text.text.get(..cursor).unwrap_or("").to_string();
    let right = text.text.get(cursor..).unwrap_or("").to_string();

    if !left.is_empty() {
        replacement.push(Node::Text(TextNode {
            text: left,
            marks: marks_before.clone(),
        }));
        caret_child_ix += 1;
    }

    replacement.push(Node::Text(TextNode {
        text: String::new(),
        marks: marks_after,
    }));

    if !right.is_empty() {
        replacement.push(Node::Text(TextNode {
            text: right,
            marks: marks_before,
        }));
    }

    let mut ops: Vec<Op> = Vec::new();
    ops.push(Op::RemoveNode {
        path: focus.path.clone(),
    });
    for (i, node) in replacement.into_iter().enumerate() {
        let mut path = block_path.to_vec();
        path.push(base_child_ix + i);
        ops.push(Op::InsertNode { path, node });
    }

    let mut caret_path = block_path.to_vec();
    caret_path.push(caret_child_ix);
    let selection_after = Selection::collapsed(Point::new(caret_path, 0));
    Ok((ops, selection_after))
}

fn insert_emoji(editor: &crate::core::Editor, emoji: String) -> Result<Transaction, String> {
    let sel = editor.selection().clone();
    if !sel.is_collapsed() {
        return Err("Selection must be collapsed".into());
    }

    let focus = sel.focus;
    if focus.path.is_empty() {
        return Err("Selection is not in a text node".into());
    }
    let (child_ix, block_path) = focus
        .path
        .split_last()
        .ok_or_else(|| "Selection is not in a text node".to_string())?;

    let Some(Node::Element(el)) = node_at_path(editor.doc(), block_path) else {
        return Err("Selection is not in a text block".into());
    };
    let Some(Node::Text(text)) = el.children.get(*child_ix) else {
        return Err("Selection is not in a text node".into());
    };

    let cursor = clamp_to_char_boundary(&text.text, focus.offset);
    let left = text.text.get(..cursor).unwrap_or("").to_string();
    let right = text.text.get(cursor..).unwrap_or("").to_string();
    let marks = text.marks.clone();

    let mut replacement: Vec<Node> = Vec::new();
    let base_child_ix = *child_ix;
    let mut emoji_ix = base_child_ix;

    if !left.is_empty() {
        replacement.push(Node::Text(TextNode {
            text: left,
            marks: marks.clone(),
        }));
        emoji_ix += 1;
    }

    let mut attrs = Attrs::default();
    attrs.insert("emoji".to_string(), Value::String(emoji));
    replacement.push(Node::Void(crate::core::VoidNode {
        kind: "emoji".to_string(),
        attrs,
    }));

    if right.is_empty() {
        replacement.push(Node::Text(TextNode {
            text: String::new(),
            marks: marks.clone(),
        }));
    } else {
        replacement.push(Node::Text(TextNode { text: right, marks }));
    }

    let mut ops: Vec<Op> = Vec::new();
    ops.push(Op::RemoveNode {
        path: focus.path.clone(),
    });
    for (i, node) in replacement.into_iter().enumerate() {
        let mut path = block_path.to_vec();
        path.push(base_child_ix + i);
        ops.push(Op::InsertNode { path, node });
    }

    let mut selection_path = block_path.to_vec();
    selection_path.push(emoji_ix + 1);
    let selection_after = Selection::collapsed(Point::new(selection_path, 0));
    Ok(Transaction::new(ops)
        .selection_after(selection_after)
        .source("command:emoji.insert"))
}

fn insert_mention(editor: &crate::core::Editor, label: String) -> Result<Transaction, String> {
    let sel = editor.selection().clone();
    if !sel.is_collapsed() {
        return Err("Selection must be collapsed".into());
    }

    let focus = sel.focus;
    if focus.path.is_empty() {
        return Err("Selection is not in a text node".into());
    }
    let (child_ix, block_path) = focus
        .path
        .split_last()
        .ok_or_else(|| "Selection is not in a text node".to_string())?;

    let Some(Node::Element(el)) = node_at_path(editor.doc(), block_path) else {
        return Err("Selection is not in a text block".into());
    };
    let Some(Node::Text(text)) = el.children.get(*child_ix) else {
        return Err("Selection is not in a text node".into());
    };

    let cursor = clamp_to_char_boundary(&text.text, focus.offset);
    let left = text.text.get(..cursor).unwrap_or("").to_string();
    let right = text.text.get(cursor..).unwrap_or("").to_string();
    let marks = text.marks.clone();

    let mut replacement: Vec<Node> = Vec::new();
    let base_child_ix = *child_ix;
    let mut mention_ix = base_child_ix;

    if !left.is_empty() {
        replacement.push(Node::Text(TextNode {
            text: left,
            marks: marks.clone(),
        }));
        mention_ix += 1;
    }

    let mut attrs = Attrs::default();
    attrs.insert("label".to_string(), Value::String(label));
    replacement.push(Node::Void(crate::core::VoidNode {
        kind: "mention".to_string(),
        attrs,
    }));

    if right.is_empty() {
        replacement.push(Node::Text(TextNode {
            text: String::new(),
            marks: marks.clone(),
        }));
    } else {
        replacement.push(Node::Text(TextNode { text: right, marks }));
    }

    let mut ops: Vec<Op> = Vec::new();
    ops.push(Op::RemoveNode {
        path: focus.path.clone(),
    });
    for (i, node) in replacement.into_iter().enumerate() {
        let mut path = block_path.to_vec();
        path.push(base_child_ix + i);
        ops.push(Op::InsertNode { path, node });
    }

    let mut selection_path = block_path.to_vec();
    selection_path.push(mention_ix + 1);
    let selection_after = Selection::collapsed(Point::new(selection_path, 0));
    Ok(Transaction::new(ops)
        .selection_after(selection_after)
        .source("command:mention.insert"))
}

fn insert_image(
    editor: &crate::core::Editor,
    src: String,
    alt: Option<String>,
) -> Result<Transaction, String> {
    let focus = editor.selection().focus.clone();
    let block_path = focus.path.split_last().map(|(_, p)| p).unwrap_or(&[]);

    let (parent_path, insert_at) = if block_path.is_empty() {
        (Vec::new(), editor.doc().children.len())
    } else {
        let (block_ix, parent) = block_path.split_last().unwrap();
        (parent.to_vec(), block_ix + 1)
    };

    let image_path = {
        let mut path = parent_path.clone();
        path.push(insert_at);
        path
    };
    let paragraph_element_path = {
        let mut path = parent_path.clone();
        path.push(insert_at + 1);
        path
    };
    let paragraph_text_path = {
        let mut path = paragraph_element_path.clone();
        path.push(0);
        path
    };

    Ok(Transaction::new(vec![
        Op::InsertNode {
            path: image_path,
            node: Node::image(src, alt),
        },
        Op::InsertNode {
            path: paragraph_element_path.clone(),
            node: Node::paragraph(""),
        },
    ])
    .selection_after(Selection::collapsed(Point::new(paragraph_text_path, 0)))
    .source("command:image.insert"))
}

fn insert_images(
    editor: &crate::core::Editor,
    images: Vec<(String, Option<String>)>,
) -> Result<Transaction, String> {
    if images.is_empty() {
        return Err("No images to insert".into());
    }

    let focus = editor.selection().focus.clone();
    let block_path = focus.path.split_last().map(|(_, p)| p).unwrap_or(&[]);

    let (parent_path, insert_at) = if block_path.is_empty() {
        (Vec::new(), editor.doc().children.len())
    } else {
        let (block_ix, parent) = block_path.split_last().unwrap();
        (parent.to_vec(), block_ix + 1)
    };

    let mut ops: Vec<Op> = Vec::new();
    for (i, (src, alt)) in images.into_iter().enumerate() {
        let mut path = parent_path.clone();
        path.push(insert_at + i);
        ops.push(Op::InsertNode {
            path,
            node: Node::image(src, alt),
        });
    }

    let mut paragraph_element_path = parent_path.clone();
    paragraph_element_path.push(insert_at + ops.len());
    let mut paragraph_text_path = paragraph_element_path.clone();
    paragraph_text_path.push(0);
    ops.push(Op::InsertNode {
        path: paragraph_element_path,
        node: Node::paragraph(""),
    });

    Ok(Transaction::new(ops)
        .selection_after(Selection::collapsed(Point::new(paragraph_text_path, 0)))
        .source("command:image.insert_many"))
}

fn apply_mark_range(
    editor: &crate::core::Editor,
    sel: &Selection,
    apply: &dyn Fn(Marks) -> Marks,
) -> Result<(Vec<Op>, Selection), String> {
    let (start, end) = ordered_selection_points(sel);

    let Some(start_block_path) = start.path.split_last().map(|(_, p)| p.to_vec()) else {
        return Err("Selection start is not in a text block".into());
    };
    let Some(end_block_path) = end.path.split_last().map(|(_, p)| p.to_vec()) else {
        return Err("Selection end is not in a text block".into());
    };

    let blocks = text_blocks_in_order(editor.doc(), editor.registry());
    let start_index = blocks
        .iter()
        .position(|b| b.path == start_block_path)
        .ok_or_else(|| "Selection start is not in a text block".to_string())?;
    let end_index = blocks
        .iter()
        .position(|b| b.path == end_block_path)
        .ok_or_else(|| "Selection end is not in a text block".to_string())?;

    let (start_index, end_index) = if start_index <= end_index {
        (start_index, end_index)
    } else {
        (end_index, start_index)
    };

    let start_inline_ix = start.path.last().copied().unwrap_or(0);
    let end_inline_ix = end.path.last().copied().unwrap_or(0);

    let mut ops: Vec<Op> = Vec::new();
    let mut new_anchor = sel.anchor.clone();
    let mut new_focus = sel.focus.clone();

    for (block_index, block) in blocks
        .iter()
        .enumerate()
        .take(end_index + 1)
        .skip(start_index)
    {
        let children = block.el.children.as_slice();
        let total_len = total_inline_text_len(children);
        if total_len == 0 {
            continue;
        }

        let start_global = if block_index == start_index {
            point_global_offset(children, start_inline_ix, start.offset)
        } else {
            0
        };
        let end_global = if block_index == end_index {
            point_global_offset(children, end_inline_ix, end.offset)
        } else {
            total_len
        };

        if start_global >= end_global {
            continue;
        }

        let new_children = apply_marks_in_block(children, start_global, end_global, apply);

        for child_ix in (0..children.len()).rev() {
            let mut remove_path = block.path.clone();
            remove_path.push(child_ix);
            ops.push(Op::RemoveNode { path: remove_path });
        }
        for (child_ix, node) in new_children.iter().cloned().enumerate() {
            let mut insert_path = block.path.clone();
            insert_path.push(child_ix);
            ops.push(Op::InsertNode {
                path: insert_path,
                node,
            });
        }

        if is_point_in_block(&new_anchor, &block.path) {
            let global = point_global_offset(
                children,
                new_anchor.path.last().copied().unwrap_or(0),
                new_anchor.offset,
            );
            new_anchor = point_for_global_offset(&block.path, &new_children, global);
        }
        if is_point_in_block(&new_focus, &block.path) {
            let global = point_global_offset(
                children,
                new_focus.path.last().copied().unwrap_or(0),
                new_focus.offset,
            );
            new_focus = point_for_global_offset(&block.path, &new_children, global);
        }
    }

    Ok((
        ops,
        Selection {
            anchor: new_anchor,
            focus: new_focus,
        },
    ))
}
