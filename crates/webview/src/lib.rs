pub mod webview;
pub use http;
pub use serde;
pub use serde_json;
pub use wry;

use http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serialize_to_javascript::{DefaultTemplate, Template, default_template};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use wry::{Error as WryError, Result, WebView, WebViewBuilder, WebViewId};

pub use gpui_manos_webview_macros::{
    api_handler, api_handlers, command, command_handler, command_handlers, generate_handler,
};

// todo: implement dev server

const INVOKE_KEY: &str = "gpui";

pub mod async_runtime {
    use std::future::Future;

    pub fn block_on<F: Future>(future: F) -> F::Output {
        pollster::block_on(future)
    }
}

thread_local! {
    static IPC_WEBVIEWS: RefCell<HashMap<String, Weak<wry::WebView>>> =
        RefCell::new(HashMap::new());
}

pub(crate) fn register_webview_for_ipc(webview: &Rc<wry::WebView>) {
    let id = webview.id().to_string();
    IPC_WEBVIEWS.with(|registry| {
        registry.borrow_mut().insert(id, Rc::downgrade(webview));
    });
}

pub(crate) fn unregister_webview_for_ipc(webview_id: &str) {
    IPC_WEBVIEWS.with(|registry| {
        registry.borrow_mut().remove(webview_id);
    });
}

