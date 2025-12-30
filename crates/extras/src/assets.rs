use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

/// Asset source for `gpui-manos-components`.
///
/// Includes the SVG icon set used by `plate_toolbar` and story demos (e.g. DnD tree).
pub struct ExtrasAssetSource;

impl ExtrasAssetSource {
    pub fn new() -> Self {
        Self
    }
}

const ASSETS: &[(&str, &[u8])] = &[
    (
        "icons/archive.svg",
        include_bytes!("../assets/icons/archive.svg"),
    ),
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
        "icons/arrow-down.svg",
        include_bytes!("../assets/icons/arrow-down.svg"),
    ),
    (
        "icons/arrow-down-to-line.svg",
        include_bytes!("../assets/icons/arrow-down-to-line.svg"),
    ),
    (
        "icons/arrow-up.svg",
        include_bytes!("../assets/icons/arrow-up.svg"),
    ),
    (
        "icons/arrow-up-to-line.svg",
        include_bytes!("../assets/icons/arrow-up-to-line.svg"),
    ),
    (
        "icons/arrow-left-right.svg",
        include_bytes!("../assets/icons/arrow-left-right.svg"),
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
        "icons/chevron-right.svg",
        include_bytes!("../assets/icons/chevron-right.svg"),
    ),
    (
        "icons/chevron-up.svg",
        include_bytes!("../assets/icons/chevron-up.svg"),
    ),
    (
        "icons/chevrons-up-down.svg",
        include_bytes!("../assets/icons/chevrons-up-down.svg"),
    ),
    (
        "icons/chevrons-down-up.svg",
        include_bytes!("../assets/icons/chevrons-down-up.svg"),
    ),
    (
        "icons/check.svg",
        include_bytes!("../assets/icons/check.svg"),
    ),
    (
        "icons/close.svg",
        include_bytes!("../assets/icons/close.svg"),
    ),
    (
        "icons/columns-2.svg",
        include_bytes!("../assets/icons/columns-2.svg"),
    ),
    ("icons/copy.svg", include_bytes!("../assets/icons/copy.svg")),
    (
        "icons/download.svg",
        include_bytes!("../assets/icons/download.svg"),
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
    ("icons/file.svg", include_bytes!("../assets/icons/file.svg")),
    ("icons/film.svg", include_bytes!("../assets/icons/film.svg")),
    (
        "icons/highlighter.svg",
        include_bytes!("../assets/icons/highlighter.svg"),
    ),
    (
        "icons/github.svg",
        include_bytes!("../assets/icons/github.svg"),
    ),
    (
        "icons/git-compare.svg",
        include_bytes!("../assets/icons/git-compare.svg"),
    ),
    (
        "icons/gallery-horizontal.svg",
        include_bytes!("../assets/icons/gallery-horizontal.svg"),
    ),
    (
        "icons/gallery-vertical-end.svg",
        include_bytes!("../assets/icons/gallery-vertical-end.svg"),
    ),
    (
        "icons/layers.svg",
        include_bytes!("../assets/icons/layers.svg"),
    ),
    (
        "icons/layers-plus.svg",
        include_bytes!("../assets/icons/layers-plus.svg"),
    ),
    (
        "icons/layers-2.svg",
        include_bytes!("../assets/icons/layers-2.svg"),
    ),
    (
        "icons/send-to-back.svg",
        include_bytes!("../assets/icons/send-to-back.svg"),
    ),
    (
        "icons/circle-pile.svg",
        include_bytes!("../assets/icons/circle-pile.svg"),
    ),
    (
        "icons/folder-closed.svg",
        include_bytes!("../assets/icons/folder-closed.svg"),
    ),
    (
        "icons/folder.svg",
        include_bytes!("../assets/icons/folder.svg"),
    ),
    (
        "icons/folder-open.svg",
        include_bytes!("../assets/icons/folder-open.svg"),
    ),
    (
        "icons/folder-sync.svg",
        include_bytes!("../assets/icons/folder-sync.svg"),
    ),
    (
        "icons/image.svg",
        include_bytes!("../assets/icons/image.svg"),
    ),
    ("icons/info.svg", include_bytes!("../assets/icons/info.svg")),
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
        "icons/layout-dashboard.svg",
        include_bytes!("../assets/icons/layout-dashboard.svg"),
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
    (
        "icons/panel-bottom.svg",
        include_bytes!("../assets/icons/panel-bottom.svg"),
    ),
    (
        "icons/panel-bottom-open.svg",
        include_bytes!("../assets/icons/panel-bottom-open.svg"),
    ),
    ("icons/pen.svg", include_bytes!("../assets/icons/pen.svg")),
    (
        "icons/pen-line.svg",
        include_bytes!("../assets/icons/pen-line.svg"),
    ),
    ("icons/plus.svg", include_bytes!("../assets/icons/plus.svg")),
    (
        "icons/replace.svg",
        include_bytes!("../assets/icons/replace.svg"),
    ),
    (
        "icons/refresh-cw.svg",
        include_bytes!("../assets/icons/refresh-cw.svg"),
    ),
    (
        "icons/redo-2.svg",
        include_bytes!("../assets/icons/redo-2.svg"),
    ),
    (
        "icons/search.svg",
        include_bytes!("../assets/icons/search.svg"),
    ),
    (
        "icons/square-library.svg",
        include_bytes!("../assets/icons/square-library.svg"),
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
        "icons/text-align-start.svg",
        include_bytes!("../assets/icons/text-align-start.svg"),
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
    (
        "icons/library.svg",
        include_bytes!("../assets/icons/library.svg"),
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
