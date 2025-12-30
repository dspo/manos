use std::borrow::Cow;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use gpui::App;

mod embedded_fonts {
    include!(concat!(env!("OUT_DIR"), "/embedded_fonts.rs"));
}

pub(super) struct LoadedFontsSummary {
    pub(super) loaded_files: usize,
    pub(super) detected_families: Vec<String>,
}

pub(super) fn try_load_user_fonts(cx: &mut App) -> Option<LoadedFontsSummary> {
    let font_dirs = candidate_font_dirs();
    let mut loaded_files = 0usize;
    let mut fonts: Vec<Cow<'static, [u8]>> = Vec::new();

    for bytes in embedded_fonts::EMBEDDED_FONT_BYTES.iter().copied() {
        loaded_files += 1;
        fonts.push(Cow::Borrowed(bytes));
    }

    let font_bytes = read_fonts_from_dirs(&font_dirs);
    loaded_files += font_bytes.len();
    fonts.extend(font_bytes.into_iter().map(Cow::Owned));

    if fonts.is_empty() {
        return None;
    }

    let before: HashSet<String> = cx.text_system().all_font_names().into_iter().collect();

    if let Err(err) = cx.text_system().add_fonts(fonts) {
        eprintln!("git-viewer: failed to load fonts: {err:#}");
        return None;
    }

    let after = cx.text_system().all_font_names();
    let detected_families: Vec<String> = after
        .into_iter()
        .filter(|name| !before.contains(name))
        .collect();

    Some(LoadedFontsSummary {
        loaded_files,
        detected_families,
    })
}

pub(super) struct AppliedThemeFonts {
    pub(super) ui_font_family: bool,
    pub(super) mono_font_family: bool,
}

pub(super) fn select_theme_fonts_from_env(cx: &mut App) -> AppliedThemeFonts {
    let font_family = std::env::var("GIT_VIEWER_FONT_FAMILY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let mono_font_family = std::env::var("GIT_VIEWER_MONO_FONT_FAMILY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let ui_set = font_family.is_some();
    let mono_set = mono_font_family.is_some();

    if let Some(font_family) = font_family {
        gpui_component::Theme::global_mut(cx).font_family = font_family.into();
    }
    if let Some(mono_font_family) = mono_font_family {
        gpui_component::Theme::global_mut(cx).mono_font_family = mono_font_family.into();
    }

    AppliedThemeFonts {
        ui_font_family: ui_set,
        mono_font_family: mono_set,
    }
}

fn candidate_font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(dir) = std::env::var("GIT_VIEWER_FONTS_DIR") {
        let dir = dir.trim();
        if !dir.is_empty() {
            dirs.push(PathBuf::from(dir));
        }
    }

    // Development-friendly default: `crates/git-viewer/assets/fonts`
    #[cfg(debug_assertions)]
    dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/fonts"));

    // Distribution-friendly default: `fonts/` next to the executable.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("fonts"));
        }
    }

    dirs.sort();
    dirs.dedup();
    dirs
}

fn read_fonts_from_dirs(dirs: &[PathBuf]) -> Vec<Vec<u8>> {
    let mut bytes = Vec::new();
    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        match std::fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !is_font_file(&path) {
                        continue;
                    }
                    match std::fs::read(&path) {
                        Ok(data) => bytes.push(data),
                        Err(err) => eprintln!(
                            "git-viewer: failed to read font file {}: {err:#}",
                            path.display()
                        ),
                    }
                }
            }
            Err(err) => eprintln!(
                "git-viewer: failed to read fonts dir {}: {err:#}",
                dir.display()
            ),
        }
    }
    bytes
}

fn is_font_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    matches!(ext.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc")
}