fn ipc_webview_for_label(webview_label: Option<&str>) -> Option<Rc<wry::WebView>> {
    IPC_WEBVIEWS.with(|registry| {
        let weak = {
            let registry = registry.borrow();
            if let Some(label) = webview_label {
                registry.get(label).cloned()
            } else if registry.len() == 1 {
                registry.values().next().cloned()
            } else {
                None
            }
        };

        weak.and_then(|w| w.upgrade())
    })
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PostMessageOptions {
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    custom_protocol_ipc_blocked: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostMessageRequest {
    cmd: String,
    callback: u32,
    error: u32,
    #[serde(default)]
    payload: serde_json::Value,
    #[serde(default)]
    options: Option<PostMessageOptions>,
    #[serde(rename = "__TAURI_INVOKE_KEY__")]
    invoke_key: String,
    #[serde(default)]
    webview_label: Option<String>,
}

pub struct Invoke {
    pub command: String,
    pub request: http::Request<Vec<u8>>,
}

pub type InvokeHandler =
    Arc<dyn Fn(Invoke) -> Option<http::Response<Vec<u8>>> + Send + Sync + 'static>;

pub struct Builder<'a> {
    builder: WebViewBuilder<'a>,
    webview_id: WebViewId<'a>,
    invoke_handler: Option<InvokeHandler>,
    handlers: HashMap<
        String,
        Arc<dyn Fn(http::Request<Vec<u8>>) -> http::Response<Vec<u8>> + Send + Sync + 'static>,
    >,
}

impl<'a> Builder<'a> {
    pub fn new() -> Self {
        Builder {
            builder: WebViewBuilder::new(),
            webview_id: WebViewId::default(),
            invoke_handler: None,
            handlers: HashMap::new(),
        }
    }

    pub fn with_webview_id(mut self, webview_id: WebViewId<'a>) -> Self {
        self.webview_id = webview_id;
        self
    }

    pub fn apply<F>(mut self, f: F) -> Self
    where
        F: FnOnce(WebViewBuilder<'a>) -> WebViewBuilder<'a>,
    {
        self.builder = f(self.builder);
        self
    }

    /// Registers a single low-level HTTP handler (used by the `ipc://` custom protocol).
    pub fn serve_api<F>(mut self, api: (String, F)) -> Self
    where
        F: Fn(http::Request<Vec<u8>>) -> http::Response<Vec<u8>> + Send + Sync + 'static,
    {
        let (name, f) = api;
        self.handlers.insert(name, Arc::new(f));

        self
    }

    /// Registers an invoke handler, similar to Tauri's `Builder::invoke_handler`.
    ///
    /// Typically used with `gpui_manos_webview::generate_handler![...]`.
    pub fn invoke_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(Invoke) -> Option<http::Response<Vec<u8>>> + Send + Sync + 'static,
    {
        self.invoke_handler = Some(Arc::new(handler));
        self
    }

    pub fn serve_apis<I, F>(mut self, apis: I) -> Self
    where
        I: IntoIterator<Item = (String, F)>,
        F: Fn(http::Request<Vec<u8>>) -> http::Response<Vec<u8>> + Send + Sync + 'static,
    {
        for (name, f) in apis {
            self.handlers.insert(name, Arc::new(f));
        }

        self
    }

    // todo: implement fallback to ipc

    // todo: implement channel for performance

    pub fn build_as_child(self, window: &mut gpui::Window) -> Result<WebView> {
        if self.webview_id.is_empty() {
            return Result::Err(WryError::InitScriptError);
        }

        use raw_window_handle::HasWindowHandle;

        let window_handle = window.window_handle()?;
        let webview_id = self.webview_id;
        self.with_initialization_script_for_main_only()
            .with_apis()
            .builder
            .with_id(webview_id)
            .build_as_child(&window_handle)
    }

    pub fn webview_builder(self) -> WebViewBuilder<'a> {
        self.builder
    }

    // todo: implement more professional serve static
    pub fn serve_static<S: ToString + 'static>(self, static_root: S) -> Self {
        let static_root = static_root.to_string();
        self.apply(move |b| {
            let static_root_for_asset = static_root.clone();
            let static_root_for_wry = static_root.clone();

            b.with_asynchronous_custom_protocol(
                "asset".into(),
                move |webview_id, request, responder| {
                    let response = serve_static(webview_id, static_root_for_asset.clone(), request)
                        .unwrap_or_else(response_internal_server_err);
                    responder.respond(response)
                },
            )
            .with_asynchronous_custom_protocol(
                "wry".into(),
                move |webview_id, request, responder| {
                    let response = serve_static(webview_id, static_root_for_wry.clone(), request)
                        .unwrap_or_else(response_internal_server_err);
                    responder.respond(response)
                },
            )
            .with_url("asset://localhost")
        })
    }

    fn with_initialization_script_for_main_only(mut self) -> Self {
        // todo: fix mocked_window_id and webview_id
        let scripts = prepare_scripts(
            String::from("mocked_window_id"),
            self.webview_id.to_string(),
        )
        .unwrap();
        for s in scripts {
            self = self.apply(|b| b.with_initialization_script_for_main_only(s.script, true));
        }
        self
    }

    fn with_apis(self) -> Self {
        let handlers = self.handlers.clone();
        let invoke_handler = self.invoke_handler.clone();
        self.apply(move |b| {
            let handlers_for_post_message = handlers.clone();
            let invoke_handler_for_post_message = invoke_handler.clone();
            b.with_ipc_handler(move |request: http::Request<String>| {
                let message: PostMessageRequest = match serde_json::from_str(request.body()) {
                    Ok(message) => message,
                    Err(err) => {
                        eprintln!("[webview] invalid IPC postMessage payload: {err}");
                        return;
                    }
                };

                if message.invoke_key != INVOKE_KEY {
                    eprintln!("[webview] rejected IPC postMessage with invalid invoke key");
                    return;
                }

                let payload_bytes = match serde_json::to_vec(&message.payload) {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        eprintln!("[webview] failed to serialize IPC payload: {err}");
                        return;
                    }
                };

                let mut request_builder = http::Request::builder()
                    .method(http::Method::POST)
                    .uri("ipc://localhost");

                let PostMessageOptions {
                    headers,
                    custom_protocol_ipc_blocked: _custom_protocol_ipc_blocked,
                } = message.options.unwrap_or_default();

                for (key, value) in headers {
                    if let (Ok(name), Ok(value)) = (
                        http::header::HeaderName::from_bytes(key.as_bytes()),
                        http::HeaderValue::from_str(&value),
                    ) {
                        request_builder = request_builder.header(name, value);
                    }
                }

                let request = match request_builder.body(payload_bytes) {
                    Ok(request) => request,
                    Err(err) => {
                        eprintln!("[webview] failed to build IPC request: {err}");
                        return;
                    }
                };

                let cmd = message.cmd;
                let response = if let Some(ref handler) = invoke_handler_for_post_message {
                    handler(Invoke {
                        command: cmd.clone(),
                        request,
                    })
                    .unwrap_or_else(|| ipc::not_found(cmd.clone()))
                } else if let Some(handler) = handlers_for_post_message.get(&cmd) {
                    handler(request)
                } else {
                    ipc::not_found(cmd.clone())
                };

                let (parts, body) = response.into_parts();
                let response_header = parts
                    .headers
                    .get("Tauri-Response")
                    .and_then(|value| value.to_str().ok());
                let callback_id = if response_header == Some("ok") {
                    message.callback
                } else {
                    message.error
                };

                let content_type = parts
                    .headers
                    .get(CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default();
                let content_type = content_type.split(',').next().unwrap_or_default();

                let data = match content_type {
                    "application/json" => serde_json::from_slice::<serde_json::Value>(&body)
                        .unwrap_or_else(|_| {
                            serde_json::Value::String(String::from_utf8_lossy(&body).into_owned())
                        }),
                    "text/plain" => {
                        serde_json::Value::String(String::from_utf8_lossy(&body).into_owned())
                    }
                    _ => serde_json::to_value(body).unwrap_or(serde_json::Value::Null),
                };

                let data = serde_json::to_string(&data).unwrap_or_else(|_| "null".to_string());
                let js = format!(
                    "window.__TAURI_INTERNALS__.runCallback({callback_id}, {data});"
                );

                let Some(webview) = ipc_webview_for_label(message.webview_label.as_deref())
                else {
                    eprintln!(
                        "[webview] IPC postMessage fallback used but no webview is registered; cannot run callback for `{cmd}`"
                    );
                    return;
                };

                let _ = webview.evaluate_script(&js);
            })
            .with_asynchronous_custom_protocol(
                "ipc".into(),
                move |webview_id, request, responder| {
                    fn respond(
                        responder: wry::RequestAsyncResponder,
                        mut response: http::Response<Vec<u8>>,
                    ) {
                        response.headers_mut().insert(
                            http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
                            http::HeaderValue::from_static("*"),
                        );
                        response.headers_mut().insert(
                            http::header::ACCESS_CONTROL_EXPOSE_HEADERS,
                            http::HeaderValue::from_static("Tauri-Response"),
                        );
                        responder.respond(response);
                    }

                    println!(
                        "webview_id: {}, method: {}, scheme: {:?}, host: {:?}, path: {:?}",
                        webview_id,
                        request.method(),
                        request.uri().scheme(),
                        request.uri().host(),
                        request.uri().path()
                    );

                    match *request.method() {
                        http::Method::POST => {}
                        http::Method::OPTIONS => {
                            let mut response = http::Response::new(Vec::new());
                            response.headers_mut().insert(
                                http::header::ACCESS_CONTROL_ALLOW_HEADERS,
                                http::HeaderValue::from_static("*"),
                            );
                            respond(responder, response);
                            return;
                        }
                        _ => {
                            let mut response =
                                http::Response::new("only POST and OPTIONS are allowed".into());
                            *response.status_mut() = http::StatusCode::METHOD_NOT_ALLOWED;
                            response.headers_mut().insert(
                                http::header::CONTENT_TYPE,
                                http::HeaderValue::from_static("text/plain"),
                            );
                            respond(responder, response);
                            return;
                        }
                    }

                    let invoke_key = request
                        .headers()
                        .get("Tauri-Invoke-Key")
                        .and_then(|value| value.to_str().ok());
                    if invoke_key != Some(INVOKE_KEY) {
                        respond(responder, ipc::bad_request("invalid invoke key"));
                        return;
                    }

                    let raw_path = request.uri().path().to_string();
                    let command = decode_uri_component(
                        raw_path.strip_prefix('/').unwrap_or(raw_path.as_str()),
                    );

                    let invoke_handler = invoke_handler.clone();
                    let api_handler = handlers.get(&command).cloned();

                    std::thread::spawn(move || {
                        let response = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                            || {
                                if let Some(handler) = invoke_handler {
                                    handler(Invoke { command, request })
                                        .unwrap_or_else(|| ipc::not_found(raw_path))
                                } else if let Some(handler) = api_handler {
                                    handler(request)
                                } else {
                                    ipc::not_found(raw_path)
                                }
                            },
                        ))
                        .unwrap_or_else(|_| ipc::internal_error("invoke handler panicked"));

                        respond(responder, response);
                    });
                },
            )
        })
    }
}

