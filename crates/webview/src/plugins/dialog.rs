//! Dialog plugin - file picker dialogs using rfd

use crate::ipc::{bad_request, ok_json};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenDialogOptions {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub default_path: Option<String>,
    #[serde(default)]
    pub directory: bool,
    #[serde(default)]
    pub multiple: bool,
    #[serde(default)]
    pub filters: Vec<DialogFilter>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDialogOptions {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub default_path: Option<String>,
    #[serde(default)]
    pub filters: Vec<DialogFilter>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

/// Result from an open dialog.
///
/// When the user cancels the dialog, `Single(None)` or `Multiple(vec![])` is returned.
/// This matches Tauri's behavior where cancellation is not an error.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum OpenDialogResult {
    Single(Option<String>),
    Multiple(Vec<String>),
}

/// Result from a save dialog.
///
/// `None` indicates the user cancelled the dialog.
pub type SaveDialogResult = Option<String>;

pub fn open(request: http::Request<Vec<u8>>) -> http::Response<Vec<u8>> {
    let options: OpenDialogOptions = match serde_json::from_slice(request.body()) {
        Ok(opts) => opts,
        Err(err) => return bad_request(format!("invalid options: {err}")),
    };

    let mut dialog = rfd::FileDialog::new();

    if let Some(title) = &options.title {
        dialog = dialog.set_title(title);
    }

    if let Some(default_path) = &options.default_path {
        dialog = dialog.set_directory(default_path);
    }

    for filter in &options.filters {
        let extensions: Vec<&str> = filter.extensions.iter().map(|s| s.as_str()).collect();
        dialog = dialog.add_filter(&filter.name, &extensions);
    }

    // rfd returns None when user cancels - this is expected behavior, not an error.
    // We pass through None/empty results to match Tauri's API behavior.
    let result = if options.directory {
        if options.multiple {
            let paths = dialog.pick_folders();
            OpenDialogResult::Multiple(
                paths
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect(),
            )
        } else {
            let path = dialog.pick_folder();
            OpenDialogResult::Single(path.map(|p| p.to_string_lossy().into_owned()))
        }
    } else if options.multiple {
        let paths = dialog.pick_files();
        OpenDialogResult::Multiple(
            paths
                .unwrap_or_default()
                .into_iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
        )
    } else {
        let path = dialog.pick_file();
        OpenDialogResult::Single(path.map(|p| p.to_string_lossy().into_owned()))
    };

    ok_json(&result)
}

pub fn save(request: http::Request<Vec<u8>>) -> http::Response<Vec<u8>> {
    let options: SaveDialogOptions = match serde_json::from_slice(request.body()) {
        Ok(opts) => opts,
        Err(err) => return bad_request(format!("invalid options: {err}")),
    };

    let mut dialog = rfd::FileDialog::new();

    if let Some(title) = &options.title {
        dialog = dialog.set_title(title);
    }

    if let Some(default_path) = &options.default_path {
        dialog = dialog.set_file_name(default_path);
    }

    for filter in &options.filters {
        let extensions: Vec<&str> = filter.extensions.iter().map(|s| s.as_str()).collect();
        dialog = dialog.add_filter(&filter.name, &extensions);
    }

    // rfd returns None when user cancels - this is expected behavior, not an error.
    let path = dialog.save_file();
    let result: SaveDialogResult = path.map(|p| p.to_string_lossy().into_owned());

    ok_json(&result)
}

/// Returns the handlers for the dialog plugin
pub fn handlers() -> Vec<(
    String,
    std::sync::Arc<dyn Fn(http::Request<Vec<u8>>) -> http::Response<Vec<u8>> + Send + Sync>,
)> {
    vec![
        (
            "plugin:dialog|open".to_string(),
            std::sync::Arc::new(open),
        ),
        (
            "plugin:dialog|save".to_string(),
            std::sync::Arc::new(save),
        ),
    ]
}
