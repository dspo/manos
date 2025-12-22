use gpui::{Action, App, Entity, Menu, MenuItem, SharedString, Window};
use gpui_component::{ActiveTheme as _, ThemeMode, ThemeRegistry, menu::AppMenuBar};

use crate::themes::{SwitchTheme, SwitchThemeMode};

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
pub struct Quit;

pub fn init(
    title: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<AppMenuBar> {
    cx.on_action(|_: &Quit, cx: &mut App| {
        cx.quit();
    });

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
