//! Find and replace plugin for text search and replacement.

use crate::core::{Document, Node, Point, Selection};
use crate::ops::{Op, Path, Transaction};
use crate::plugin::{
    CommandError, CommandSpec, PlatePlugin, PluginRegistry, QueryError, QuerySpec,
};

pub struct FindReplacePlugin;

impl PlatePlugin for FindReplacePlugin {
    fn id(&self) -> &'static str {
        "find_replace"
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new("find_replace.find", "Find text", |editor, args| {
                let query = args
                    .as_ref()
                    .and_then(|v| v.get("query"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| CommandError::new("missing args.query"))?;
                let case_sensitive = args
                    .as_ref()
                    .and_then(|v| v.get("case_sensitive"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let matches = find_text_matches(editor.doc(), editor.registry(), query, case_sensitive);
                if let Some(first) = matches.first() {
                    let selection = Selection {
                        anchor: first.start.clone(),
                        focus: first.end.clone(),
                    };
                    editor.set_selection(selection);
                }
                Ok(())
            })
            .description("Find text in the document and select the first match.")
            .keywords(["find", "search"])
            .args_example(serde_json::json!({ "query": "hello", "case_sensitive": false })),

            CommandSpec::new("find_replace.replace", "Replace text", |editor, args| {
                let query = args
                    .as_ref()
                    .and_then(|v| v.get("query"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| CommandError::new("missing args.query"))?;
                let replacement = args
                    .as_ref()
                    .and_then(|v| v.get("replacement"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| CommandError::new("missing args.replacement"))?;
                let case_sensitive = args
                    .as_ref()
                    .and_then(|v| v.get("case_sensitive"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let matches = find_text_matches(editor.doc(), editor.registry(), query, case_sensitive);
                if let Some(first) = matches.first() {
                    let tx = build_replace_transaction(first, replacement)
                        .source("command:find_replace.replace");
                    editor
                        .apply(tx)
                        .map_err(|e| CommandError::new(format!("failed to replace: {e:?}")))?;
                }
                Ok(())
            })
            .description("Replace the first occurrence of text.")
            .keywords(["replace", "substitute"])
            .args_example(serde_json::json!({ "query": "hello", "replacement": "world", "case_sensitive": false })),

            CommandSpec::new("find_replace.replace_all", "Replace all text", |editor, args| {
                let query = args
                    .as_ref()
                    .and_then(|v| v.get("query"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| CommandError::new("missing args.query"))?;
                let replacement = args
                    .as_ref()
                    .and_then(|v| v.get("replacement"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| CommandError::new("missing args.replacement"))?;
                let case_sensitive = args
                    .as_ref()
                    .and_then(|v| v.get("case_sensitive"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let mut matches = find_text_matches(editor.doc(), editor.registry(), query, case_sensitive);
                // Process matches in reverse order to avoid path invalidation
                matches.reverse();

                for m in &matches {
                    let tx = build_replace_transaction(m, replacement)
                        .source("command:find_replace.replace_all");
                    editor
                        .apply(tx)
                        .map_err(|e| CommandError::new(format!("failed to replace: {e:?}")))?;
                }
                Ok(())
            })
            .description("Replace all occurrences of text.")
            .keywords(["replace", "substitute", "all"])
            .args_example(serde_json::json!({ "query": "hello", "replacement": "world", "case_sensitive": false })),
        ]
    }

    fn queries(&self) -> Vec<QuerySpec> {
        vec![QuerySpec {
            id: "find_replace.matches".to_string(),
            handler: std::sync::Arc::new(|editor, args| {
                let query = args
                    .as_ref()
                    .and_then(|v| v.get("query"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| QueryError::new("missing args.query"))?;
                let case_sensitive = args
                    .as_ref()
                    .and_then(|v| v.get("case_sensitive"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let matches = find_text_matches(editor.doc(), editor.registry(), query, case_sensitive);
                let result: Vec<serde_json::Value> = matches
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "start": { "path": m.start.path, "offset": m.start.offset },
                            "end": { "path": m.end.path, "offset": m.end.offset },
                            "block_path": m.block_path,
                            "text": m.text,
                        })
                    })
                    .collect();
                Ok(serde_json::Value::Array(result))
            }),
        }]
    }
}

#[derive(Debug, Clone)]
struct TextMatch {
    start: Point,
    end: Point,
    block_path: Path,
    text: String,
}

fn find_text_matches(
    doc: &Document,
    registry: &PluginRegistry,
    query: &str,
    case_sensitive: bool,
) -> Vec<TextMatch> {
    if query.is_empty() {
        return Vec::new();
    }

    let blocks = super::text_blocks_in_order(doc, registry);
    let mut matches = Vec::new();

    let query_normalized = if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };

    for block in &blocks {
        let block_text = collect_block_text(&block.el.children);
        let search_text = if case_sensitive {
            block_text.clone()
        } else {
            block_text.to_lowercase()
        };

        let mut search_start = 0;
        while let Some(pos) = search_text[search_start..].find(&query_normalized) {
            let global_start = search_start + pos;
            let global_end = global_start + query.len();

            let start_point = global_offset_to_point(&block.path, &block.el.children, global_start);
            let end_point = global_offset_to_point(&block.path, &block.el.children, global_end);

            matches.push(TextMatch {
                start: start_point,
                end: end_point,
                block_path: block.path.clone(),
                text: block_text[global_start..global_end].to_string(),
            });

            search_start = global_start + 1;
        }
    }

    matches
}

fn collect_block_text(children: &[Node]) -> String {
    let mut text = String::new();
    for node in children {
        match node {
            Node::Text(t) => text.push_str(&t.text),
            Node::Void(v) => text.push_str(&v.inline_text()),
            Node::Element(_) => {}
        }
    }
    text
}

fn global_offset_to_point(block_path: &Path, children: &[Node], global_offset: usize) -> Point {
    let mut remaining = global_offset;

    for (ix, node) in children.iter().enumerate() {
        let node_len = match node {
            Node::Text(t) => t.text.len(),
            Node::Void(v) => v.inline_text_len(),
            Node::Element(_) => 0,
        };

        if remaining <= node_len {
            let mut path = block_path.clone();
            path.push(ix);
            return Point {
                path,
                offset: remaining,
            };
        }
        remaining -= node_len;
    }

    // Fallback to end of last node
    let last_ix = children.len().saturating_sub(1);
    let last_len = children.last().map_or(0, |n| match n {
        Node::Text(t) => t.text.len(),
        Node::Void(v) => v.inline_text_len(),
        Node::Element(_) => 0,
    });
    let mut path = block_path.clone();
    path.push(last_ix);
    Point {
        path,
        offset: last_len,
    }
}

fn build_replace_transaction(m: &TextMatch, replacement: &str) -> Transaction {
    let query_len = m.text.len();
    let start_offset = m.start.offset;

    Transaction::new(vec![
        Op::RemoveText {
            path: m.start.path.clone(),
            range: start_offset..(start_offset + query_len),
        },
        Op::InsertText {
            path: m.start.path.clone(),
            offset: start_offset,
            text: replacement.to_string(),
        },
    ])
    .selection_after(Selection::collapsed(Point::new(
        m.start.path.clone(),
        start_offset + replacement.len(),
    )))
}
