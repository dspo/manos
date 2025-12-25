use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::ops::{Op, Path, Transaction};
use crate::plugin::{CommandError, CommandSpec, NodeSpec, NormalizePass, PluginRegistry};

pub type Attrs = BTreeMap<String, serde_json::Value>;
pub type ElementKind = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Document {
    #[serde(default)]
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "node", rename_all = "snake_case")]
pub enum Node {
    Element(ElementNode),
    Text(TextNode),
    Void(VoidNode),
}

impl Node {
    pub fn paragraph(text: impl Into<String>) -> Self {
        Node::Element(ElementNode {
            kind: "paragraph".to_string(),
            attrs: Attrs::default(),
            children: vec![Node::Text(TextNode {
                text: text.into(),
                marks: Marks::default(),
            })],
        })
    }

    pub fn divider() -> Self {
        Node::Void(VoidNode {
            kind: "divider".to_string(),
            attrs: Attrs::default(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElementNode {
    pub kind: ElementKind,
    #[serde(default)]
    pub attrs: Attrs,
    #[serde(default)]
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoidNode {
    pub kind: ElementKind,
    #[serde(default)]
    pub attrs: Attrs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextNode {
    pub text: String,
    #[serde(default)]
    pub marks: Marks,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Marks {
    #[serde(default)]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    #[serde(default)]
    pub path: Path,
    pub offset: usize,
}

impl Point {
    pub fn new(path: Path, offset: usize) -> Self {
        Self { path, offset }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    pub anchor: Point,
    pub focus: Point,
}

impl Selection {
    pub fn collapsed(point: Point) -> Self {
        Self {
            anchor: point.clone(),
            focus: point,
        }
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.focus
    }
}

#[derive(Debug, Clone)]
pub struct UndoRecord {
    pub inverse_ops: Vec<Op>,
    pub selection_before: Selection,
    pub selection_after: Selection,
}

#[derive(Debug, Default)]
pub struct EditorConfig {
    pub max_undo: usize,
    pub max_normalize_iterations: usize,
}

impl EditorConfig {
    fn with_defaults(mut self) -> Self {
        if self.max_undo == 0 {
            self.max_undo = 200;
        }
        if self.max_normalize_iterations == 0 {
            self.max_normalize_iterations = 100;
        }
        self
    }
}

pub struct Editor {
    doc: Document,
    selection: Selection,
    registry: PluginRegistry,
    config: EditorConfig,
    undo_stack: Vec<UndoRecord>,
    redo_stack: Vec<UndoRecord>,
}

impl Editor {
    pub fn new(doc: Document, selection: Selection, registry: PluginRegistry) -> Self {
        let config = EditorConfig::default().with_defaults();
        let mut editor = Self {
            doc,
            selection,
            registry,
            config,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        };
        editor.normalize_in_place();
        editor
    }

    pub fn with_core_plugins() -> Self {
        let registry = PluginRegistry::core();
        let doc = Document {
            children: vec![Node::paragraph("")],
        };
        let selection = Selection::collapsed(Point::new(vec![0, 0], 0));
        Self::new(doc, selection, registry)
    }

    pub fn doc(&self) -> &Document {
        &self.doc
    }

    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn set_selection(&mut self, selection: Selection) {
        self.selection = selection;
        self.normalize_selection_in_place();
    }

    pub fn registry(&self) -> &PluginRegistry {
        &self.registry
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(record) = self.undo_stack.pop() else {
            return false;
        };

        let selection_before = self.selection.clone();
        let mut redo_inverse: Vec<Op> = Vec::new();
        for op in record.inverse_ops.iter().cloned() {
            if let Ok(inv) = self.apply_op(op) {
                redo_inverse.push(inv);
            } else {
                // If we can't apply inverse ops, bail out and stop mutating further.
                break;
            }
        }

        let selection_after = record.selection_before.clone();
        self.selection = selection_after.clone();
        self.normalize_in_place();

        self.redo_stack.push(UndoRecord {
            inverse_ops: redo_inverse,
            selection_before,
            selection_after,
        });
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(record) = self.redo_stack.pop() else {
            return false;
        };

        let selection_before = self.selection.clone();
        let mut undo_inverse: Vec<Op> = Vec::new();
        for op in record.inverse_ops.iter().cloned() {
            if let Ok(inv) = self.apply_op(op) {
                undo_inverse.push(inv);
            } else {
                break;
            }
        }

        let selection_after = record.selection_after.clone();
        self.selection = selection_after.clone();
        self.normalize_in_place();

        self.undo_stack.push(UndoRecord {
            inverse_ops: undo_inverse,
            selection_before,
            selection_after,
        });
        true
    }

    pub fn apply(&mut self, tx: Transaction) -> Result<(), ApplyError> {
        let selection_before = self.selection.clone();

        let mut inverse_ops: Vec<Op> = Vec::new();
        for op in tx.ops.iter().cloned() {
            let inv = self.apply_op(op)?;
            inverse_ops.push(inv);
        }

        if let Some(sel) = tx.selection_after {
            self.selection = sel;
        }

        let inverse_normalize = self.normalize_with_inverse_ops()?;
        inverse_ops.extend(inverse_normalize);

        self.normalize_selection_in_place();

        let selection_after = self.selection.clone();

        self.undo_stack.push(UndoRecord {
            inverse_ops,
            selection_before,
            selection_after,
        });
        self.redo_stack.clear();
        if self.undo_stack.len() > self.config.max_undo {
            self.undo_stack.remove(0);
        }

        Ok(())
    }

    pub fn run_command(
        &mut self,
        id: &str,
        args: Option<serde_json::Value>,
    ) -> Result<(), CommandError> {
        let Some(command) = self.registry.command(id) else {
            return Err(CommandError::new(format!("Unknown command: {id}")));
        };
        (command.handler)(self, args)
    }

    fn normalize_in_place(&mut self) {
        let _ = self.normalize_with_inverse_ops();
        self.normalize_selection_in_place();
    }

    fn normalize_selection_in_place(&mut self) {
        self.selection = self
            .registry
            .normalize_selection(&self.doc, &self.selection);
    }

    fn normalize_with_inverse_ops(&mut self) -> Result<Vec<Op>, ApplyError> {
        let mut inverse_ops: Vec<Op> = Vec::new();
        for _ in 0..self.config.max_normalize_iterations {
            let ops = self.registry.normalize(&self.doc);
            if ops.is_empty() {
                return Ok(inverse_ops);
            }
            for op in ops {
                let inv = self.apply_op(op)?;
                inverse_ops.push(inv);
            }
        }
        Err(ApplyError::NormalizeDidNotConverge)
    }

    fn apply_op(&mut self, op: Op) -> Result<Op, ApplyError> {
        match op {
            Op::InsertText { path, offset, text } => {
                let text_node = node_text_mut(&mut self.doc, &path)?;
                let offset = clamp_to_char_boundary(&text_node.text, offset);
                text_node.text.insert_str(offset, &text);
                Ok(Op::RemoveText {
                    path,
                    range: offset..offset + text.len(),
                })
            }
            Op::RemoveText { path, range } => {
                let text_node = node_text_mut(&mut self.doc, &path)?;
                let start =
                    clamp_to_char_boundary(&text_node.text, range.start.min(text_node.text.len()));
                let end =
                    clamp_to_char_boundary(&text_node.text, range.end.min(text_node.text.len()));
                if start >= end {
                    return Ok(Op::InsertText {
                        path,
                        offset: start,
                        text: String::new(),
                    });
                }
                let removed = text_node.text[start..end].to_string();
                text_node.text.replace_range(start..end, "");
                Ok(Op::InsertText {
                    path,
                    offset: start,
                    text: removed,
                })
            }
            Op::InsertNode { path, node } => {
                insert_node(&mut self.doc, &path, node)?;
                Ok(Op::RemoveNode { path })
            }
            Op::RemoveNode { path } => {
                let removed = remove_node(&mut self.doc, &path)?;
                Ok(Op::InsertNode {
                    path,
                    node: removed,
                })
            }
            Op::SetNodeAttrs { path, patch } => {
                let node = node_mut(&mut self.doc, &path)?;
                let old = match node {
                    Node::Element(el) => patch_apply(&mut el.attrs, &patch),
                    Node::Void(v) => patch_apply(&mut v.attrs, &patch),
                    Node::Text(_) => {
                        return Err(ApplyError::InvalidPath("Text has no attrs".into()));
                    }
                };
                Ok(Op::SetNodeAttrs { path, patch: old })
            }
        }
    }
}

#[derive(Debug)]
pub enum ApplyError {
    InvalidPath(String),
    NormalizeDidNotConverge,
}

impl From<PathError> for ApplyError {
    fn from(value: PathError) -> Self {
        ApplyError::InvalidPath(value.0)
    }
}

#[derive(Debug)]
pub struct PathError(pub String);

fn clamp_to_char_boundary(s: &str, mut ix: usize) -> usize {
    ix = ix.min(s.len());
    while ix > 0 && !s.is_char_boundary(ix) {
        ix -= 1;
    }
    ix
}

fn node_mut<'a>(doc: &'a mut Document, path: &[usize]) -> Result<&'a mut Node, PathError> {
    if path.is_empty() {
        return Err(PathError("Empty path".into()));
    }

    let mut current: *mut Node = std::ptr::null_mut();
    let mut children: *mut Vec<Node> = &mut doc.children;

    for (depth, &ix) in path.iter().enumerate() {
        // SAFETY: We only keep raw pointers within this loop iteration.
        let vec = unsafe { &mut *children };
        if ix >= vec.len() {
            return Err(PathError(format!(
                "Path out of bounds at depth {depth}: {ix} >= {}",
                vec.len()
            )));
        }
        current = &mut vec[ix];
        if depth + 1 < path.len() {
            children = match unsafe { &mut *current } {
                Node::Element(el) => &mut el.children,
                Node::Void(_) | Node::Text(_) => {
                    return Err(PathError(format!("Non-container node at depth {depth}")));
                }
            };
        }
    }

    // SAFETY: current points to a node in the document tree.
    unsafe { current.as_mut() }.ok_or_else(|| PathError("Failed to resolve path".into()))
}

fn node_text_mut<'a>(doc: &'a mut Document, path: &[usize]) -> Result<&'a mut TextNode, PathError> {
    match node_mut(doc, path)? {
        Node::Text(t) => Ok(t),
        _ => Err(PathError("Expected Text node".into())),
    }
}

fn insert_node(doc: &mut Document, path: &[usize], node: Node) -> Result<(), PathError> {
    if path.is_empty() {
        return Err(PathError("Empty insert path".into()));
    }

    let (parent_path, index) = path.split_at(path.len() - 1);
    let index = index[0];

    let children = if parent_path.is_empty() {
        &mut doc.children
    } else {
        match node_mut(doc, parent_path)? {
            Node::Element(el) => &mut el.children,
            Node::Void(_) | Node::Text(_) => {
                return Err(PathError("Insert parent is not a container".into()));
            }
        }
    };

    if index > children.len() {
        return Err(PathError(format!(
            "Insert index out of bounds: {index} > {}",
            children.len()
        )));
    }
    children.insert(index, node);
    Ok(())
}

fn remove_node(doc: &mut Document, path: &[usize]) -> Result<Node, PathError> {
    if path.is_empty() {
        return Err(PathError("Empty remove path".into()));
    }

    let (parent_path, index) = path.split_at(path.len() - 1);
    let index = index[0];

    let children = if parent_path.is_empty() {
        &mut doc.children
    } else {
        match node_mut(doc, parent_path)? {
            Node::Element(el) => &mut el.children,
            Node::Void(_) | Node::Text(_) => {
                return Err(PathError("Remove parent is not a container".into()));
            }
        }
    };

    if index >= children.len() {
        return Err(PathError(format!(
            "Remove index out of bounds: {index} >= {}",
            children.len()
        )));
    }
    Ok(children.remove(index))
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AttrPatch {
    #[serde(default)]
    pub set: Attrs,
    #[serde(default)]
    pub remove: Vec<String>,
}

fn patch_apply(attrs: &mut Attrs, patch: &AttrPatch) -> AttrPatch {
    let mut old_set: Attrs = Attrs::new();
    let mut old_remove: Vec<String> = Vec::new();

    for (k, v) in &patch.set {
        if let Some(prev) = attrs.insert(k.clone(), v.clone()) {
            old_set.insert(k.clone(), prev);
        } else {
            old_remove.push(k.clone());
        }
    }

    for key in &patch.remove {
        if let Some(prev) = attrs.remove(key) {
            old_set.insert(key.clone(), prev);
        }
    }

    AttrPatch {
        set: old_set,
        remove: old_remove,
    }
}

impl Editor {
    pub fn core_specs(&self) -> &HashMap<String, NodeSpec> {
        self.registry.node_specs()
    }

    pub fn core_normalize_passes(&self) -> &[Box<dyn NormalizePass>] {
        self.registry.normalize_passes()
    }

    pub fn core_commands(&self) -> &HashMap<String, CommandSpec> {
        self.registry.commands()
    }
}
