use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _, TitleBar,
    WindowExt as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
    menu::AppMenuBar,
    notification::Notification,
    popover::Popover,
};
use gpui_manos_components::plate_toolbar::{
    PlateIconName, PlateToolbarColorPicker, PlateToolbarDropdownButton, PlateToolbarIconButton,
    PlateToolbarSeparator,
};
use gpui_manos_plate::{PlateValue, RichTextState};

use crate::app_menus::{About, Open, Save, SaveAs};

pub struct RichTextExample {
    app_menu_bar: Entity<AppMenuBar>,
    editor: Entity<RichTextState>,
    file_path: Option<PathBuf>,
    link_input: Entity<InputState>,
}

impl RichTextExample {
    pub fn new(
        app_menu_bar: Entity<AppMenuBar>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor = cx.new(|cx| RichTextState::new(window, cx));
        let focus_handle = editor.read(cx).focus_handle();
        window.focus(&focus_handle);

        let link_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("https://example.com", window, cx);
            state
        });

        Self {
            app_menu_bar,
            editor,
            file_path: None,
            link_input,
        }
    }

    pub fn view(
        app_menu_bar: Entity<AppMenuBar>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| Self::new(app_menu_bar, window, cx))
    }

    fn open_link_dialog(&self, initial: String, window: &mut Window, cx: &mut Context<Self>) {
        let link_input = self.link_input.clone();
        let editor = self.editor.clone();

        link_input.update(cx, |state, cx| {
            state.set_value(initial, window, cx);
        });

        window.open_dialog(cx, move |dialog, window, cx| {
            let link_input = link_input.clone();
            let editor = editor.clone();
            let handle = link_input.focus_handle(cx);
            window.focus(&handle);
            dialog
                .title("Set Link")
                .w(px(520.))
                .child(div().p(px(12.)).child(Input::new(&link_input).w_full()))
                .on_ok(move |_, window, cx| {
                    let url = link_input.read(cx).value().trim().to_string();
                    editor.update(cx, |editor, cx| {
                        if url.is_empty() {
                            editor.command_unset_link(cx);
                        } else {
                            editor.command_set_link(url, cx);
                        }
                    });
                    let handle = editor.read(cx).focus_handle();
                    window.focus(&handle);
                    true
                })
        });
    }

    fn open_from_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open Plate JSON".into()),
        });

        let editor = self.editor.clone();
        let this = cx.entity();
        cx.spawn_in(window, async move |_, window| {
            let path: PathBuf = picked.await.ok()?.ok()??.into_iter().next()?;
            let content = std::fs::read_to_string(&path).ok()?;
            let parsed = PlateValue::from_json_str(&content).ok();
            window
                .update(|window, cx| match parsed {
                    Some(value) => {
                        _ = this.update(cx, |this, _| this.file_path = Some(path.clone()));
                        _ = editor.update(cx, |editor, cx| editor.load_plate_value(value, cx));
                    }
                    None => {
                        window.push_notification(
                            Notification::new().message("Failed to parse JSON"),
                            cx,
                        );
                    }
                })
                .ok();
            Some(())
        })
        .detach();
    }

    fn save_to_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.editor.read(cx).plate_value();
        let json = value.to_json_pretty().unwrap_or_else(|_| "{}".to_string());

        if let Some(path) = self.file_path.clone() {
            if std::fs::write(&path, json).is_ok() {
                window.push_notification(
                    Notification::new().message(format!("Saved to {}", path.display())),
                    cx,
                );
            }
            return;
        }

        self.save_as(window, cx);
    }

    fn save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.editor.read(cx).plate_value();
        let json = value.to_json_pretty().unwrap_or_else(|_| "{}".to_string());

        let directory = self
            .file_path
            .as_ref()
            .and_then(|path| path.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let suggested_name = self
            .file_path
            .as_ref()
            .and_then(|p| p.file_name().map(|name| name.to_string_lossy().to_string()))
            .unwrap_or_else(|| "plate.json".to_string());

        let picked = cx.prompt_for_new_path(&directory, Some(&suggested_name));
        let this = cx.entity();

        cx.spawn_in(window, async move |_, window| {
            let path: PathBuf = picked.await.ok()?.ok()??;
            match std::fs::write(&path, json) {
                Ok(()) => {
                    window
                        .update(|window, cx| {
                            _ = this.update(cx, |this, _| this.file_path = Some(path.clone()));
                            window.push_notification(
                                Notification::new().message(format!("Saved to {}", path.display())),
                                cx,
                            );
                        })
                        .ok();
                }
                Err(err) => {
                    window
                        .update(|window, cx| {
                            window.push_notification(
                                Notification::new().message(format!("Failed to save: {err}")),
                                cx,
                            );
                        })
                        .ok();
                }
            }
            Some(())
        })
        .detach();
    }
}

