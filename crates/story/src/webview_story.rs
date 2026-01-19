use gpui::{App, AppContext as _, Context, Entity, Render, Window};
use gpui_manos_webview::webview::WebView;
use gpui_manos_webview::wry::WebViewId;
use serde::Serialize;
use std::path::PathBuf;

pub struct WebViewStory {
    webview: Entity<WebView>,
    visible: bool,
}

impl WebViewStory {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let webview = build_webview(window, cx);
        cx.new(|_| Self {
            webview,
            visible: true,
        })
    }

    pub fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        if visible {
            self.webview.update(cx, |webview, _| webview.show());
        } else {
            self.webview.update(cx, |webview, _| webview.hide());
        }
        cx.notify();
    }
}

impl Render for WebViewStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl gpui::IntoElement {
        self.webview.clone()
    }
}

fn build_webview(window: &mut Window, cx: &mut App) -> Entity<WebView> {
    cx.new(|cx: &mut Context<WebView>| {
        let static_root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/webview-app/dist");
        let index_html = static_root.join("index.html");

        let builder = gpui_manos_webview::Builder::new()
            .with_webview_id(WebViewId::from("webview"))
            .invoke_handler(gpui_manos_webview::generate_handler![
                greet,
                greet_async,
                greet_slow,
                count_to,
                channel_large_json,
                channel_large_bytes,
                get_bytes,
                echo_bytes,
                inspect_request
            ]);

        let builder = if index_html.exists() {
            builder.serve_static(static_root.to_string_lossy().to_string())
        } else {
            eprintln!(
                "[webview story] missing web assets: {}",
                index_html.to_string_lossy()
            );

            let template = include_str!("../examples/webview-assets-missing.html");
            let html = template.replace(
                "__EXPECTED_PATH__",
                &escape_html(&index_html.to_string_lossy()),
            );

            builder.apply(|b| b.with_html(html))
        };

        let webview = builder.build_as_child(window).unwrap();

        WebView::new(webview, window, cx)
    })
}

#[gpui_manos_webview::command]
fn greet(name: String) -> Result<String, String> {
    Ok(format!("Hello, {}! (from GPUI)", name))
}

#[gpui_manos_webview::command]
async fn greet_async(name: String) -> Result<String, String> {
    Ok(format!("Hello, {}! (from GPUI async)", name))
}

#[gpui_manos_webview::command]
fn greet_slow(name: String) -> Result<String, String> {
    std::thread::sleep(std::time::Duration::from_millis(750));
    Ok(format!("Hello, {}! (from GPUI slow)", name))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CountEvent {
    value: u32,
}

#[gpui_manos_webview::command]
fn count_to(n: u32, on_event: gpui_manos_webview::ipc::Channel<CountEvent>) -> Result<(), String> {
    for value in 0..=n {
        on_event.send(CountEvent { value })?;
        std::thread::sleep(std::time::Duration::from_millis(150));
    }

    Ok(())
}

#[gpui_manos_webview::command]
fn channel_large_json(on_event: gpui_manos_webview::ipc::Channel<String>) -> Result<(), String> {
    on_event.send("x".repeat(10_000))?;
    Ok(())
}

#[gpui_manos_webview::command]
fn channel_large_bytes(
    on_event: gpui_manos_webview::ipc::Channel<gpui_manos_webview::ipc::Response>,
) -> Result<(), String> {
    on_event.send(gpui_manos_webview::ipc::Response::binary(vec![7u8; 20_000]))?;
    Ok(())
}

#[gpui_manos_webview::command]
fn get_bytes() -> gpui_manos_webview::ipc::Response {
    gpui_manos_webview::ipc::Response::binary("hello from gpui-manos-webview".as_bytes().to_vec())
}

#[gpui_manos_webview::command]
fn echo_bytes(request: gpui_manos_webview::ipc::Request) -> gpui_manos_webview::ipc::Response {
    gpui_manos_webview::ipc::Response::binary(request.into_body())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestInfo {
    method: String,
    uri: String,
    content_type: Option<String>,
    body_len: usize,
}

#[gpui_manos_webview::command]
fn inspect_request(request: gpui_manos_webview::ipc::Request) -> RequestInfo {
    let content_type = request
        .headers()
        .get(gpui_manos_webview::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|value| value.to_string());

    RequestInfo {
        method: request.method().to_string(),
        uri: request.uri().to_string(),
        content_type,
        body_len: request.body().len(),
    }
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\'', "&#39;")
}
