pub mod webview;
pub use http;
pub use serde;
pub use serde_json;
pub use wry;

use http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE};
use serde::Serialize;
use serialize_to_javascript::{DefaultTemplate, Template, default_template};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use wry::{Error as WryError, Result, WebView, WebViewBuilder, WebViewId};

pub use gpui_manos_webview_macros::{
    api_handler, api_handlers, command, command_handler, command_handlers, generate_handler,
};

// todo: implement dev server

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
        self.apply(move |b| {
            b.with_asynchronous_custom_protocol(
                "wry".into(),
                move |webview_id, request, responder| {
                    let response = serve_static(webview_id, static_root.to_string(), request)
                        .unwrap_or_else(response_internal_server_err);
                    responder.respond(response)
                },
            )
            .with_url("wry://localhost")
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
            b.with_asynchronous_custom_protocol(
                "ipc".into(),
                move |webview_id, request, responder| {
                    println!(
                        "webview_id: {}, method: {}, scheme: {:?}, host: {:?}, path: {:?}",
                        webview_id,
                        request.method(),
                        request.uri().scheme(),
                        request.uri().host(),
                        request.uri().path()
                    );

                    let raw_path = request.uri().path().to_string();
                    let command = decode_uri_component(
                        raw_path.strip_prefix('/').unwrap_or(raw_path.as_str()),
                    );

                    if let Some(ref handler) = invoke_handler {
                        let invoke = Invoke { command, request };
                        if let Some(response) = handler(invoke) {
                            responder.respond(response);
                            return;
                        }

                        responder.respond(ipc::not_found(raw_path));
                        return;
                    }

                    if let Some(handler) = handlers.get(&command) {
                        let response = handler(request);
                        responder.respond(response);
                        return;
                    }

                    responder.respond(ipc::not_found(raw_path));
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

    pub fn bad_request<S: ToString>(message: S) -> http::Response<Vec<u8>> {
        response_builder(http::StatusCode::BAD_REQUEST, "error")
            .header(CONTENT_TYPE, HeaderValue::from_static("text/plain"))
            .body(message.to_string().into_bytes())
            .unwrap()
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
    // Read the file content from file path
    let root = PathBuf::from(static_path.to_string());
    let path = if path == "/" {
        "index.html"
    } else {
        //  removing leading slash
        &path[1..]
    };

    let content = match std::fs::canonicalize(root.join(path)) {
        Ok(path) => match std::fs::read(path) {
            Ok(content) => content,
            Err(err) => {
                println!("failed to read file: {err}");
                return Ok(response_not_found(err)); // todo: change it to internal server error
            }
        },
        Err(err) => {
            println!("failed to canonicalize path: {err}");
            return Ok(response_not_found(path));
        }
    };

    // Return asset contents and mime types based on file extentions
    // If you don't want to do this manually, there are some crates for you.
    // Such as `infer` and `mime_guess`.
    let mimetype = if path.ends_with(".html") || path == "/" {
        "text/html"
    } else if path.ends_with(".js") {
        "text/javascript"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".wasm") {
        "application/wasm"
    } else if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else {
        return Ok(response_not_implemented(path));
    };

    http::Response::builder()
        .status(http::StatusCode::OK)
        .header(CONTENT_TYPE, mimetype)
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(content)
        .map_err(Into::into)
}

fn response_not_found<S: ToString>(content: S) -> http::Response<Vec<u8>> {
    http::Response::builder()
        .status(http::StatusCode::NOT_FOUND)
        .header(CONTENT_TYPE, "text/plain")
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(format!("{} not found", content.to_string()).into_bytes())
        .unwrap()
}

fn response_not_implemented<S: ToString>(content: S) -> http::Response<Vec<u8>> {
    http::Response::builder()
        .status(http::StatusCode::NOT_IMPLEMENTED)
        .header(CONTENT_TYPE, "text/plain")
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(format!("{} not implemented", content.to_string()).into_bytes())
        .unwrap()
}

fn response_internal_server_err<S: ToString>(content: S) -> http::Response<Vec<u8>> {
    http::Response::builder()
        .status(http::StatusCode::INTERNAL_SERVER_ERROR)
        .header(CONTENT_TYPE, "text/plain")
        .body(content.to_string().as_bytes().to_vec())
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
            invoke_key: "gpui",
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
        invoke_key: "gpui",
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
