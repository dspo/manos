# gpui-manos-webview

A lightweight Rust framework for building webviews in GPUI applications, combining GPUI with WRY to create a "mini-Tauri" like experience.

## Overview

`gpui-manos-webview` integrates GPUI and WRY (a cross-platform WebView library)
to provide a simple framework for embedding WebViews anywhere in GPUI applications with Tauri-like bidirectional communication between frontend and Rust.

## Features

- **WebView Integration**: Thanks to the Wry project, we can leverage Wry to integrate WebView components into GPUI Apps
- **Static File Serving**: Easily serve local HTML, CSS, and JavaScript files
- **Rust Command Bridging**: Call Rust functions from web content using Tauri-like `#[command]` + `generate_handler!`
- **Lightweight**: Can be described as lightweight, though it's still quite simple in its current state

## Tauri vs Wry

- `wry` is a cross-platform WebView library.
- `tauri` is an application framework; on desktop it uses `wry` (via `tauri-runtime-wry`) to provide the WebView/runtime layer.

## Commands (Tauri-like)

Register commands on the Rust side:

```rust
use gpui_manos_webview::{Builder, webview::WebView};
use gpui_manos_webview::wry::WebViewId;

#[gpui_manos_webview::command]
fn greet(name: String) -> Result<String, String> {
    Ok(format!("Hello, {name}!"))
}

fn view(window: &mut gpui::Window, app: &mut gpui::App) -> gpui::Entity<WebView> {
    app.new(|cx: &mut gpui::Context<WebView>| {
        let webview = Builder::new()
            .with_webview_id(WebViewId::from("main"))
            .serve_static("path/to/frontend/dist")
            .invoke_handler(gpui_manos_webview::generate_handler![greet])
            .build_as_child(window)
            .unwrap();

        WebView::new(webview, window, cx)
    })
}
```

Call commands from the frontend:

```js
await window.__TAURI_INTERNALS__.invoke("greet", { name: "Manos" })
```

Or (if you use the Tauri JS client in your bundler):

```ts
import { invoke } from "@tauri-apps/api/core"
await invoke("greet", { name: "Manos" })
```

Notes:

- Argument keys default to `camelCase` (same as Tauri). For example `fn foo(user_name: String)` expects `{ userName: "..." }`.
- You can opt out with `#[gpui_manos_webview::command(rename_all = "snake_case")]`.
- `generate_handler![...]` detects duplicate command names at compile time.
- Commands can be synchronous or `async fn` (async is executed via a simple `block_on`, not a full Tokio runtime).
- `ipc://` custom-protocol invokes run command execution off-thread; postMessage fallback executes on the IPC handler thread.
- Binary responses: return `gpui_manos_webview::ipc::Response::binary(...)` to resolve `ArrayBuffer` on the frontend (custom-protocol path).
- Raw request access: add a `gpui_manos_webview::ipc::Request` parameter to read method/uri/headers/body.
- Channels: accept `gpui_manos_webview::ipc::Channel<T>` and call `send(T)` to stream messages to the frontend (Tauri `Channel`-compatible message format).
- Return types supported:
  - `T: serde::Serialize`
  - `Result<T, E>`: default `E: ToString` (rejects with a JSON string); opt-in `#[gpui_manos_webview::command(error = "json")]` for `E: serde::Serialize` (rejects with structured JSON)

## Issues (Gap vs Tauri)

This section tracks known gaps between this "mini-Tauri" implementation and upstream Tauri behavior.

Roadmap (calling Rust from the frontend): see `CALLING_RUST_PARITY.md`.

### P0 — Compatibility / Security

- [x] Protocol alignment: `convertFileSrc()` uses `asset://` by default but `serve_static()` registers `wry://` only; align schemes or register `asset://` too (JS: `src/scripts/tauri/core.js`, Rust: `src/lib.rs`).
- [ ] IPC request validation: (partial) enforce `Tauri-Invoke-Key`; still missing `Origin`, `Tauri-Callback`, `Tauri-Error`, and request body decoding based on `Content-Type` (ref: Tauri `crates/tauri/src/ipc/protocol.rs`).
- [x] IPC preflight & method handling: support `OPTIONS` and restrict to `POST/OPTIONS` like Tauri (ref: Tauri `crates/tauri/src/ipc/protocol.rs`).
- [x] postMessage fallback: implement the `window.ipc.postMessage` bridge when custom-protocol IPC fails (Tauri JS falls back; Rust handler is missing here).

### P1 — Behavior Parity

- [ ] Static asset fallback chain: support `path.html` / `path/index.html` / `index.html` fallbacks (ref: Tauri `crates/tauri/src/manager/mod.rs` `get_asset`).
- [ ] CSP + security headers: add configurable headers and per-origin `Access-Control-Allow-Origin` (avoid unconditional `*`).
- [ ] Dev server/proxy mode: proxy the app protocol to a dev server (and optional caching) in development (ref: Tauri `crates/tauri/src/protocol/tauri.rs`).
- [x] Channel base: `ipc::Channel<T>` streams JSON messages and signals end on drop.
- [ ] Channel optimization: implement `plugin:__TAURI_CHANNEL__|fetch` fast-path + large/binary payload strategy.
- [ ] Compose `invoke_handler` with low-level APIs: allow plugin-style `plugin:xxx|yyy` handlers even when `invoke_handler` is set.

### P2 — Scripts Without Backend Support

- [ ] Implement `plugin:webview|set_webview_zoom`, `plugin:webview|internal_toggle_devtools`, `plugin:webview|print` so injected scripts don’t 404.
- [ ] Replace mocked window/webview labels in injected metadata.
- [ ] Event system parity: add Rust-side listener management to match injected JS event dispatcher.
- [ ] Isolation pattern: implement isolation mode (iframe + crypto) and related protocol(s).

## License

This project is dual-licensed under both MIT and Apache 2.0 licenses.

## Acknowledgments

This project builds upon and is inspired by the excellent work of the following projects:

- [Tauri](https://tauri.app/) provides excellent ideas for building desktop applications, with a complete toolchain and rich ecosystem. Some [scripts](./src/scripts/) in this project are copied from Tauri project.
- [Wry](https://github.com/tauri-apps/wry) is a cross-platform WebView library that provides us with the ability to embed WebViews on different platforms.
- [GPUI](https://gpui.rs/) is the main framework for this project.
- [GPUI Component](https://longbridge.github.io/gpui-component/) offers many useful and refined components.
