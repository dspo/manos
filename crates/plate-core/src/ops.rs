use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::core::{AttrPatch, Node, Selection};

pub type Path = Vec<usize>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    InsertText {
        #[serde(default)]
        path: Path,
        offset: usize,
        text: String,
    },
    RemoveText {
        #[serde(default)]
        path: Path,
        range: Range<usize>,
    },
    InsertNode {
        #[serde(default)]
        path: Path,
        node: Node,
    },
    RemoveNode {
        #[serde(default)]
        path: Path,
    },
    SetNodeAttrs {
        #[serde(default)]
        path: Path,
        patch: AttrPatch,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TransactionMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    #[serde(default)]
    pub ops: Vec<Op>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_after: Option<Selection>,
    #[serde(default)]
    pub meta: TransactionMeta,
}

impl Transaction {
    pub fn new(ops: Vec<Op>) -> Self {
        Self {
            ops,
            selection_after: None,
            meta: TransactionMeta::default(),
        }
    }

    pub fn selection_after(mut self, selection_after: Selection) -> Self {
        self.selection_after = Some(selection_after);
        self
    }

    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.meta.source = Some(source.into());
        self
    }
}
