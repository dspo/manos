//! Math plugin for inline and block math formulas.

use serde_json::Value;

use crate::core::{Attrs, Node, Point, Selection, TextNode, VoidNode};
use crate::ops::{Op, Path, Transaction};
use crate::plugin::{
    ChildConstraint, CommandError, CommandSpec, NodeRole, NodeSpec, PlatePlugin,
};

pub struct MathPlugin;

impl PlatePlugin for MathPlugin {
    fn id(&self) -> &'static str {
        "math"
    }

    fn node_specs(&self) -> Vec<NodeSpec> {
        vec![
            NodeSpec {
                kind: "math_inline".to_string(),
                role: NodeRole::Inline,
                is_void: true,
                children: ChildConstraint::None,
            },
            NodeSpec {
                kind: "math_block".to_string(),
                role: NodeRole::Block,
                is_void: true,
                children: ChildConstraint::None,
            },
        ]
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new("math.insert_inline", "Insert inline math", |editor, args| {
                let latex = args
                    .as_ref()
                    .and_then(|v| v.get("latex"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                insert_math_inline(editor, latex)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("failed to insert inline math: {e:?}"))
                        })
                    })
            })
            .description("Insert an inline math formula at the caret/selection.")
            .keywords(["math", "latex", "formula", "inline", "equation"])
            .args_example(serde_json::json!({ "latex": "E = mc^2" })),
            CommandSpec::new("math.insert_block", "Insert block math", |editor, args| {
                let latex = args
                    .as_ref()
                    .and_then(|v| v.get("latex"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                insert_math_block(editor, latex)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("failed to insert block math: {e:?}"))
                        })
                    })
            })
            .description("Insert a block-level math formula.")
            .keywords(["math", "latex", "formula", "block", "equation", "display"])
            .args_example(serde_json::json!({ "latex": "\\int_0^\\infty e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}" })),
            CommandSpec::new("math.update", "Update math formula", |editor, args| {
                let latex = args
                    .as_ref()
                    .and_then(|v| v.get("latex"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| CommandError::new("missing args.latex"))?
                    .to_string();
                let path: Path = args
                    .as_ref()
                    .and_then(|v| v.get("path"))
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .ok_or_else(|| CommandError::new("missing args.path"))?;

                update_math(editor, path, latex)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor.apply(tx).map_err(|e| {
                            CommandError::new(format!("failed to update math: {e:?}"))
                        })
                    })
            })
            .description("Update the LaTeX content of an existing math node.")
            .keywords(["math", "latex", "update", "edit"])
            .args_example(serde_json::json!({ "path": [0, 1], "latex": "a^2 + b^2 = c^2" })),
        ]
    }
}

fn insert_math_inline(editor: &crate::core::Editor, latex: String) -> Result<Transaction, String> {
    let sel = editor.selection().clone();
    if !sel.is_collapsed() {
        return Err("selection must be collapsed".into());
    }

    let focus = sel.focus;
    if focus.path.is_empty() {
        return Err("selection is not in a text node".into());
    }
    let (child_ix, block_path) = focus
        .path
        .split_last()
        .ok_or_else(|| "selection is not in a text node".to_string())?;

    let Some(Node::Element(el)) = super::node_at_path(editor.doc(), block_path) else {
        return Err("selection is not in a text block".into());
    };
    let Some(Node::Text(text)) = el.children.get(*child_ix) else {
        return Err("selection is not in a text node".into());
    };

    let cursor = super::clamp_to_char_boundary(&text.text, focus.offset);
    let left = text.text.get(..cursor).unwrap_or("").to_string();
    let right = text.text.get(cursor..).unwrap_or("").to_string();
    let marks = text.marks.clone();

    let mut replacement: Vec<Node> = Vec::new();
    let base_child_ix = *child_ix;
    let mut math_ix = base_child_ix;

    if !left.is_empty() {
        replacement.push(Node::Text(TextNode {
            text: left,
            marks: marks.clone(),
        }));
        math_ix += 1;
    }

    let mut attrs = Attrs::default();
    attrs.insert("latex".to_string(), Value::String(latex));
    replacement.push(Node::Void(VoidNode {
        kind: "math_inline".to_string(),
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
    selection_path.push(math_ix + 1);
    let selection_after = Selection::collapsed(Point::new(selection_path, 0));
    Ok(Transaction::new(ops)
        .selection_after(selection_after)
        .source("command:math.insert_inline"))
}

fn insert_math_block(editor: &crate::core::Editor, latex: String) -> Result<Transaction, String> {
    let focus = editor.selection().focus.clone();
    let block_path = focus.path.split_last().map(|(_, p)| p).unwrap_or(&[]);

    let (parent_path, insert_at) = if block_path.is_empty() {
        (Vec::new(), editor.doc().children.len())
    } else {
        let (block_ix, parent) = block_path.split_last().unwrap();
        (parent.to_vec(), block_ix + 1)
    };

    let math_path = {
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

    let mut attrs = Attrs::default();
    attrs.insert("latex".to_string(), Value::String(latex));

    Ok(Transaction::new(vec![
        Op::InsertNode {
            path: math_path,
            node: Node::Void(VoidNode {
                kind: "math_block".to_string(),
                attrs,
            }),
        },
        Op::InsertNode {
            path: paragraph_element_path,
            node: Node::paragraph(""),
        },
    ])
    .selection_after(Selection::collapsed(Point::new(paragraph_text_path, 0)))
    .source("command:math.insert_block"))
}

fn update_math(editor: &crate::core::Editor, path: Path, latex: String) -> Result<Transaction, String> {
    let Some(node) = super::node_at_path(editor.doc(), &path) else {
        return Err("path does not exist".into());
    };

    let Node::Void(v) = node else {
        return Err("node at path is not a void node".into());
    };

    if v.kind != "math_inline" && v.kind != "math_block" {
        return Err(format!("node at path is not a math node (kind: {})", v.kind));
    }

    let mut patch_set = Attrs::default();
    patch_set.insert("latex".to_string(), Value::String(latex));

    Ok(Transaction::new(vec![Op::SetNodeAttrs {
        path,
        patch: crate::core::AttrPatch {
            set: patch_set,
            remove: Vec::new(),
        },
    }])
    .source("command:math.update"))
}

