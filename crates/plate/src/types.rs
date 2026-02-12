use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone)]
pub struct CommandInfo {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub keywords: Vec<String>,
    pub args_example: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EmbedLocalImagesReport {
    pub embedded: usize,
    pub skipped: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BundleExportReport {
    pub bundled: usize,
    pub bundled_data_url: usize,
    pub skipped: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleAsset {
    pub relative_path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CollectAssetsReport {
    pub rewritten: usize,
    pub rewritten_data_url: usize,
    pub assets_written: usize,
    pub skipped: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortabilityIssueLevel {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortabilityIssue {
    pub level: PortabilityIssueLevel,
    pub src: String,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct PortabilityReport {
    pub base_dir: Option<PathBuf>,
    pub images_total: usize,
    pub images_data_url: usize,
    pub images_http: usize,
    pub images_resource: usize,
    pub images_absolute_ok: usize,
    pub images_absolute_missing: usize,
    pub images_relative_ok: usize,
    pub images_relative_missing: usize,
    pub issues: Vec<PortabilityIssue>,
}

impl PortabilityReport {
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.level == PortabilityIssueLevel::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.level == PortabilityIssueLevel::Warning)
            .count()
    }
}
