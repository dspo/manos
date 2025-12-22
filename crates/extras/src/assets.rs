use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

/// Asset source for `gpui-component-extras`.
///
/// Currently only includes the SVG icon set used by `plate_toolbar`.
pub struct ExtrasAssetSource;

impl ExtrasAssetSource {
    pub fn new() -> Self {
        Self
    }
}

const ASSETS: &[(&str, &[u8])] = &[
    (
        "icons/align-center.svg",
        include_bytes!("../assets/icons/align-center.svg"),
    ),
    (
        "icons/align-justify.svg",
        include_bytes!("../assets/icons/align-justify.svg"),
    ),
    (
        "icons/align-left.svg",
        include_bytes!("../assets/icons/align-left.svg"),
    ),
    (
        "icons/align-right.svg",
        include_bytes!("../assets/icons/align-right.svg"),
    ),
    (
        "icons/arrow-down-to-line.svg",
        include_bytes!("../assets/icons/arrow-down-to-line.svg"),
    ),
    (
        "icons/arrow-up-to-line.svg",
        include_bytes!("../assets/icons/arrow-up-to-line.svg"),
    ),
    (
        "icons/audio-lines.svg",
        include_bytes!("../assets/icons/audio-lines.svg"),
    ),
    (
        "icons/baseline.svg",
        include_bytes!("../assets/icons/baseline.svg"),
    ),
    ("icons/bold.svg", include_bytes!("../assets/icons/bold.svg")),
    (
        "icons/chevron-down.svg",
        include_bytes!("../assets/icons/chevron-down.svg"),
    ),
    (
        "icons/code-xml.svg",
        include_bytes!("../assets/icons/code-xml.svg"),
    ),
    (
        "icons/ellipsis.svg",
        include_bytes!("../assets/icons/ellipsis.svg"),
    ),
    (
        "icons/file-up.svg",
        include_bytes!("../assets/icons/file-up.svg"),
    ),
    ("icons/film.svg", include_bytes!("../assets/icons/film.svg")),
    (
        "icons/highlighter.svg",
        include_bytes!("../assets/icons/highlighter.svg"),
    ),
    (
        "icons/image.svg",
        include_bytes!("../assets/icons/image.svg"),
    ),
    (
        "icons/indent-decrease.svg",
        include_bytes!("../assets/icons/indent-decrease.svg"),
    ),
    (
        "icons/indent-increase.svg",
        include_bytes!("../assets/icons/indent-increase.svg"),
    ),
    (
        "icons/italic.svg",
        include_bytes!("../assets/icons/italic.svg"),
    ),
    ("icons/link.svg", include_bytes!("../assets/icons/link.svg")),
    (
        "icons/unlink.svg",
        include_bytes!("../assets/icons/unlink.svg"),
    ),
    (
        "icons/list-collapse.svg",
        include_bytes!("../assets/icons/list-collapse.svg"),
    ),
    (
        "icons/list-ordered.svg",
        include_bytes!("../assets/icons/list-ordered.svg"),
    ),
    (
        "icons/list-todo.svg",
        include_bytes!("../assets/icons/list-todo.svg"),
    ),
    ("icons/list.svg", include_bytes!("../assets/icons/list.svg")),
    (
        "icons/message-square-text.svg",
        include_bytes!("../assets/icons/message-square-text.svg"),
    ),
    (
        "icons/minus.svg",
        include_bytes!("../assets/icons/minus.svg"),
    ),
    (
        "icons/paint-bucket.svg",
        include_bytes!("../assets/icons/paint-bucket.svg"),
    ),
    ("icons/pen.svg", include_bytes!("../assets/icons/pen.svg")),
    ("icons/plus.svg", include_bytes!("../assets/icons/plus.svg")),
    (
        "icons/redo-2.svg",
        include_bytes!("../assets/icons/redo-2.svg"),
    ),
    (
        "icons/smile.svg",
        include_bytes!("../assets/icons/smile.svg"),
    ),
    (
        "icons/strikethrough.svg",
        include_bytes!("../assets/icons/strikethrough.svg"),
    ),
    (
        "icons/table.svg",
        include_bytes!("../assets/icons/table.svg"),
    ),
    (
        "icons/underline.svg",
        include_bytes!("../assets/icons/underline.svg"),
    ),
    (
        "icons/undo-2.svg",
        include_bytes!("../assets/icons/undo-2.svg"),
    ),
    (
        "icons/wand-sparkles.svg",
        include_bytes!("../assets/icons/wand-sparkles.svg"),
    ),
    (
        "icons/wrap-text.svg",
        include_bytes!("../assets/icons/wrap-text.svg"),
    ),
];

impl AssetSource for ExtrasAssetSource {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        for (asset_path, bytes) in ASSETS {
            if *asset_path == path {
                return Ok(Some(Cow::Borrowed(*bytes)));
            }
        }

        Ok(None)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let path = path.trim_matches('/');
        let prefix = if path.is_empty() {
            String::new()
        } else {
            format!("{path}/")
        };

        let mut children: Vec<SharedString> = Vec::new();
        for (asset_path, _) in ASSETS {
            if !asset_path.starts_with(&prefix) {
                continue;
            }

            let rest = &asset_path[prefix.len()..];
            let Some((first, _)) = rest.split_once('/') else {
                children.push(rest.into());
                continue;
            };

            if !children.iter().any(|item| item.as_ref() == first) {
                children.push(first.into());
            }
        }

        Ok(children)
    }
}
