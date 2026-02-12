use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};
use gpui_component_assets::Assets as ComponentAssets;
use rust_embed::RustEmbed;

/// Embedded local assets for `gpui-manos-assets`.
///
/// Contains SVG icons used by `plate_toolbar` and story demos that are not
/// available in `gpui-component-assets`.
#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
struct LocalAssets;

/// Hybrid asset source for `gpui-manos-assets`.
///
/// Implements a layered loading strategy:
/// 1. First, try to load from local assets (allows overriding)
/// 2. Fall back to `gpui-component-assets` for common icons
///
/// This approach avoids duplicating icons that are already provided by
/// `gpui-component-assets` while allowing custom icons for the rich text
/// editor toolbar and other components.
pub struct ExtrasAssetSource;

impl ExtrasAssetSource {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExtrasAssetSource {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetSource for ExtrasAssetSource {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        // First, try local assets
        if let Some(file) = LocalAssets::get(path) {
            return Ok(Some(file.data));
        }

        // Fall back to gpui-component-assets
        ComponentAssets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let path = path.trim_matches('/');
        let prefix = if path.is_empty() {
            String::new()
        } else {
            format!("{path}/")
        };

        // Collect from both sources and deduplicate
        let mut children: Vec<SharedString> = Vec::new();

        // Local assets first
        for asset_path in LocalAssets::iter() {
            if !asset_path.starts_with(&prefix) {
                continue;
            }

            let rest = &asset_path[prefix.len()..];
            let name = rest.split('/').next().unwrap_or(rest);

            if !children.iter().any(|item| item.as_ref() == name) {
                children.push(SharedString::from(name.to_string()));
            }
        }

        // Then gpui-component-assets
        if let Ok(component_children) = ComponentAssets.list(path) {
            for child in component_children {
                if !children.iter().any(|item| item.as_ref() == child.as_ref()) {
                    children.push(child);
                }
            }
        }

        Ok(children)
    }
}
