//! Slash command plugin for command palette activation.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::Node;
use crate::ops::Transaction;
use crate::plugin::{PlatePlugin, QueryError, QuerySpec, TransactionTransform};

pub struct SlashCommandPlugin;

impl PlatePlugin for SlashCommandPlugin {
    fn id(&self) -> &'static str {
        "slash_command"
    }

    fn transaction_transforms(&self) -> Vec<Box<dyn TransactionTransform>> {
        vec![Box::new(SlashCommandDetector)]
    }

    fn queries(&self) -> Vec<QuerySpec> {
        vec![
            QuerySpec {
                id: "slash_command.list_commands".to_string(),
                handler: std::sync::Arc::new(|editor, _args| {
                    let commands: Vec<SlashCommandInfo> = editor
                        .registry()
                        .commands()
                        .values()
                        .filter(|cmd| !cmd.hidden)
                        .map(|cmd| SlashCommandInfo {
                            id: cmd.id.clone(),
                            label: cmd.label.clone(),
                            description: cmd.description.clone(),
                            keywords: cmd.keywords.clone(),
                        })
                        .collect();
                    serde_json::to_value(commands)
                        .map_err(|e| QueryError::new(format!("failed to serialize commands: {e}")))
                }),
            },
            QuerySpec {
                id: "slash_command.filter_commands".to_string(),
                handler: std::sync::Arc::new(|editor, args| {
                    let filter = args
                        .and_then(|v| v.get("filter").and_then(|f| f.as_str()).map(|s| s.to_lowercase()))
                        .unwrap_or_default();

                    let commands: Vec<SlashCommandInfo> = editor
                        .registry()
                        .commands()
                        .values()
                        .filter(|cmd| !cmd.hidden)
                        .filter(|cmd| {
                            if filter.is_empty() {
                                return true;
                            }
                            let label_match = cmd.label.to_lowercase().contains(&filter);
                            let id_match = cmd.id.to_lowercase().contains(&filter);
                            let keyword_match = cmd.keywords.iter().any(|k| k.to_lowercase().contains(&filter));
                            let desc_match = cmd
                                .description
                                .as_ref()
                                .map(|d| d.to_lowercase().contains(&filter))
                                .unwrap_or(false);
                            label_match || id_match || keyword_match || desc_match
                        })
                        .map(|cmd| SlashCommandInfo {
                            id: cmd.id.clone(),
                            label: cmd.label.clone(),
                            description: cmd.description.clone(),
                            keywords: cmd.keywords.clone(),
                        })
                        .collect();

                    serde_json::to_value(commands)
                        .map_err(|e| QueryError::new(format!("failed to serialize commands: {e}")))
                }),
            },
            QuerySpec {
                id: "slash_command.is_active".to_string(),
                handler: std::sync::Arc::new(|editor, _args| {
                    let is_active = is_slash_command_active(editor);
                    Ok(Value::Bool(is_active))
                }),
            },
            QuerySpec {
                id: "slash_command.get_filter_text".to_string(),
                handler: std::sync::Arc::new(|editor, _args| {
                    let filter_text = get_slash_command_filter_text(editor);
                    Ok(filter_text.map(Value::String).unwrap_or(Value::Null))
                }),
            },
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SlashCommandInfo {
    id: String,
    label: String,
    description: Option<String>,
    keywords: Vec<String>,
}

struct SlashCommandDetector;

impl TransactionTransform for SlashCommandDetector {
    fn id(&self) -> &'static str {
        "slash_command.detector"
    }

    fn transform(&self, editor: &crate::core::Editor, tx: &Transaction) -> Option<Transaction> {
        let source = tx.meta.source.as_deref()?;
        if source != "ime:replace_text" {
            return None;
        }

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

        let children = if let Some(doc) = &doc_after {
            let Node::Element(el) = super::node_at_path(doc, &block_path)? else {
                return None;
            };
            el.children.clone()
        } else {
            let Node::Element(el) = super::node_at_path(editor.doc(), &block_path)? else {
                return None;
            };
            el.children.clone()
        };

        let mut text = String::new();
        for (ix, child) in children.iter().enumerate() {
            match child {
                Node::Text(t) => {
                    if ix < focus_child_ix {
                        text.push_str(&t.text);
                    } else if ix == focus_child_ix {
                        let end = focus.offset.min(t.text.len());
                        text.push_str(&t.text[..end]);
                    }
                }
                _ => return None,
            }
        }

        if text == "/" {
            let mut new_tx = tx.clone();
            new_tx.meta.source = Some("slash_command:activated".to_string());
            return Some(new_tx);
        }

        None
    }
}

fn is_slash_command_active(editor: &crate::core::Editor) -> bool {
    get_slash_command_filter_text(editor).is_some()
}

fn get_slash_command_filter_text(editor: &crate::core::Editor) -> Option<String> {
    let sel = editor.selection();
    if !sel.is_collapsed() {
        return None;
    }

    let focus = &sel.focus;
    let (focus_child_ix, block_path) = focus.path.split_last()?;
    let block_path = block_path.to_vec();
    let focus_child_ix = *focus_child_ix;

    let Node::Element(el) = super::node_at_path(editor.doc(), &block_path)? else {
        return None;
    };

    let mut text = String::new();
    for (ix, child) in el.children.iter().enumerate() {
        match child {
            Node::Text(t) => {
                if ix < focus_child_ix {
                    text.push_str(&t.text);
                } else if ix == focus_child_ix {
                    let end = focus.offset.min(t.text.len());
                    text.push_str(&t.text[..end]);
                }
            }
            _ => return None,
        }
    }

    if text.starts_with('/') {
        Some(text[1..].to_string())
    } else {
        None
    }
}
