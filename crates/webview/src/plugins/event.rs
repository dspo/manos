//! Event plugin - implements Tauri's event system for listen/unlisten
//!
//! This plugin handles the `plugin:event|listen` and `plugin:event|unlisten` commands
//! that are used by `@tauri-apps/api/event`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE};
use http::{Response, StatusCode};
use serde::{Deserialize, Serialize};

/// Global counter for generating unique event IDs
static NEXT_EVENT_ID: AtomicU32 = AtomicU32::new(0);

/// Default label used when no webview label is provided
const DEFAULT_WEBVIEW_LABEL: &str = "__default__";

/// Global registry of JS event listeners, keyed by webview label
static LISTENERS: OnceLock<RwLock<HashMap<String, Vec<JsListener>>>> = OnceLock::new();

/// Represents a registered JS event listener
#[derive(Debug, Clone)]
struct JsListener {
    event_id: u32,
    event: String,
    handler_id: u32,
}

fn listeners() -> &'static RwLock<HashMap<String, Vec<JsListener>>> {
    LISTENERS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Get all handler IDs registered for a specific event
///
/// # Arguments
/// * `event` - The event name to look up
/// * `webview_label` - Optional webview label. If None, uses the default label.
pub fn get_handler_ids_for_event(event: &str, webview_label: Option<&str>) -> Vec<u32> {
    let label = webview_label.unwrap_or(DEFAULT_WEBVIEW_LABEL);
    let Ok(listeners) = listeners().read() else {
        return Vec::new();
    };
    listeners
        .get(label)
        .map(|list| {
            list.iter()
                .filter(|l| l.event == event)
                .map(|l| l.handler_id)
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListenRequest {
    event: String,
    handler: u32,
    #[serde(default)]
    webview_label: Option<String>,
}

/// Response returns event_id (not handler_id) for unlisten support
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListenResponse {
    event_id: u32,
}

/// Handle the plugin:event|listen command
/// This registers an event listener and returns an event_id for unlisten
fn handle_listen(request: http::Request<Vec<u8>>) -> Response<Vec<u8>> {
    let body = request.body();

    match serde_json::from_slice::<ListenRequest>(body) {
        Ok(req) => {
            // Generate unique event_id
            let event_id = NEXT_EVENT_ID.fetch_add(1, Ordering::Relaxed);

            // Determine webview label
            let label = req
                .webview_label
                .unwrap_or_else(|| DEFAULT_WEBVIEW_LABEL.to_string());

            // Store listener in global registry
            let listener = JsListener {
                event_id,
                event: req.event.clone(),
                handler_id: req.handler,
            };

            {
                let Ok(mut listeners) = listeners().write() else {
                    return Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header(CONTENT_TYPE, "text/plain")
                        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                        .body("failed to acquire lock".as_bytes().to_vec())
                        .unwrap();
                };
                listeners.entry(label).or_default().push(listener);
            }

            // Return event_id (frontend uses this for unlisten)
            let response = ListenResponse { event_id };
            response_json(StatusCode::OK, &response)
        }
        Err(e) => Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(CONTENT_TYPE, "text/plain")
            .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(format!("Invalid request: {}", e).into_bytes())
            .unwrap(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnlistenRequest {
    event: String,
    event_id: u32,
    #[serde(default)]
    webview_label: Option<String>,
}

/// Handle the plugin:event|unlisten command
fn handle_unlisten(request: http::Request<Vec<u8>>) -> Response<Vec<u8>> {
    let body = request.body();

    match serde_json::from_slice::<UnlistenRequest>(body) {
        Ok(req) => {
            // Determine webview label
            let label = req
                .webview_label
                .unwrap_or_else(|| DEFAULT_WEBVIEW_LABEL.to_string());

            // Remove listener from global registry
            if let Ok(mut listeners) = listeners().write() {
                if let Some(list) = listeners.get_mut(&label) {
                    list.retain(|l| !(l.event == req.event && l.event_id == req.event_id));
                }
            }
            response_json(StatusCode::OK, &serde_json::json!({}))
        }
        Err(_) => {
            // Fallback: just acknowledge (for backwards compatibility)
            response_json(StatusCode::OK, &serde_json::json!({}))
        }
    }
}

pub fn handlers() -> HashMap<
    String,
    Arc<dyn Fn(http::Request<Vec<u8>>) -> Response<Vec<u8>> + Send + Sync + 'static>,
> {
    let mut handlers: HashMap<
        String,
        Arc<dyn Fn(http::Request<Vec<u8>>) -> Response<Vec<u8>> + Send + Sync + 'static>,
    > = HashMap::new();

    handlers.insert("plugin:event|listen".to_string(), Arc::new(handle_listen));

    handlers.insert(
        "plugin:event|unlisten".to_string(),
        Arc::new(handle_unlisten),
    );

    handlers
}

fn response_json<T: Serialize>(status: StatusCode, data: &T) -> Response<Vec<u8>> {
    let body = serde_json::to_vec(data).unwrap_or_default();
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(body)
        .unwrap()
}
