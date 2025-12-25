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
- Commands are currently synchronous. Return types supported:
  - `T: serde::Serialize`
  - `Result<T, E>` where `T: serde::Serialize` and `E: ToString`

## License

This project is dual-licensed under both MIT and Apache 2.0 licenses.

## Acknowledgments

This project builds upon and is inspired by the excellent work of the following projects:

- [Tauri](https://tauri.app/) provides excellent ideas for building desktop applications, with a complete toolchain and rich ecosystem. Some [scripts](./src/scripts/) in this project are copied from Tauri project.
- [Wry](https://github.com/tauri-apps/wry) is a cross-platform WebView library that provides us with the ability to embed WebViews on different platforms.
- [GPUI](https://gpui.rs/) is the main framework for this project.
- [GPUI Component](https://longbridge.github.io/gpui-component/) offers many useful and refined components.
