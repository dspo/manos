use std::collections::{BTreeMap, HashMap};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ops::{Op, Path, Transaction};
use crate::plugin::{
    CommandError, CommandSpec, NodeSpec, NormalizePass, PluginRegistry, QueryError,
    TransactionPreview,
};

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

impl VoidNode {
    pub fn inline_text(&self) -> String {
        match self.kind.as_str() {
            "mention" => {
                let label = self
                    .attrs
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or("mention");
                if label.starts_with('@') {
                    label.to_string()
                } else {
                    format!("@{label}")
                }
            }
            _ => "□".to_string(),
        }
    }

    pub fn inline_text_len(&self) -> usize {
        match self.kind.as_str() {
            "mention" => {
                let label = self
                    .attrs
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or("mention");
                label.len() + (!label.starts_with('@') as usize)
            }
            _ => 1,
        }
    }
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
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub strikethrough: bool,
    #[serde(default)]
    pub code: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight_color: Option<String>,
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

    pub fn with_richtext_plugins() -> Self {
        let registry = PluginRegistry::richtext();
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

        let UndoRecord {
            inverse_ops,
            selection_before,
            selection_after,
        } = record;

        let mut redo_ops: Vec<Op> = Vec::new();
        for op in inverse_ops.iter().cloned() {
            if let Ok(inv) = self.apply_op(op) {
                redo_ops.push(inv);
            } else {
                // If we can't apply inverse ops, bail out and stop mutating further.
                break;
            }
        }
        redo_ops.reverse();

        self.selection = selection_before.clone();
        self.normalize_in_place();

        self.redo_stack.push(UndoRecord {
            selection_before,
            selection_after,
            inverse_ops: redo_ops,
        });
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(record) = self.redo_stack.pop() else {
            return false;
        };

        let UndoRecord {
            inverse_ops,
            selection_before,
            selection_after,
        } = record;

        let mut undo_ops: Vec<Op> = Vec::new();
        for op in inverse_ops.iter().cloned() {
            if let Ok(inv) = self.apply_op(op) {
                undo_ops.push(inv);
            } else {
                break;
            }
        }
        undo_ops.reverse();

        self.selection = selection_after.clone();
        self.normalize_in_place();

        self.undo_stack.push(UndoRecord {
            selection_before,
            selection_after,
            inverse_ops: undo_ops,
        });
        true
    }

