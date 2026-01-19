# Manos

This repository implements a set of GPUI ideas, including (but not limited to) an enhanced WebView component, a Plate.js-like rich text editor component, dnd-list, and a dnt tree component.
The original intention of this repository is to validate some GPUI inspirations, not to provide production components.
This repository is still under active development; many APIs are not exposed, are immature, or are changing frequently, and many features are still experimental.
This code can serve as a starting point for learning GPUI and developing on GPUI, but it is not recommended for production use.
This repository makes extensive use of AI tools for programming, agents like Codex, models like GPT-5.2.
Some code in this repository is directly copied from repositories such as [Tauri](https://github.com/tauri-apps/tauri), [Zed](https://github.com/zed-industries/zed), [GPUI](https://www.gpui.rs/), and [GPUI Component](https://github.com/longbridge/gpui-component) (in cases where it cannot be cleanly referenced in Cargo.toml), so it inherits their licenses.

# "Tauri" on GPUI

![gpui-wry](docs/gpui-wry.png)

A component to support Tauri API work on [GPUI](https://www.gpui.rs/) + [Wry](https://github.com/tauri-apps/wry).
Dedicated to hybrid GUI development combining GPUI and frontend technologies.

Based on this, we can embed any web interface anywhere inside a GPUI app.
The GPUI UI and the Web App UI can work together seamlessly.

Currently, calling between the frontend (TS/JS) and Rust is supported in both directions.

From the code below, calling Rust from the frontend works exactly like Tauri’s `invoke` API (`import { invoke } from "@tauri-apps/api/core"`).
In other words, it’s the same as in a Tauri project.
In fact, Tauri’s default example project can be migrated to this Webview component without changing a single line.

```ts
import { invoke } from "@tauri-apps/api/core";

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  async function greet() {
    setGreetMsg(await invoke("greet", { name }));
  }

  ...
}
```

On the Rust side, we provide Tauri-like macros `#[command]` and `generate_handler![]`.
Usage:

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

However, Tauri’s plugin system is not supported at the moment, because it depends on Tauri’s RuntimeHandler.
It may be difficult to support to that extent in the future as well.

## Plate.rs -- A plate.js like richtext editor component.

> "A rich text editor built on `gpui` and `gpui-component`, inspired by `plate.js`."

This repository is intended to host additional GPUI components about a Richtext Editor built on top of `gpui` and
`gpui-component`.

![img.png](docs/richtext.example.png)

# Run
- Story gallery app: `cargo run` (left sidebar selects stories; right side renders the selected view)
- Welcome Tauri story (WebView): build assets first: `cd crates/story/examples/webview-app && pnpm install && pnpm build` (loads `crates/story/examples/webview-app/dist` if present; otherwise shows a friendly hint page)
