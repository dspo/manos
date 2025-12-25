# gpui-manos-webview

A lightweight Rust framework for building webviews in GPUI applications, combining GPUI with WRY to create a "mini-Tauri" like experience.

## Overview

`gpui-manos-webview` integrates GPUI and WRY (a cross-platform WebView library)
to provide a simple framework for embedding WebViews anywhere in GPUI applications with Tauri-like bidirectional communication between frontend and Rust.

## Features

- **WebView Integration**: Thanks to the Wry project, we can leverage Wry to integrate WebView components into GPUI Apps
- **Static File Serving**: Easily serve local HTML, CSS, and JavaScript files
- **Rust Command Bridging**: Call Rust functions from web content using the provided macros
- **Lightweight**: Can be described as lightweight, though it's still quite simple in its current state

## License

This project is dual-licensed under both MIT and Apache 2.0 licenses.

## Acknowledgments

This project builds upon and is inspired by the excellent work of the following projects:

- [Tauri](https://tauri.app/) provides excellent ideas for building desktop applications, with a complete toolchain and rich ecosystem. Some [scripts](./src/scripts/) in this project are copied from Tauri project.
- [Wry](https://github.com/tauri-apps/wry) is a cross-platform WebView library that provides us with the ability to embed WebViews on different platforms.
- [GPUI](https://gpui.rs/) is the main framework for this project.
- [GPUI Component](https://longbridge.github.io/gpui-component/) offers many useful and refined components.
