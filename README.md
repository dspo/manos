# Manos

> "A rich text editor built on `gpui` and `gpui-component`, inspired by `plate.js`."

This repository is intended to host additional GPUI components about a Richtext Editor built on top of `gpui` and
`gpui-component`.

![img.png](docs/richtext.example.png)

## Run

- Story app: `cargo run`
- DnD list example: `cargo run --example dnd_list`
- DnD tree example: `cargo run --example dnd_tree`
- Rich text example: `cargo run --example richtext`
- WebView example: `cargo run --example webview` (loads `crates/story/examples/webview-app/dist` if present; otherwise shows a friendly hint page)
- Plate toolbar buttons: `cargo run --example plate_toolbar_buttons`

## Docs

- DnD List design: `docs/dnd-list-component-design.md`
- DnD Tree implementation notes: `docs/dnd-tree.md`
- WebView module: `crates/webview/README.MD`
