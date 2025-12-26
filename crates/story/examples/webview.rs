use gpui::{
    App, AppContext, Application, Bounds, Context, Entity, WindowBounds, WindowOptions, px, size,
};
use gpui_manos_webview::webview::WebView;
use gpui_manos_webview::wry::WebViewId;
use serde::Serialize;
use std::path::PathBuf;

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.activate(true);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(900.), px(640.0)),
                cx,
            ))),
            is_resizable: true,
            ..Default::default()
        };

        cx.open_window(options, webview_view).unwrap();
    });
}

fn webview_view(window: &mut gpui::Window, app: &mut App) -> Entity<WebView> {
    app.new(|cx: &mut Context<WebView>| {
        let static_root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/webview-app/dist");
        let index_html = static_root.join("index.html");

        let builder = gpui_manos_webview::Builder::new()
            .with_webview_id(WebViewId::from("webview"))
            .invoke_handler(gpui_manos_webview::generate_handler![
                greet,
                greet_async,
                greet_slow,
                get_bytes,
                echo_bytes,
                inspect_request
            ]);

        let builder = if index_html.exists() {
            builder.serve_static(static_root.to_string_lossy().to_string())
        } else {
            eprintln!(
                "[webview example] missing web assets: {}",
                index_html.to_string_lossy()
            );

            let template = include_str!("webview-assets-missing.html");
            let html = template.replace(
                "__EXPECTED_PATH__",
                &escape_html(&index_html.to_string_lossy()),
            );

            builder.apply(|b| b.with_html(html))
        };

        let webview = builder.build_as_child(window).unwrap();
        #[cfg(debug_assertions)]
        webview.open_devtools();

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
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
