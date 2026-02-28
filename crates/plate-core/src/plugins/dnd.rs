//! Drag and drop plugin for block reordering.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ops::{Op, Path, Transaction};
use crate::plugin::{CommandError, CommandSpec, PlatePlugin, QueryError, QuerySpec};

/// Arguments for the `dnd.move_node` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveNodeArgs {
    /// Source path of the node to move.
    pub from: Path,
    /// Destination path where the node should be inserted.
    pub to: Path,
}

/// Arguments for the `dnd.can_drop` query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanDropArgs {
    /// Source path of the node being dragged.
    pub from: Path,
    /// Target path where the node would be dropped.
    pub to: Path,
}

/// Result of the `dnd.can_drop` query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanDropResult {
    pub allowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

pub struct DndPlugin;

impl PlatePlugin for DndPlugin {
    fn id(&self) -> &'static str {
        "dnd"
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![CommandSpec::new("dnd.move_node", "Move Node", dnd_move_node_handler)
            .description("Move a node from one position to another")
            .args_example(serde_json::json!({
                "from": [0],
                "to": [2]
            }))
            .hidden(true)]
    }

    fn queries(&self) -> Vec<QuerySpec> {
        vec![QuerySpec {
            id: "dnd.can_drop".to_string(),
            handler: std::sync::Arc::new(dnd_can_drop_handler),
        }]
    }
}

fn dnd_move_node_handler(
    editor: &mut crate::core::Editor,
    args: Option<Value>,
) -> Result<(), CommandError> {
    let args: MoveNodeArgs = args
        .ok_or_else(|| CommandError::new("missing arguments for dnd.move_node"))?
        .try_into()
        .map_err(|e: serde_json::Error| CommandError::new(format!("invalid arguments: {e}")))?;

    let MoveNodeArgs { from, to } = args;

    if from.is_empty() {
        return Err(CommandError::new("source path cannot be empty"));
    }
    if to.is_empty() {
        return Err(CommandError::new("destination path cannot be empty"));
    }

    // Check if moving to same position (no-op)
    if from == to {
        return Ok(());
    }

    // Check if trying to move a node into itself
    if to.starts_with(&from) {
        return Err(CommandError::new("cannot move a node into itself"));
    }

    // Get the node to move
    let node = super::node_at_path(editor.doc(), &from)
        .ok_or_else(|| CommandError::new("source node not found"))?
        .clone();

    // Calculate the adjusted destination path after removal
    let adjusted_to = adjust_path_after_remove(&from, &to);

    let tx = Transaction::new(vec![
        Op::RemoveNode { path: from.clone() },
        Op::InsertNode {
            path: adjusted_to,
            node,
        },
    ])
    .source("dnd.move_node");

    editor
        .apply(tx)
        .map_err(|e| CommandError::new(format!("failed to apply move: {e:?}")))
}

fn dnd_can_drop_handler(
    editor: &crate::core::Editor,
    args: Option<Value>,
) -> Result<Value, QueryError> {
    let args: CanDropArgs = args
        .ok_or_else(|| QueryError::new("missing arguments for dnd.can_drop"))?
        .try_into()
        .map_err(|e: serde_json::Error| QueryError::new(format!("invalid arguments: {e}")))?;

    let CanDropArgs { from, to } = args;

    // Basic validation
    if from.is_empty() {
        return Ok(serde_json::to_value(CanDropResult {
            allowed: false,
            reason: Some("source path cannot be empty".to_string()),
        })
        .unwrap());
    }

    if to.is_empty() {
        return Ok(serde_json::to_value(CanDropResult {
            allowed: false,
            reason: Some("destination path cannot be empty".to_string()),
        })
        .unwrap());
    }

    // Cannot move to same position
    if from == to {
        return Ok(serde_json::to_value(CanDropResult {
            allowed: false,
            reason: Some("cannot move to same position".to_string()),
        })
        .unwrap());
    }

    // Cannot move a node into itself
    if to.starts_with(&from) {
        return Ok(serde_json::to_value(CanDropResult {
            allowed: false,
            reason: Some("cannot move a node into itself".to_string()),
        })
        .unwrap());
    }

    // Check if source exists
    if super::node_at_path(editor.doc(), &from).is_none() {
        return Ok(serde_json::to_value(CanDropResult {
            allowed: false,
            reason: Some("source node not found".to_string()),
        })
        .unwrap());
    }

    // For top-level blocks, always allow reordering
    if from.len() == 1 && to.len() == 1 {
        return Ok(serde_json::to_value(CanDropResult {
            allowed: true,
            reason: None,
        })
        .unwrap());
    }

    // Default: allow the drop
    Ok(serde_json::to_value(CanDropResult {
        allowed: true,
        reason: None,
    })
    .unwrap())
}

/// Adjust the destination path after a node is removed from the source path.
fn adjust_path_after_remove(from: &[usize], to: &[usize]) -> Path {
    if from.is_empty() || to.is_empty() {
        return to.to_vec();
    }

    // Find common prefix length
    let common_len = from
        .iter()
        .zip(to.iter())
        .take_while(|(a, b)| a == b)
        .count();

    // If paths diverge before the last element of `from`, no adjustment needed
    if common_len < from.len() - 1 {
        return to.to_vec();
    }

    // If `to` is at the same level as `from` (same parent)
    if common_len == from.len() - 1 && to.len() > common_len {
        let from_ix = from[common_len];
        let to_ix = to[common_len];

        // If destination is after source at the same level, decrement by 1
        if to_ix > from_ix {
            let mut adjusted = to.to_vec();
            adjusted[common_len] -= 1;
            return adjusted;
        }
    }

    to.to_vec()
}

impl TryFrom<Value> for MoveNodeArgs {
    type Error = serde_json::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<Value> for CanDropArgs {
    type Error = serde_json::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}
