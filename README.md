# Manos

> "A rich text editor built on `gpui` and `gpui-component`, inspired by `plate.js`."

This repository is intended to host additional GPUI components about a Richtext Editor built on top of `gpui` and
`gpui-component`.

![img.png](docs/richtext.example.png)

# Mini Tauri on GPUI

A mini-tauri work on webview on GPUI + Wry.

Based on this, we can embed any web interface anywhere inside a GPUI app.
The GPUI UI and the UI from a Web App can work together seamlessly.

Currently, calling between the frontend (TS/JS) and Rust is supported in both directions.
Calling Rust from the frontend is identical to how Tauri’s `import { invoke } from "@tauri-apps/api/core"` function is used.
On the Rust side, we provide Tauri-like macros `#[command]` and `generate_handler![]` .
However, Tauri’s plugin system is not supported at the moment, because it depends on Tauri’s RuntimeHandler.
It may be difficult to support to that extent in the future as well.

Some usages:

```rust
#[command]
fn greet(name: String) -> Result<String, String> {
    Ok(format!("Hello, {}! (from GPUI)", name))
}
```

```rust
let builder = Builder::new()
    .invoke_handler(
        generate_handler![greet]
    );
```

```ts
import { invoke } from "@tauri-apps/api/core";

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setGreetMsg(await invoke("greet", { name }));
  }

  ...
````

![gpui-wry](docs/gpui-wry.png)

## Run
- Story gallery app: `cargo run` (left sidebar selects stories; right side renders the selected view)
- WebView story: build assets first: `cd crates/story/examples/webview-app && pnpm install && pnpm build` (loads `crates/story/examples/webview-app/dist` if present; otherwise shows a friendly hint page)

## Docs

- DnD List design: `docs/dnd-list-component-design.md`
- DnD Tree implementation notes: `docs/dnd-tree.md`
- WebView module: `crates/webview/README.MD`