fn decode_uri_component(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = from_hex(bytes[i + 1]);
            let lo = from_hex(bytes[i + 2]);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }

    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub mod ipc {
    use super::*;
    use http::HeaderValue;

    #[derive(Debug)]
    pub struct Request {
        parts: http::request::Parts,
        body: Vec<u8>,
    }

    impl Request {
        pub fn new(parts: http::request::Parts, body: Vec<u8>) -> Self {
            Self { parts, body }
        }

        pub fn method(&self) -> &http::Method {
            &self.parts.method
        }

        pub fn uri(&self) -> &http::Uri {
            &self.parts.uri
        }

        pub fn headers(&self) -> &http::HeaderMap {
            &self.parts.headers
        }

        pub fn body(&self) -> &[u8] {
            &self.body
        }

        pub fn into_body(self) -> Vec<u8> {
            self.body
        }
    }

    #[derive(Debug)]
    pub struct Response {
        body: Vec<u8>,
        content_type: String,
    }

    impl Response {
        pub fn new(body: impl Into<Vec<u8>>, content_type: impl Into<String>) -> Self {
            Self {
                body: body.into(),
                content_type: content_type.into(),
            }
        }

        pub fn binary(body: impl Into<Vec<u8>>) -> Self {
            Self::new(body, "application/octet-stream")
        }

        fn into_http_response(self) -> http::Response<Vec<u8>> {
            let mut builder = response_builder(http::StatusCode::OK, "ok");
            builder = builder.header(
                CONTENT_TYPE,
                HeaderValue::from_str(&self.content_type)
                    .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
            );
            builder.body(self.body).unwrap()
        }
    }

    pub trait IntoInvokeResponse {
        fn into_invoke_response(self) -> http::Response<Vec<u8>>;
    }

    impl<T: serde::Serialize> IntoInvokeResponse for T {
        fn into_invoke_response(self) -> http::Response<Vec<u8>> {
            ok_json(&self)
        }
    }

    impl IntoInvokeResponse for Response {
        fn into_invoke_response(self) -> http::Response<Vec<u8>> {
            self.into_http_response()
        }
    }

    pub fn respond<T: IntoInvokeResponse>(value: T) -> http::Response<Vec<u8>> {
        value.into_invoke_response()
    }

    fn response_builder(
        status_code: http::StatusCode,
        tauri_response: &'static str,
    ) -> http::response::Builder {
        http::Response::builder()
            .status(status_code)
            .header(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"))
            .header(
                http::header::ACCESS_CONTROL_EXPOSE_HEADERS,
                "Tauri-Response",
            )
            .header("Tauri-Response", HeaderValue::from_static(tauri_response))
    }

    pub fn ok_json<T: serde::Serialize>(value: &T) -> http::Response<Vec<u8>> {
        match serde_json::to_vec(value) {
            Ok(body) => response_builder(http::StatusCode::OK, "ok")
                .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                .body(body)
                .unwrap(),
            Err(err) => internal_error(err),
        }
    }

    pub fn bad_request_json<T: serde::Serialize>(value: &T) -> http::Response<Vec<u8>> {
        match serde_json::to_vec(value) {
            Ok(body) => response_builder(http::StatusCode::BAD_REQUEST, "error")
                .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                .body(body)
                .unwrap(),
            Err(err) => internal_error(err),
        }
    }

    pub fn bad_request<S: ToString>(message: S) -> http::Response<Vec<u8>> {
        response_builder(http::StatusCode::BAD_REQUEST, "error")
            .header(CONTENT_TYPE, HeaderValue::from_static("text/plain"))
            .body(message.to_string().into_bytes())
            .unwrap()
    }

    pub fn internal_error_json<T: serde::Serialize>(value: &T) -> http::Response<Vec<u8>> {
        match serde_json::to_vec(value) {
            Ok(body) => response_builder(http::StatusCode::INTERNAL_SERVER_ERROR, "error")
                .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                .body(body)
                .unwrap(),
            Err(err) => internal_error(err),
        }
    }

    pub fn internal_error<S: ToString>(message: S) -> http::Response<Vec<u8>> {
        response_builder(http::StatusCode::INTERNAL_SERVER_ERROR, "error")
            .header(CONTENT_TYPE, HeaderValue::from_static("text/plain"))
            .body(message.to_string().into_bytes())
            .unwrap()
    }

    pub fn not_found<S: ToString>(message: S) -> http::Response<Vec<u8>> {
        response_builder(http::StatusCode::NOT_FOUND, "error")
            .header(CONTENT_TYPE, HeaderValue::from_static("text/plain"))
            .body(format!("{} not found", message.to_string()).into_bytes())
            .unwrap()
    }
}

// todo: this is too simple, refactor it like Tauri
fn serve_static<S: ToString>(
    webview_id: WebViewId,
    static_path: S,
    request: http::Request<Vec<u8>>,
) -> http::Result<http::Response<Vec<u8>>> {
    let path = request.uri().path();
    println!(
        "webview_id: {}, method: {:?}, scheme: {:?}, host: {:?}, path: {:?}",
        webview_id,
        request.method(),
        request.uri().scheme(),
        request.uri().host(),
        request.uri().path()
    );

    let static_root = static_path.to_string();
    let root = match fs::canonicalize(&static_root) {
        Ok(root) => root,
        Err(err) => {
            println!("failed to canonicalize static root `{static_root}`: {err}");
            return Ok(response_internal_server_err("static root not accessible"));
        }
    };

    match resolve_static_asset(&root, path) {
        Ok(asset) => response_asset(asset),
        Err(StaticAssetError::NotFound(requested)) => {
            println!("static asset not found: {}", requested.display());
            Ok(response_not_found(requested.display()))
        }
        Err(StaticAssetError::OutsideRoot(requested)) => {
            println!(
                "attempt to read outside static root: {}",
                requested.display()
            );
            Ok(response_forbidden(requested.display()))
        }
        Err(StaticAssetError::IsDirectory(requested)) => {
            println!("requested path is a directory: {}", requested.display());
            Ok(response_forbidden(requested.display()))
        }
        Err(StaticAssetError::Io(err)) => {
            println!("failed to read static asset: {err}");
            Ok(response_internal_server_err("failed to read static asset"))
        }
    }
}

fn response_not_found<S: ToString>(content: S) -> http::Response<Vec<u8>> {
    http::Response::builder()
        .status(http::StatusCode::NOT_FOUND)
        .header(CONTENT_TYPE, "text/plain")
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(format!("{} not found", content.to_string()).into_bytes())
        .unwrap()
}

fn response_internal_server_err<S: ToString>(content: S) -> http::Response<Vec<u8>> {
    http::Response::builder()
        .status(http::StatusCode::INTERNAL_SERVER_ERROR)
        .header(CONTENT_TYPE, "text/plain")
        .body(content.to_string().as_bytes().to_vec())
        .unwrap()
}

struct StaticAsset {
    bytes: Vec<u8>,
    mime: String,
}

enum StaticAssetError {
    NotFound(PathBuf),
    OutsideRoot(PathBuf),
    IsDirectory(PathBuf),
    Io(io::Error),
}

fn resolve_static_asset(
    root: &Path,
    uri_path: &str,
) -> std::result::Result<StaticAsset, StaticAssetError> {
    let relative = sanitize_path(uri_path)?;
    let candidate = root.join(&relative);

    let resolved = match fs::canonicalize(&candidate) {
        Ok(path) => path,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Err(StaticAssetError::NotFound(relative));
        }
        Err(err) => return Err(StaticAssetError::Io(err)),
    };

    if !resolved.starts_with(root) {
        return Err(StaticAssetError::OutsideRoot(relative));
    }

    if resolved.is_dir() {
        return Err(StaticAssetError::IsDirectory(relative));
    }

    let bytes = fs::read(&resolved).map_err(StaticAssetError::Io)?;
    let mime = mime_guess::from_path(&resolved)
        .first_or_octet_stream()
        .essence_str()
        .to_string();

    Ok(StaticAsset { bytes, mime })
}

