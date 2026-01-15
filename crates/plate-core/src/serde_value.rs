use serde::{Deserialize, Serialize};

use crate::core::Document;

const DEFAULT_SCHEMA: &str = "gpui-plate";
const DEFAULT_VERSION: u32 = 1;

fn default_schema() -> String {
    DEFAULT_SCHEMA.to_string()
}

fn default_version() -> u32 {
    DEFAULT_VERSION
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlateValue {
    #[serde(default = "default_schema")]
    pub schema: String,
    #[serde(default = "default_version")]
    pub version: u32,
    pub document: Document,
}

impl PlateValue {
    pub fn from_document(document: Document) -> Self {
        Self {
            schema: default_schema(),
            version: default_version(),
            document,
        }
    }

    pub fn into_document(self) -> Document {
        self.document
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json_str(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}
