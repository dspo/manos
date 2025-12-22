use std::path::{Path, PathBuf};

use gpui::{Action, App, SharedString};
use gpui_component::{Theme, ThemeMode, ThemeRegistry};

#[derive(Action, Clone, PartialEq)]
#[action(namespace = themes, no_json)]
pub struct SwitchTheme(pub SharedString);

#[derive(Action, Clone, PartialEq)]
#[action(namespace = themes, no_json)]
pub struct SwitchThemeMode(pub ThemeMode);

fn dir_has_theme_files(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };

    entries.flatten().any(|entry| {
        entry
            .path()
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
    })
}

pub fn init(cx: &mut App) {
    let local_dir = PathBuf::from("./themes");
    let parent_dir = PathBuf::from("../themes");

    let theme_dir = if dir_has_theme_files(&local_dir) {
        local_dir
    } else if dir_has_theme_files(&parent_dir) {
        parent_dir
    } else {
        let _ = std::fs::create_dir_all(&local_dir);
        local_dir
    };

    let _ = ThemeRegistry::watch_dir(theme_dir, cx, |_| {});

    cx.on_action(|switch: &SwitchTheme, cx| {
        let theme_name = switch.0.clone();
        if let Some(theme_config) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
            Theme::global_mut(cx).apply_config(&theme_config);
        }
        cx.refresh_windows();
    });

    cx.on_action(|switch: &SwitchThemeMode, cx| {
        Theme::change(switch.0, None, cx);
        cx.refresh_windows();
    });
}