fn sanitize_path(path: &str) -> std::result::Result<PathBuf, StaticAssetError> {
    let decoded = decode_uri_component(path);
    let trimmed = decoded.trim_start_matches('/');
    let fallback = if trimmed.is_empty() {
        "index.html"
    } else {
        trimmed
    };
    let mut buf = PathBuf::new();

    for component in Path::new(fallback).components() {
        match component {
            Component::Normal(part) => buf.push(part),
            Component::CurDir => continue,
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => {
                return Err(StaticAssetError::OutsideRoot(PathBuf::from(fallback)));
            }
        }
    }

    if buf.as_os_str().is_empty() {
        buf.push("index.html");
    }

    Ok(buf)
}

fn response_asset(asset: StaticAsset) -> http::Result<http::Response<Vec<u8>>> {
    http::Response::builder()
        .status(http::StatusCode::OK)
        .header(CONTENT_TYPE, asset.mime)
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(asset.bytes)
        .map_err(Into::into)
}

fn response_forbidden<S: ToString>(content: S) -> http::Response<Vec<u8>> {
    http::Response::builder()
        .status(http::StatusCode::FORBIDDEN)
        .header(CONTENT_TYPE, "text/plain")
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(format!("{} is not accessible", content.to_string()).into_bytes())
        .unwrap()
}