impl Render for RichTextExample {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (can_undo, can_redo, marks, bulleted, ordered, table_active, heading_level, quote) = {
            let editor = self.editor.read(cx);
            (
                editor.can_undo(),
                editor.can_redo(),
                editor.active_marks(),
                editor.is_bulleted_list_active(),
                editor.is_ordered_list_active(),
                editor.is_table_active(),
                editor.heading_level(),
                editor.is_blockquote_active(),
            )
        };
        let bold = marks.bold;
        let italic = marks.italic;
        let underline = marks.underline;
        let strikethrough = marks.strikethrough;
        let code = marks.code;
        let link = marks.link.is_some();
        let text_color = marks.text_color.as_deref().and_then(parse_hex_color);
        let highlight_color = marks.highlight_color.as_deref().and_then(parse_hex_color);
        let heading_label = match heading_level.unwrap_or(0).clamp(0, 6) {
            1 => "Heading 1".to_string(),
            2 => "Heading 2".to_string(),
            3 => "Heading 3".to_string(),
            4 => "Heading 4".to_string(),
            5 => "Heading 5".to_string(),
            6 => "Heading 6".to_string(),
            _ => "Paragraph".to_string(),
        };

        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(theme.muted)
            .on_action(cx.listener(|this, _: &Open, window, cx| {
                this.open_from_file(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Save, window, cx| {
                this.save_to_file(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveAs, window, cx| {
                this.save_as(window, cx);
            }))
            .on_action(cx.listener(|_, _: &About, window, cx| {
                window.push_notification(
                    Notification::new()
                        .message("Manos Rich Text (Plate Core)")
                        .autohide(true),
                    cx,
                );
            }))
            .child(
                TitleBar::new().child(div().flex().items_center().child(self.app_menu_bar.clone())),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .p(px(8.))
                    .bg(theme.background)
                    .border_b_1()
                    .border_color(theme.border)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        PlateToolbarIconButton::new("undo", PlateIconName::Undo2)
                            .disabled(!can_undo)
                            .tooltip("Undo")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor.update(cx, |ed, cx| ed.command_undo(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("redo", PlateIconName::Redo2)
                            .disabled(!can_redo)
                            .tooltip("Redo")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor.update(cx, |ed, cx| ed.command_redo(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(PlateToolbarSeparator)
                    .child(
                        PlateToolbarIconButton::new("divider", PlateIconName::Minus)
                            .tooltip("Insert divider (Cmd/Ctrl+Shift+D)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_insert_divider(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("mention", PlateIconName::MessageSquareText)
                            .tooltip("Insert mention (Cmd/Ctrl+Shift+M)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor.update(cx, |ed, cx| {
                                    ed.command_insert_mention("Alice".into(), cx)
                                });
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("table", PlateIconName::Table)
                            .tooltip("Insert table (2×2)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_insert_table(2, 2, cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("table-row", PlateIconName::ArrowDownToLine)
                            .disabled(!table_active)
                            .tooltip("Insert row below")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_insert_table_row_below(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("table-col", PlateIconName::IndentIncrease)
                            .disabled(!table_active)
                            .tooltip("Insert column right")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_insert_table_col_right(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new(
                            "table-delete-row",
                            PlateIconName::ArrowUpToLine,
                        )
                        .disabled(!table_active)
                        .tooltip("Delete row")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.editor
                                .update(cx, |ed, cx| ed.command_delete_table_row(cx));
                            let handle = this.editor.read(cx).focus_handle();
                            window.focus(&handle);
                        })),
                    )
                    .child(
                        PlateToolbarIconButton::new(
                            "table-delete-col",
                            PlateIconName::IndentDecrease,
                        )
                        .disabled(!table_active)
                        .tooltip("Delete column")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.editor
                                .update(cx, |ed, cx| ed.command_delete_table_col(cx));
                            let handle = this.editor.read(cx).focus_handle();
                            window.focus(&handle);
                        })),
                    )
                    .child(PlateToolbarSeparator)
                    .child({
                        let editor = self.editor.clone();
                        let heading_label = heading_label.clone();
                        let heading_level = heading_level;

                        Popover::new("plate-toolbar-heading-menu")
                            .appearance(false)
                            .trigger(
                                PlateToolbarDropdownButton::new("plate-toolbar-heading-trigger")
                                    .min_width(px(120.))
                                    .tooltip("Heading")
                                    .child(div().child(heading_label))
                                    .on_click(|_, _, _| {}),
                            )
                            .content(move |_, _window, cx| {
                                let theme = cx.theme();
                                let popover = cx.entity();

                                let editor_for_items = editor.clone();
                                let make_item =
                                    |id: &'static str,
                                     label: &'static str,
                                     target: Option<u64>,
                                     selected: bool| {
                                        let popover = popover.clone();
                                        let editor = editor_for_items.clone();
                                        div()
                                            .id(id)
                                            .flex()
                                            .items_center()
                                            .justify_start()
                                            .h(px(28.))
                                            .px(px(10.))
                                            .rounded(px(6.))
                                            .bg(theme.transparent)
                                            .text_color(theme.popover_foreground)
                                            .cursor_pointer()
                                            .hover(|this| {
                                                this.bg(theme.accent)
                                                    .text_color(theme.accent_foreground)
                                            })
                                            .when(selected, |this| {
                                                this.bg(theme.accent)
                                                    .text_color(theme.accent_foreground)
                                            })
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                move |_, window, cx| {
                                                    window.prevent_default();
                                                    editor.update(cx, |editor, cx| {
                                                        if let Some(level) = target {
                                                            editor.command_set_heading(level, cx);
                                                        } else {
                                                            editor.command_unset_heading(cx);
                                                        }
                                                    });
                                                    let handle = editor.read(cx).focus_handle();
                                                    window.focus(&handle);
                                                    popover.update(cx, |state, cx| {
                                                        state.dismiss(window, cx);
                                                    });
                                                },
                                            )
                                            .child(label)
                                    };

                                let selected_level = heading_level;
                                div()
                                    .p(px(6.))
                                    .bg(theme.popover)
                                    .border_1()
                                    .border_color(theme.border)
                                    .rounded(theme.radius)
                                    .shadow_md()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(make_item(
                                        "plate-toolbar-heading-paragraph",
                                        "Paragraph",
                                        None,
                                        selected_level.is_none(),
                                    ))
                                    .child(make_item(
                                        "plate-toolbar-heading-1",
                                        "Heading 1",
                                        Some(1),
                                        selected_level == Some(1),
                                    ))
                                    .child(make_item(
                                        "plate-toolbar-heading-2",
                                        "Heading 2",
                                        Some(2),
                                        selected_level == Some(2),
                                    ))
                                    .child(make_item(
                                        "plate-toolbar-heading-3",
                                        "Heading 3",
                                        Some(3),
                                        selected_level == Some(3),
                                    ))
                            })
                    })
                    .child(
                        PlateToolbarIconButton::new("bold", PlateIconName::Bold)
                            .selected(bold)
                            .tooltip("Bold (Cmd/Ctrl+B)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor.update(cx, |ed, cx| ed.command_toggle_bold(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("italic", PlateIconName::Italic)
                            .selected(italic)
                            .tooltip("Italic (Cmd/Ctrl+I)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_toggle_italic(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("underline", PlateIconName::Underline)
                            .selected(underline)
                            .tooltip("Underline (Cmd/Ctrl+U)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_toggle_underline(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("strikethrough", PlateIconName::Strikethrough)
                            .selected(strikethrough)
                            .tooltip("Strikethrough (Cmd/Ctrl+Shift+X)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_toggle_strikethrough(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("code", PlateIconName::CodeXml)
                            .selected(code)
                            .tooltip("Code (Cmd/Ctrl+E)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor.update(cx, |ed, cx| ed.command_toggle_code(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarColorPicker::new("text-color", PlateIconName::Baseline)
                            .tooltip("Text color")
                            .value(text_color)
                            .on_change({
                                let editor = self.editor.clone();
                                move |color, window, cx| {
                                    editor.update(cx, |editor, cx| {
                                        if let Some(color) = color {
                                            editor.command_set_text_color(hsla_to_hex(color), cx);
                                        } else {
                                            editor.command_unset_text_color(cx);
                                        }
                                    });
                                    let handle = editor.read(cx).focus_handle();
                                    window.focus(&handle);
                                }
                            }),
                    )
                    .child(
                        PlateToolbarColorPicker::new("highlight-color", PlateIconName::PaintBucket)
                            .tooltip("Highlight color")
                            .value(highlight_color)
                            .on_change({
                                let editor = self.editor.clone();
                                move |color, window, cx| {
                                    editor.update(cx, |editor, cx| {
                                        if let Some(color) = color {
                                            editor.command_set_highlight_color(
                                                hsla_to_hex(color),
                                                cx,
                                            );
                                        } else {
                                            editor.command_unset_highlight_color(cx);
                                        }
                                    });
                                    let handle = editor.read(cx).focus_handle();
                                    window.focus(&handle);
                                }
                            }),
                    )
                    .child(
                        PlateToolbarIconButton::new("list", PlateIconName::List)
                            .selected(bulleted)
                            .tooltip("Bulleted list (Cmd/Ctrl+Shift+8)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_toggle_bulleted_list(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("list-ordered", PlateIconName::ListOrdered)
                            .selected(ordered)
                            .tooltip("Ordered list (Cmd/Ctrl+Shift+7)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_toggle_ordered_list(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("blockquote", PlateIconName::WrapText)
                            .selected(quote)
                            .tooltip("Blockquote")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_toggle_blockquote(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("link", PlateIconName::Link)
                            .selected(link)
                            .tooltip(if link {
                                "Edit link (clear URL to remove)"
                            } else {
                                "Set link"
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                let initial =
                                    this.editor.read(cx).active_link_url().unwrap_or_default();
                                this.open_link_dialog(initial, window, cx);
                            })),
                    )
                    .child(PlateToolbarSeparator)
                    .child(
                        Button::new("open")
                            .icon(IconName::FolderOpen)
                            .small()
                            .ghost()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_from_file(window, cx)),
                            ),
                    )
                    .child(Button::new("save").small().ghost().on_click(
                        cx.listener(|this, _, window, cx| this.save_to_file(window, cx)),
                    )),
            )
            .child(div().flex_1().min_h(px(0.)).child(self.editor.clone()))
    }
}

fn parse_hex_color(value: &str) -> Option<Hsla> {
    Rgba::try_from(value).ok().map(Hsla::from)
}

fn hsla_to_hex(color: Hsla) -> String {
    let rgba: Rgba = color.into();
    let r = (rgba.r * 255.0).round() as u8;
    let g = (rgba.g * 255.0).round() as u8;
    let b = (rgba.b * 255.0).round() as u8;
    let a = (rgba.a * 255.0).round() as u8;
    format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
}
