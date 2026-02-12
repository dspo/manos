//! Block selection plugin for selecting and manipulating blocks.

use serde::{Deserialize, Serialize};

use crate::core::{Document, Node, Point, Selection};
use crate::ops::{Op, Path, Transaction};
use crate::plugin::{
    CommandError, CommandSpec, NodeRole, PlatePlugin, PluginRegistry, QueryError, QuerySpec,
};

pub struct BlockSelectionPlugin;

impl PlatePlugin for BlockSelectionPlugin {
    fn id(&self) -> &'static str {
        "block_selection"
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new(
                "block_selection.select_block",
                "Select block",
                |editor, args| {
                    let path: Path = args
                        .as_ref()
                        .and_then(|v| v.get("path"))
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .ok_or_else(|| CommandError::new("missing args.path"))?;

                    if !is_valid_block_path(editor.doc(), editor.registry(), &path) {
                        return Err(CommandError::new("invalid block path"));
                    }

                    let selection = block_selection_for_path(editor.doc(), &path)
                        .ok_or_else(|| CommandError::new("cannot create selection for block"))?;
                    editor.set_selection(selection);
                    Ok(())
                },
            )
            .description("Select a block at the specified path.")
            .keywords(["select", "block"])
            .args_example(serde_json::json!({ "path": [0] })),
            CommandSpec::new(
                "block_selection.select_blocks",
                "Select multiple blocks",
                |editor, args| {
                    let paths: Vec<Path> = args
                        .as_ref()
                        .and_then(|v| v.get("paths"))
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .ok_or_else(|| CommandError::new("missing args.paths"))?;

                    if paths.is_empty() {
                        return Err(CommandError::new("paths array is empty"));
                    }

                    for path in &paths {
                        if !is_valid_block_path(editor.doc(), editor.registry(), path) {
                            return Err(CommandError::new(format!(
                                "invalid block path: {:?}",
                                path
                            )));
                        }
                    }

                    let selection = multi_block_selection(editor.doc(), &paths)
                        .ok_or_else(|| CommandError::new("cannot create selection for blocks"))?;
                    editor.set_selection(selection);
                    Ok(())
                },
            )
            .description("Select multiple blocks at the specified paths.")
            .keywords(["select", "blocks", "multi"])
            .args_example(serde_json::json!({ "paths": [[0], [1], [2]] })),
            CommandSpec::new(
                "block_selection.delete_selected_blocks",
                "Delete selected blocks",
                |editor, args| {
                    let paths: Vec<Path> = args
                        .as_ref()
                        .and_then(|v| v.get("paths"))
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .ok_or_else(|| CommandError::new("missing args.paths"))?;

                    if paths.is_empty() {
                        return Err(CommandError::new("paths array is empty"));
                    }

                    let mut sorted_paths = paths.clone();
                    sorted_paths.sort_by(|a, b| b.cmp(a));

                    let mut ops = Vec::new();
                    for path in sorted_paths {
                        if is_valid_block_path(editor.doc(), editor.registry(), &path) {
                            ops.push(Op::RemoveNode { path });
                        }
                    }

                    if ops.is_empty() {
                        return Err(CommandError::new("no valid blocks to delete"));
                    }

                    let tx = Transaction::new(ops)
                        .source("command:block_selection.delete_selected_blocks");
                    editor
                        .apply(tx)
                        .map_err(|e| CommandError::new(format!("failed to delete blocks: {e:?}")))
                },
            )
            .description("Delete blocks at the specified paths.")
            .keywords(["delete", "remove", "blocks"])
            .args_example(serde_json::json!({ "paths": [[0], [1]] })),
            CommandSpec::new(
                "block_selection.duplicate_block",
                "Duplicate block",
                |editor, args| {
                    let path: Path = args
                        .as_ref()
                        .and_then(|v| v.get("path"))
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .ok_or_else(|| CommandError::new("missing args.path"))?;

                    let node = get_node_at_path(editor.doc(), &path)
                        .ok_or_else(|| CommandError::new("invalid block path"))?
                        .clone();

                    let mut insert_path = path.clone();
                    if let Some(last) = insert_path.last_mut() {
                        *last += 1;
                    }

                    let tx = Transaction::new(vec![Op::InsertNode {
                        path: insert_path,
                        node,
                    }])
                    .source("command:block_selection.duplicate_block");

                    editor
                        .apply(tx)
                        .map_err(|e| CommandError::new(format!("failed to duplicate block: {e:?}")))
                },
            )
            .description("Duplicate a block at the specified path.")
            .keywords(["duplicate", "copy", "clone", "block"])
            .args_example(serde_json::json!({ "path": [0] })),
            CommandSpec::new(
                "block_selection.move_block_up",
                "Move block up",
                |editor, args| {
                    let path: Path = args
                        .as_ref()
                        .and_then(|v| v.get("path"))
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .ok_or_else(|| CommandError::new("missing args.path"))?;

                    if path.is_empty() {
                        return Err(CommandError::new("empty path"));
                    }

                    let last_ix = *path.last().unwrap();
                    if last_ix == 0 {
                        return Err(CommandError::new("block is already at the top"));
                    }

                    let node = get_node_at_path(editor.doc(), &path)
                        .ok_or_else(|| CommandError::new("invalid block path"))?
                        .clone();

                    let mut new_path = path.clone();
                    *new_path.last_mut().unwrap() = last_ix - 1;

                    let tx = Transaction::new(vec![
                        Op::RemoveNode { path: path.clone() },
                        Op::InsertNode {
                            path: new_path,
                            node,
                        },
                    ])
                    .source("command:block_selection.move_block_up");

                    editor
                        .apply(tx)
                        .map_err(|e| CommandError::new(format!("failed to move block: {e:?}")))
                },
            )
            .description("Move a block up by one position.")
            .keywords(["move", "up", "block", "reorder"])
            .args_example(serde_json::json!({ "path": [1] })),
            CommandSpec::new(
                "block_selection.move_block_down",
                "Move block down",
                |editor, args| {
                    let path: Path = args
                        .as_ref()
                        .and_then(|v| v.get("path"))
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .ok_or_else(|| CommandError::new("missing args.path"))?;

                    if path.is_empty() {
                        return Err(CommandError::new("empty path"));
                    }

                    let sibling_count = get_sibling_count(editor.doc(), &path)
                        .ok_or_else(|| CommandError::new("cannot determine sibling count"))?;
                    let last_ix = *path.last().unwrap();
                    if last_ix >= sibling_count - 1 {
                        return Err(CommandError::new("block is already at the bottom"));
                    }

                    let node = get_node_at_path(editor.doc(), &path)
                        .ok_or_else(|| CommandError::new("invalid block path"))?
                        .clone();

                    let mut new_path = path.clone();
                    *new_path.last_mut().unwrap() = last_ix + 2;

                    let tx = Transaction::new(vec![
                        Op::RemoveNode { path: path.clone() },
                        Op::InsertNode {
                            path: new_path,
                            node,
                        },
                    ])
                    .source("command:block_selection.move_block_down");

                    editor
                        .apply(tx)
                        .map_err(|e| CommandError::new(format!("failed to move block: {e:?}")))
                },
            )
            .description("Move a block down by one position.")
            .keywords(["move", "down", "block", "reorder"])
            .args_example(serde_json::json!({ "path": [0] })),
        ]
    }

    fn queries(&self) -> Vec<QuerySpec> {
        vec![
            QuerySpec {
                id: "block_selection.get_selected_blocks".to_string(),
                handler: std::sync::Arc::new(|editor, args| {
                    let paths: Option<Vec<Path>> = args
                        .as_ref()
                        .and_then(|v| v.get("paths"))
                        .and_then(|v| serde_json::from_value(v.clone()).ok());

                    let result: Vec<BlockInfo> = if let Some(paths) = paths {
                        paths
                            .iter()
                            .filter_map(|path| {
                                get_block_info(editor.doc(), editor.registry(), path)
                            })
                            .collect()
                    } else {
                        get_blocks_in_selection(
                            editor.doc(),
                            editor.registry(),
                            editor.selection(),
                        )
                    };

                    serde_json::to_value(result)
                        .map_err(|e| QueryError::new(format!("failed to serialize: {e}")))
                }),
            },
            QuerySpec {
                id: "block_selection.get_block_at_path".to_string(),
                handler: std::sync::Arc::new(|editor, args| {
                    let path: Path = args
                        .as_ref()
                        .and_then(|v| v.get("path"))
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .ok_or_else(|| QueryError::new("missing args.path"))?;

                    let info = get_block_info(editor.doc(), editor.registry(), &path)
                        .ok_or_else(|| QueryError::new("invalid block path"))?;

                    serde_json::to_value(info)
                        .map_err(|e| QueryError::new(format!("failed to serialize: {e}")))
                }),
            },
            QuerySpec {
                id: "block_selection.get_all_block_paths".to_string(),
                handler: std::sync::Arc::new(|editor, _args| {
                    let paths = collect_all_block_paths(editor.doc(), editor.registry());
                    serde_json::to_value(paths)
                        .map_err(|e| QueryError::new(format!("failed to serialize: {e}")))
                }),
            },
            QuerySpec {
                id: "block_selection.can_move_up".to_string(),
                handler: std::sync::Arc::new(|_editor, args| {
                    let path: Path = args
                        .as_ref()
                        .and_then(|v| v.get("path"))
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .ok_or_else(|| QueryError::new("missing args.path"))?;

                    let can_move = !path.is_empty() && path.last().copied().unwrap_or(0) > 0;
                    serde_json::to_value(can_move)
                        .map_err(|e| QueryError::new(format!("failed to serialize: {e}")))
                }),
            },
            QuerySpec {
                id: "block_selection.can_move_down".to_string(),
                handler: std::sync::Arc::new(|editor, args| {
                    let path: Path = args
                        .as_ref()
                        .and_then(|v| v.get("path"))
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .ok_or_else(|| QueryError::new("missing args.path"))?;

                    let can_move = if path.is_empty() {
                        false
                    } else if let Some(sibling_count) =
                        get_sibling_count(editor.doc(), &path)
                    {
                        path.last().copied().unwrap_or(0) < sibling_count - 1
                    } else {
                        false
                    };
                    serde_json::to_value(can_move)
                        .map_err(|e| QueryError::new(format!("failed to serialize: {e}")))
                }),
            },
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlockInfo {
    path: Path,
    kind: String,
    is_void: bool,
    role: String,
}

fn is_valid_block_path(doc: &Document, registry: &PluginRegistry, path: &Path) -> bool {
    if path.is_empty() {
        return false;
    }
    let Some(node) = get_node_at_path(doc, path) else {
        return false;
    };
    match node {
        Node::Element(el) => registry
            .node_specs()
            .get(&el.kind)
            .map(|spec| spec.role == NodeRole::Block)
            .unwrap_or(true),
        Node::Void(v) => registry
            .node_specs()
            .get(&v.kind)
            .map(|spec| spec.role == NodeRole::Block)
            .unwrap_or(true),
        Node::Text(_) => false,
    }
}

fn get_node_at_path<'a>(doc: &'a Document, path: &Path) -> Option<&'a Node> {
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

fn get_sibling_count(doc: &Document, path: &Path) -> Option<usize> {
    if path.is_empty() {
        return None;
    }
    if path.len() == 1 {
        return Some(doc.children.len());
    }
    let parent_path = path[..path.len() - 1].to_vec();
    match get_node_at_path(doc, &parent_path)? {
        Node::Element(el) => Some(el.children.len()),
        _ => None,
    }
}

fn block_selection_for_path(doc: &Document, path: &Path) -> Option<Selection> {
    let node = get_node_at_path(doc, path)?;
    let start = first_text_point_in_node(node, path)?;
    let end = last_text_point_in_node(node, path)?;
    Some(Selection {
        anchor: start,
        focus: end,
    })
}

fn first_text_point_in_node(node: &Node, base_path: &Path) -> Option<Point> {
    match node {
        Node::Text(t) => Some(Point {
            path: base_path.to_vec(),
            offset: 0.min(t.text.len()),
        }),
        Node::Element(el) => {
            for (ix, child) in el.children.iter().enumerate() {
                let mut child_path = base_path.to_vec();
                child_path.push(ix);
                if let Some(point) = first_text_point_in_node(child, &child_path) {
                    return Some(point);
                }
            }
            None
        }
        Node::Void(_) => None,
    }
}

fn last_text_point_in_node(node: &Node, base_path: &Path) -> Option<Point> {
    match node {
        Node::Text(t) => Some(Point {
            path: base_path.to_vec(),
            offset: t.text.len(),
        }),
        Node::Element(el) => {
            for (ix, child) in el.children.iter().enumerate().rev() {
                let mut child_path = base_path.to_vec();
                child_path.push(ix);
                if let Some(point) = last_text_point_in_node(child, &child_path) {
                    return Some(point);
                }
            }
            None
        }
        Node::Void(_) => None,
    }
}

fn multi_block_selection(doc: &Document, paths: &[Path]) -> Option<Selection> {
    if paths.is_empty() {
        return None;
    }

    let mut sorted = paths.to_vec();
    sorted.sort();

    let first_path = sorted.first()?;
    let last_path = sorted.last()?;

    let first_node = get_node_at_path(doc, first_path)?;
    let last_node = get_node_at_path(doc, last_path)?;

    let start = first_text_point_in_node(first_node, first_path)?;
    let end = last_text_point_in_node(last_node, last_path)?;

    Some(Selection {
        anchor: start,
        focus: end,
    })
}

fn get_block_info(
    doc: &Document,
    registry: &PluginRegistry,
    path: &Path,
) -> Option<BlockInfo> {
    let node = get_node_at_path(doc, path)?;
    match node {
        Node::Element(el) => {
            let spec = registry.node_specs().get(&el.kind);
            Some(BlockInfo {
                path: path.to_vec(),
                kind: el.kind.clone(),
                is_void: false,
                role: spec
                    .map(|s| format!("{:?}", s.role))
                    .unwrap_or_else(|| "Unknown".to_string()),
            })
        }
        Node::Void(v) => {
            let spec = registry.node_specs().get(&v.kind);
            Some(BlockInfo {
                path: path.to_vec(),
                kind: v.kind.clone(),
                is_void: true,
                role: spec
                    .map(|s| format!("{:?}", s.role))
                    .unwrap_or_else(|| "Unknown".to_string()),
            })
        }
        Node::Text(_) => None,
    }
}

fn get_blocks_in_selection(
    doc: &Document,
    registry: &PluginRegistry,
    selection: &Selection,
) -> Vec<BlockInfo> {
    let mut blocks = Vec::new();

    let anchor_block = selection.anchor.path.first().copied().unwrap_or(0);
    let focus_block = selection.focus.path.first().copied().unwrap_or(0);

    let (start, end) = if anchor_block <= focus_block {
        (anchor_block, focus_block)
    } else {
        (focus_block, anchor_block)
    };

    for ix in start..=end {
        if let Some(info) = get_block_info(doc, registry, &vec![ix]) {
            blocks.push(info);
        }
    }

    blocks
}

fn collect_all_block_paths(doc: &Document, registry: &PluginRegistry) -> Vec<Path> {
    fn walk(
        nodes: &[Node],
        path: &mut Vec<usize>,
        out: &mut Vec<Path>,
        registry: &PluginRegistry,
    ) {
        for (ix, node) in nodes.iter().enumerate() {
            path.push(ix);

            let is_block = match node {
                Node::Element(el) => registry
                    .node_specs()
                    .get(&el.kind)
                    .map(|s| s.role == NodeRole::Block)
                    .unwrap_or(true),
                Node::Void(v) => registry
                    .node_specs()
                    .get(&v.kind)
                    .map(|s| s.role == NodeRole::Block)
                    .unwrap_or(true),
                Node::Text(_) => false,
            };

            if is_block {
                out.push(path.clone());
            }

            if let Node::Element(el) = node {
                walk(&el.children, path, out, registry);
            }

            path.pop();
        }
    }

    let mut out = Vec::new();
    walk(&doc.children, &mut Vec::new(), &mut out, registry);
    out
}