fn prepare_scripts(
    current_window_label: String,
    current_webview_label: String,
) -> std::result::Result<Vec<InitializationScript>, Box<dyn std::error::Error>> {
    let ipc_init = IpcJavascript {
        isolation_origin: "",
    }
    .render_default(&core::default::Default::default())?;

    let pattern_init = PatternJavascript {
        pattern: PatternObject::Brownfield,
    }
    .render_default(&core::default::Default::default())?;

    let mut list: Vec<InitializationScript> = Vec::new();

    list.push(InitializationScript::main_frame_script(
        r"
        Object.defineProperty(window, 'isTauri', {
          value: true,
        });

        if (!window.__TAURI_INTERNALS__) {
          Object.defineProperty(window, '__TAURI_INTERNALS__', {
            value: {
              plugins: {}
            }
          })
        }
      "
        .to_owned(),
    ));

    list.push(InitializationScript::main_frame_script(format!(
        r#"
          Object.defineProperty(window.__TAURI_INTERNALS__, 'metadata', {{
            value: {{
              currentWindow: {{ label: {current_window_label} }},
              currentWebview: {{ label: {current_webview_label} }}
            }}
          }})
        "#,
    )));

    list.push(InitializationScript::main_frame_script(
        initialization_script(&ipc_init.into_string(), &pattern_init.into_string())?,
    ));

    list.push(InitializationScript::main_frame_script(
        HotkeyZoom {
            os_name: std::env::consts::OS,
        }
        .render_default(&core::default::Default::default())?
        .into_string(),
    ));

    list.push(InitializationScript::main_frame_script(
        InvokeInitializationScript {
            process_ipc_message_fn: include_str!("scripts/tauri/process-ipc-message-fn.js"),
            os_name: std::env::consts::OS,
            fetch_channel_data_command: "plugin:__TAURI_CHANNEL__|fetch",
            invoke_key: INVOKE_KEY,
        }
        .render_default(&core::default::Default::default())?
        .into_string(),
    ));

    Ok(list)
}

