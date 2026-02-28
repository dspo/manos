//! Opener plugin - open URLs and paths using system default applications

use crate::ipc::{bad_request, internal_error, ok_json};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenUrlOptions {
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenPathOptions {
    pub path: String,
}

pub fn open_url(request: http::Request<Vec<u8>>) -> http::Response<Vec<u8>> {
    let options: OpenUrlOptions = match serde_json::from_slice(request.body()) {
        Ok(opts) => opts,
        Err(err) => return bad_request(format!("invalid options: {err}")),
    };

    match open::that(&options.url) {
        Ok(()) => ok_json(&()),
        Err(err) => internal_error(format!("failed to open url: {err}")),
    }
}

pub fn open_path(request: http::Request<Vec<u8>>) -> http::Response<Vec<u8>> {
    let options: OpenPathOptions = match serde_json::from_slice(request.body()) {
        Ok(opts) => opts,
        Err(err) => return bad_request(format!("invalid options: {err}")),
    };

    match open::that(&options.path) {
        Ok(()) => ok_json(&()),
        Err(err) => internal_error(format!("failed to open path: {err}")),
    }
}

/// Returns the handlers for the opener plugin
pub fn handlers() -> Vec<(
    String,
    std::sync::Arc<dyn Fn(http::Request<Vec<u8>>) -> http::Response<Vec<u8>> + Send + Sync>,
)> {
    vec![
        (
            "plugin:opener|open-url".to_string(),
            std::sync::Arc::new(open_url),
        ),
        (
            "plugin:opener|open-path".to_string(),
            std::sync::Arc::new(open_path),
        ),
    ]
}
