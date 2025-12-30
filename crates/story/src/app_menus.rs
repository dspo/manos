use gpui::{Action, App, Entity, KeyBinding, Menu, MenuItem, SharedString, Window};
use gpui_component::{ActiveTheme as _, ThemeMode, ThemeRegistry, menu::AppMenuBar};

use crate::themes::{SwitchTheme, SwitchThemeMode};

pub const COMMAND_PALETTE_CONTEXT: &str = "CommandPalette";
pub const FIND_CONTEXT: &str = "FindDialog";

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct About;

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct Open;

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct Save;

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct SaveAs;

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct EmbedLocalImages;

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct ExportPortableJson;

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct ExportPlateBundle;

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct CollectAssets;

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct PortabilityReport;

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct CommandPalette;

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct InsertImage;

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct SetLink;

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct Find;

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct FindNext;

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct FindPrev;

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct CommandPaletteSelectPrev;

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct CommandPaletteSelectNext;

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = richtext_example, no_json)]
pub struct Quit;

pub fn init(
    title: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<AppMenuBar> {
    cx.on_action(|_: &Quit, cx: &mut App| {
        cx.quit();
    });

    cx.bind_keys([
        #[cfg(target_os = "macos")]
        KeyBinding::new(
            "cmd-shift-p",
            CommandPalette,
            Some(gpui_manos_plate::CONTEXT),
        ),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new(
            "ctrl-shift-p",
            CommandPalette,
            Some(gpui_manos_plate::CONTEXT),
        ),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-k", SetLink, Some(gpui_manos_plate::CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-k", SetLink, Some(gpui_manos_plate::CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-i", InsertImage, Some(gpui_manos_plate::CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-i", InsertImage, Some(gpui_manos_plate::CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-f", Find, Some(gpui_manos_plate::CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-f", Find, Some(gpui_manos_plate::CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-g", FindNext, Some(gpui_manos_plate::CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-g", FindNext, Some(gpui_manos_plate::CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-g", FindPrev, Some(gpui_manos_plate::CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-g", FindPrev, Some(gpui_manos_plate::CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-g", FindNext, Some(FIND_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-g", FindNext, Some(FIND_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-g", FindPrev, Some(FIND_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-g", FindPrev, Some(FIND_CONTEXT)),
        KeyBinding::new(
            "up",
            CommandPaletteSelectPrev,
            Some(COMMAND_PALETTE_CONTEXT),
        ),
        KeyBinding::new(
            "down",
            CommandPaletteSelectNext,
            Some(COMMAND_PALETTE_CONTEXT),
        ),
    ]);

    let title: SharedString = title.into();
    update_app_menu(title.clone(), cx);
    AppMenuBar::new(window, cx)
}

fn update_app_menu(title: SharedString, cx: &mut App) {
    let mode = cx.theme().mode;
    let light_label: SharedString = if mode.is_dark() {
        "Light".into()
    } else {
        "✓ Light".into()
    };
    let dark_label: SharedString = if mode.is_dark() {
        "✓ Dark".into()
    } else {
        "Dark".into()
    };

    cx.set_menus(vec![
        Menu {
            name: title,
            items: vec![
                MenuItem::action("About", About),
                MenuItem::Separator,
                MenuItem::action("Open...", Open),
                MenuItem::action("Save", Save),
                MenuItem::action("Save As...", SaveAs),
                MenuItem::action("Embed Local Images (Data URL)", EmbedLocalImages),
                MenuItem::action("Export Portable JSON...", ExportPortableJson),
                MenuItem::action("Export Plate Bundle...", ExportPlateBundle),
                MenuItem::action("Collect Assets into ./assets (Rewrite src)", CollectAssets),
                MenuItem::action("Portability Report...", PortabilityReport),
                MenuItem::Separator,
                theme_menu(cx),
                MenuItem::Separator,
                MenuItem::Submenu(Menu {
                    name: "Appearance".into(),
                    items: vec![
                        MenuItem::action(light_label, SwitchThemeMode(ThemeMode::Light)),
                        MenuItem::action(dark_label, SwitchThemeMode(ThemeMode::Dark)),
                    ],
                }),
                MenuItem::Separator,
                MenuItem::action("Quit", Quit),
            ],
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::action("Command Palette...", CommandPalette),
                MenuItem::action("Insert Image...", InsertImage),
                MenuItem::action("Set Link...", SetLink),
                MenuItem::action("Find...", Find),
                MenuItem::action("Find Next", FindNext),
                MenuItem::action("Find Previous", FindPrev),
                MenuItem::separator(),
                MenuItem::action("Undo", gpui_manos_plate::Undo),
                MenuItem::action("Redo", gpui_manos_plate::Redo),
                MenuItem::separator(),
                MenuItem::action("Cut", gpui_manos_plate::Cut),
                MenuItem::action("Copy", gpui_manos_plate::Copy),
                MenuItem::action("Paste", gpui_manos_plate::Paste),
                MenuItem::separator(),
                MenuItem::action("Select All", gpui_manos_plate::SelectAll),
            ],
        },
    ]);
}

fn theme_menu(cx: &App) -> MenuItem {
    let themes = ThemeRegistry::global(cx).sorted_themes();
    let current_name = cx.theme().theme_name();

    MenuItem::Submenu(Menu {
        name: "Theme".into(),
        items: themes
            .iter()
            .map(|theme| {
                let checked = current_name == &theme.name;
                let label: SharedString = if checked {
                    format!("✓ {}", theme.name).into()
                } else {
                    theme.name.clone()
                };
                MenuItem::action(label, SwitchTheme(theme.name.clone()))
            })
            .collect(),
    })
}