/// An initialization script
#[derive(Debug, Clone)]
pub struct InitializationScript {
    /// The script to run
    pub script: String,
    /// Whether the script should be injected to main frame only
    pub for_main_frame_only: bool,
}

impl core::default::Default for InitializationScript {
    fn default() -> Self {
        InitializationScript {
            script: "".to_string(),
            for_main_frame_only: true,
        }
    }
}

impl InitializationScript {
    // todo: let it be usefull like Tauri
    fn main_frame_script(script: String) -> Self {
        InitializationScript {
            script,
            ..Self::default()
        }
    }
}

#[derive(Template)]
#[default_template("scripts/tauri/ipc.js")]
pub(crate) struct IpcJavascript<'a> {
    pub(crate) isolation_origin: &'a str,
}

#[derive(Template)]
#[default_template("scripts/tauri/pattern.js")]
pub(crate) struct PatternJavascript {
    pub(crate) pattern: PatternObject,
}

#[derive(Template)]
#[default_template("scripts/webview/zoom-hotkey.js")]
struct HotkeyZoom<'a> {
    os_name: &'a str,
}

/// The shape of the JavaScript Pattern config
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase", tag = "pattern")]
enum PatternObject {
    /// Brownfield pattern.
    Brownfield,
    // todo: isolation mode
    //
    // Isolation pattern. Recommended for security purposes.
    // #[cfg(feature = "isolation")]
    // Isolation {
    //     /// Which `IsolationSide` this `PatternObject` is getting injected into
    //     side: IsolationSide,
    // },
}

