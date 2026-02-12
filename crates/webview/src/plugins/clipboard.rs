//! Clipboard plugin - clipboard operations using arboard

use crate::ipc::{bad_request, internal_error, ok_json};
use arboard::Clipboard;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteTextOptions {
    pub text: String,
}

pub fn read_text(_request: http::Request<Vec<u8>>) -> http::Response<Vec<u8>> {
    let mut clipboard = match Clipboard::new() {
        Ok(c) => c,
        Err(err) => return internal_error(format!("failed to access clipboard: {err}")),
    };

    match clipboard.get_text() {
        Ok(text) => ok_json(&text),
        Err(err) => internal_error(format!("failed to read clipboard: {err}")),
    }
}

pub fn write_text(request: http::Request<Vec<u8>>) -> http::Response<Vec<u8>> {
    let options: WriteTextOptions = match serde_json::from_slice(request.body()) {
        Ok(opts) => opts,
        Err(err) => return bad_request(format!("invalid options: {err}")),
    };

    let mut clipboard = match Clipboard::new() {
        Ok(c) => c,
        Err(err) => return internal_error(format!("failed to access clipboard: {err}")),
    };

    match clipboard.set_text(&options.text) {
        Ok(()) => ok_json(&()),
        Err(err) => internal_error(format!("failed to write clipboard: {err}")),
    }
}

/// Returns the handlers for the clipboard plugin
pub fn handlers() -> Vec<(
    String,
    std::sync::Arc<dyn Fn(http::Request<Vec<u8>>) -> http::Response<Vec<u8>> + Send + Sync>,
)> {
    vec![
        (
            "plugin:clipboard-manager|read-text".to_string(),
            std::sync::Arc::new(read_text),
        ),
        (
            "plugin:clipboard-manager|write-text".to_string(),
            std::sync::Arc::new(write_text),
        ),
    ]
}