    pub fn apply(&mut self, tx: Transaction) -> Result<(), ApplyError> {
        let tx = self.transform_transaction(tx);
        let selection_before = self.selection.clone();

        let mut inverse_ops: Vec<Op> = Vec::new();
        for op in tx.ops.iter().cloned() {
            let inv = self.apply_op(op)?;
            inverse_ops.push(inv);
        }

        if let Some(sel) = tx.selection_after {
            self.selection = sel;
        }

        let mut inverse_normalize = self.normalize_with_inverse_ops()?;
        inverse_ops.append(&mut inverse_normalize);
        inverse_ops.reverse();

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

    fn transform_transaction(&self, mut tx: Transaction) -> Transaction {
        for transform in self.registry.transaction_transforms() {
            if let Some(next) = transform.transform(self, &tx) {
                tx = next;
            }
        }
        tx
    }

    pub fn preview_transaction(&self, tx: &Transaction) -> Result<TransactionPreview, ApplyError> {
        let mut doc = self.doc.clone();
        let mut selection = self.selection.clone();

        for op in tx.ops.iter().cloned() {
            let _ = apply_op_to(&mut doc, &mut selection, op)?;
        }

        if let Some(sel) = &tx.selection_after {
            selection = sel.clone();
        }

        let mut converged = false;
        for _ in 0..self.config.max_normalize_iterations {
            let ops = self.registry.normalize(&doc);
            if ops.is_empty() {
                converged = true;
                break;
            }
            for op in ops {
                let _ = apply_op_to(&mut doc, &mut selection, op)?;
            }
        }

        if !converged {
            return Err(ApplyError::NormalizeDidNotConverge);
        }

        selection = self.registry.normalize_selection(&doc, &selection);

        Ok(TransactionPreview { doc, selection })
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

    pub fn run_query_json(&self, id: &str, args: Option<Value>) -> Result<Value, QueryError> {
        let Some(query) = self.registry.query(id) else {
            return Err(QueryError::new(format!("Unknown query: {id}")));
        };
        (query.handler)(self, args)
    }

    pub fn run_query<T>(&self, id: &str, args: Option<Value>) -> Result<T, QueryError>
    where
        T: DeserializeOwned,
    {
        let value = self.run_query_json(id, args)?;
        serde_json::from_value(value)
            .map_err(|err| QueryError::new(format!("Failed to decode query result: {err}")))
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
        apply_op_to(&mut self.doc, &mut self.selection, op)
    }
}

fn apply_op_to(doc: &mut Document, selection: &mut Selection, op: Op) -> Result<Op, ApplyError> {
    match op {
        Op::InsertText { path, offset, text } => {
            let text_node = node_text_mut(doc, &path)?;
            let offset = clamp_to_char_boundary(&text_node.text, offset);
            text_node.text.insert_str(offset, &text);
            transform_selection_insert_text(selection, &path, offset, text.len());
            Ok(Op::RemoveText {
                path,
                range: offset..offset + text.len(),
            })
        }
        Op::RemoveText { path, range } => {
            let text_node = node_text_mut(doc, &path)?;
            let start =
                clamp_to_char_boundary(&text_node.text, range.start.min(text_node.text.len()));
            let end = clamp_to_char_boundary(&text_node.text, range.end.min(text_node.text.len()));
            if start >= end {
                return Ok(Op::InsertText {
                    path,
                    offset: start,
                    text: String::new(),
                });
            }
            let removed = text_node.text[start..end].to_string();
            text_node.text.replace_range(start..end, "");
            transform_selection_remove_text(selection, &path, start..end);
            Ok(Op::InsertText {
                path,
                offset: start,
                text: removed,
            })
        }
        Op::InsertNode { path, node } => {
            insert_node(doc, &path, node)?;
            transform_selection_insert_node(selection, &path);
            Ok(Op::RemoveNode { path })
        }
        Op::RemoveNode { path } => {
            let removed = remove_node(doc, &path)?;
            transform_selection_remove_node(selection, &path, &removed, doc);
            Ok(Op::InsertNode {
                path,
                node: removed,
            })
        }
        Op::SetNodeAttrs { path, patch } => {
            let node = node_mut(doc, &path)?;
            let old = match node {
                Node::Element(el) => patch_apply(&mut el.attrs, &patch),
                Node::Void(v) => patch_apply(&mut v.attrs, &patch),
                Node::Text(_) => return Err(ApplyError::InvalidPath("Text has no attrs".into())),
            };
            Ok(Op::SetNodeAttrs { path, patch: old })
        }
        Op::SetTextMarks { path, marks } => {
            let text_node = node_text_mut(doc, &path)?;
            let old = std::mem::replace(&mut text_node.marks, marks);
            Ok(Op::SetTextMarks { path, marks: old })
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

fn transform_selection_insert_text(
    selection: &mut Selection,
    path: &[usize],
    offset: usize,
    len: usize,
) {
    for point in [&mut selection.anchor, &mut selection.focus] {
        if point.path == path && point.offset >= offset {
            point.offset = point.offset.saturating_add(len);
        }
    }
}

fn transform_selection_remove_text(
    selection: &mut Selection,
    path: &[usize],
    range: std::ops::Range<usize>,
) {
    let removed_len = range.end.saturating_sub(range.start);
    for point in [&mut selection.anchor, &mut selection.focus] {
        if point.path != path {
            continue;
        }
        if point.offset <= range.start {
            continue;
        }
        if point.offset >= range.end {
            point.offset = point.offset.saturating_sub(removed_len);
        } else {
            point.offset = range.start;
        }
    }
}

fn transform_selection_insert_node(selection: &mut Selection, path: &[usize]) {
    if path.is_empty() {
        return;
    }
    let (parent_path, index) = path.split_at(path.len() - 1);
    let index = index[0];

    for point in [&mut selection.anchor, &mut selection.focus] {
        if point.path.len() <= parent_path.len() {
            continue;
        }
        if !point.path.starts_with(parent_path) {
            continue;
        }
        let depth = parent_path.len();
        if point.path[depth] >= index {
            point.path[depth] += 1;
        }
    }
}

fn transform_selection_remove_node(
    selection: &mut Selection,
    path: &[usize],
    removed: &Node,
    doc_after_remove: &Document,
) {
    if path.is_empty() {
        return;
    }
    let (parent_path, index) = path.split_at(path.len() - 1);
    let index = index[0];

    let merge_prefix_len = match (removed, index.checked_sub(1)) {
        (Node::Text(removed_text), Some(left_index)) => {
            let mut left_path = parent_path.to_vec();
            left_path.push(left_index);
            match node_ref(doc_after_remove, &left_path) {
                Some(Node::Text(left_text))
                    if left_text.marks == removed_text.marks
                        && left_text.text.ends_with(&removed_text.text) =>
                {
                    Some(left_text.text.len().saturating_sub(removed_text.text.len()))
                }
                _ => None,
            }
        }
        _ => None,
    };

    for point in [&mut selection.anchor, &mut selection.focus] {
        if point.path.len() <= parent_path.len() {
            continue;
        }
        if !point.path.starts_with(parent_path) {
            continue;
        }
        let depth = parent_path.len();
        let ix = point.path[depth];
        if ix > index {
            point.path[depth] = ix - 1;
            continue;
        }
        if ix < index {
            continue;
        }

        // Point was inside the removed subtree. Map it to a nearby point.
        if let (Some(prefix), Node::Text(removed_text), Some(left_index)) =
            (merge_prefix_len, removed, index.checked_sub(1))
        {
            point.path.truncate(depth + 1);
            point.path[depth] = left_index;
            point.offset = (prefix + point.offset).min(prefix + removed_text.text.len());
        } else {
            point.path.truncate(depth + 1);
            point.path[depth] = index.saturating_sub(1);
            point.offset = 0;
        }
    }
}

fn node_ref<'a>(doc: &'a Document, path: &[usize]) -> Option<&'a Node> {
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