fn initialization_script(
    ipc_script: &str,
    pattern_script: &str,
    // use_https_scheme: bool, // todo: use_https_scheme
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let core_script = &CoreJavascript {
        os_name: std::env::consts::OS,
        protocol_scheme: "http",
        invoke_key: INVOKE_KEY,
    }
    .render_default(&core::default::Default::default())?
    .to_string();
    let event_initialization_script = &event_initialization_script(
        "__internal_unstable_listeners_function_id__",
        "__internal_unstable_listeners_object_id__",
    );
    let freeze_prototype = false;
    let freeze_prototype = if freeze_prototype {
        include_str!("scripts/tauri/freeze_prototype.js")
    } else {
        ""
    };
    InitJavascript {
        pattern_script,
        ipc_script,
        core_script,
        event_initialization_script,
        freeze_prototype,
    }
    .render_default(&core::default::Default::default())
    .map(|s| s.into_string())
    .map_err(Into::into)
}

#[derive(Template)]
#[default_template("scripts/tauri/ipc-protocol.js")]
pub(crate) struct InvokeInitializationScript<'a> {
    /// The function that processes the IPC message.
    #[raw]
    pub(crate) process_ipc_message_fn: &'a str,
    pub(crate) os_name: &'a str,
    pub(crate) fetch_channel_data_command: &'a str,
    pub(crate) invoke_key: &'a str,
}

#[derive(Template)]
#[default_template("scripts/tauri/core.js")]
struct CoreJavascript<'a> {
    os_name: &'a str,
    protocol_scheme: &'a str,
    invoke_key: &'a str,
}

pub(crate) fn event_initialization_script(function_name: &str, listeners: &str) -> String {
    format!(
        "Object.defineProperty(window, '{function_name}', {{
      value: function (eventData, ids) {{
        const listeners = (window['{listeners}'] && window['{listeners}'][eventData.event]) || []
        for (const id of ids) {{
          const listener = listeners[id]
          if (listener) {{
            eventData.id = id
            window.__TAURI_INTERNALS__.runCallback(listener.handlerId, eventData)
          }}
        }}
      }}
    }});
  "
    )
}

#[derive(Template)]
#[default_template("scripts/tauri/init.js")]
struct InitJavascript<'a> {
    #[raw]
    pattern_script: &'a str,
    #[raw]
    ipc_script: &'a str,
    #[raw]
    core_script: &'a str,
    #[raw]
    event_initialization_script: &'a str,
    #[raw]
    freeze_prototype: &'a str,
}
