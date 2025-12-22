use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    ActiveTheme as _, IconName, PixelsExt as _, Root, Selectable as _, Sizable as _, Theme,
    ThemeMode, ThemeRegistry, TitleBar, WindowExt as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
    menu::AppMenuBar,
    notification::Notification,
    popover::Popover,
    scroll::ScrollbarShow,
};
use gpui_manos_components::plate_toolbar::{
    PlateIconName, PlateToolbarButton, PlateToolbarColorPicker, PlateToolbarDropdownButton,
    PlateToolbarIconButton, PlateToolbarRounding, PlateToolbarSeparator,
};
use gpui_manos_plate::{
    BlockAlign, BlockKind, BlockTextSize, OrderedListStyle, RichTextDocument, RichTextEditor,
    RichTextState, RichTextTheme, RichTextValue, SlateNode,
};

use crate::app_menus::{About, Open, Save, SaveAs};

fn demo_richtext_value(_theme: &gpui_component::Theme) -> RichTextValue {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../richtext.example.json");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| include_str!("../../../richtext.example.json").to_string());
    RichTextExample::parse_richtext_value(&content).0
}

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
        let theme = cx.theme().clone();
        let rich_text_theme = RichTextTheme {
            background: theme.background,
            border: theme.border,
            radius: theme.radius,
            foreground: theme.foreground,
            muted_foreground: theme.muted_foreground,
            link: theme.blue,
            selection: theme.selection,
        };

        let editor = cx.new(|cx| {
            RichTextState::new(window, cx)
                .theme(rich_text_theme)
                .default_richtext_value(demo_richtext_value(&theme))
        });
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

    fn parse_richtext_value(content: &str) -> (RichTextValue, Option<String>) {
        if let Ok(value) = serde_json::from_str::<RichTextValue>(content) {
            return (value, None);
        }

        if let Ok(nodes) = serde_json::from_str::<Vec<SlateNode>>(content) {
            return (RichTextValue::from_slate_document(nodes), None);
        }

        let fallback = RichTextValue::from_document(&RichTextDocument::from_plain_text(content));
        let warning = "Opened as plain text because the JSON could not be parsed".to_string();
        (fallback, Some(warning))
    }

    fn open_link_dialog(&self, initial: String, window: &mut Window, cx: &mut Context<Self>) {
        let link_input = self.link_input.clone();
        let editor = self.editor.clone();

        link_input.update(cx, |state, cx| {
            state.set_value(initial, window, cx);
        });

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let link_input = link_input.clone();
            let editor = editor.clone();
            dialog
                .title("Set Link")
                .w(px(520.))
                .child(div().p(px(12.)).child(Input::new(&link_input).w_full()))
                .on_ok(move |_, window, cx| {
                    let url = link_input.read(cx).value().to_string();
                    editor.update(cx, |editor, cx| {
                        editor.set_link_at_cursor_or_selection(Some(url), window, cx);
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
            prompt: Some("Open RichText JSON".into()),
        });

        let editor = self.editor.clone();
        let this = cx.entity();
        cx.spawn_in(window, async move |_, window| {
            let path: PathBuf = picked.await.ok()?.ok()??.into_iter().next()?;
            let content = std::fs::read_to_string(&path).ok()?;

            let (value, warning) = Self::parse_richtext_value(&content);
            let path_for_state = path.clone();
            window
                .update(|window, cx| {
                    _ = this.update(cx, |this, _cx| {
                        this.file_path = Some(path_for_state);
                    });
                    _ = editor.update(cx, |editor, cx| {
                        editor.load_richtext_value(value, window, cx);
                    });
                    if let Some(message) = warning.clone() {
                        window.push_notification(Notification::new().message(message), cx);
                    }
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.editor.read(cx).richtext_value();
        let json = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string());

        let directory = self
            .file_path
            .as_ref()
            .and_then(|path| path.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let suggested_name = self
            .file_path
            .as_ref()
            .and_then(|p| p.file_name().map(|name| name.to_string_lossy().to_string()))
            .unwrap_or_else(|| "richtext.json".to_string());

        let picked = cx.prompt_for_new_path(&directory, Some(&suggested_name));
        let this = cx.entity();

        cx.spawn_in(window, async move |_, window| {
            let path: PathBuf = picked.await.ok()?.ok()??;

            match std::fs::write(&path, json) {
                Ok(()) => {
                    let path_for_state = path.clone();
                    let message = format!("Saved to {}", path.display());
                    window
                        .update(|window, cx| {
                            _ = this.update(cx, |this, _cx| {
                                this.file_path = Some(path_for_state);
                            });
                            window.push_notification(Notification::new().message(message), cx);
                        })
                        .ok();
                }
                Err(err) => {
                    let message = format!("Failed to save: {err}");
                    window
                        .update(|window, cx| {
                            window.push_notification(Notification::new().message(message), cx);
                        })
                        .ok();
                }
            }

            Some(())
        })
        .detach();
    }

    fn save_to_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.editor.read(cx).richtext_value();
        let json = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string());

        if let Some(path) = self.file_path.clone() {
            cx.spawn_in(window, async move |_, window| {
                match std::fs::write(&path, json) {
                    Ok(()) => {
                        let message = format!("Saved to {}", path.display());
                        window
                            .update(|window, cx| {
                                window.push_notification(Notification::new().message(message), cx);
                            })
                            .ok();
                    }
                    Err(err) => {
                        let message = format!("Failed to save: {err}");
                        window
                            .update(|window, cx| {
                                window.push_notification(Notification::new().message(message), cx);
                            })
                            .ok();
                    }
                }

                Some(())
            })
            .detach();
            return;
        }

        self.save_as(window, cx);
    }
}

impl Render for RichTextExample {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

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
                        .message("Manos Rich Text Editor")
                        .autohide(true),
                    cx,
                );
            }))
            .child(
                TitleBar::new()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(self.app_menu_bar.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .px(px(8.))
                            .gap(px(4.))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(
                                Button::new("github")
                                    .icon(IconName::GitHub)
                                    .small()
                                    .ghost()
                                    .on_click(|_, _, cx| {
                                        cx.open_url("https://github.com/dspo/manos");
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .flex_none()
                    .bg(theme.background)
                    .border_b_1()
	                    .border_color(theme.border)
	                    .p(px(8.))
	                    .child(
	                        div()
	                            .flex()
	                            .w_full()
	                            .flex_row()
	                            .flex_wrap()
	                            .justify_center()
	                            .gap(px(4.))
	                            .items_center()
	                            .child(
	                                PlateToolbarIconButton::new("bold", PlateIconName::Bold)
	                                    .selected(self.editor.read(cx).bold_mark_active())
	                                    .tooltip("Bold")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_bold_mark(window, cx);
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(
                                PlateToolbarIconButton::new("italic", PlateIconName::Italic)
                                    .selected(self.editor.read(cx).italic_mark_active())
                                    .tooltip("Italic")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_italic_mark(window, cx);
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(
                                PlateToolbarIconButton::new("underline", PlateIconName::Underline)
                                    .selected(self.editor.read(cx).underline_mark_active())
                                    .tooltip("Underline")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_underline_mark(window, cx);
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(
                                PlateToolbarIconButton::new("strike", PlateIconName::Strikethrough)
                                    .selected(self.editor.read(cx).strikethrough_mark_active())
                                    .tooltip("Strikethrough")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_strikethrough_mark(window, cx);
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(PlateToolbarSeparator)
                            .child(
                                PlateToolbarIconButton::new("link", PlateIconName::Link)
                                    .selected(self.editor.read(cx).current_link_url().is_some())
                                    .tooltip("Set or edit link URL")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let current = this.editor.read(cx).current_link_url();
                                        let initial = current.unwrap_or_default();
                                        this.open_link_dialog(initial, window, cx);
                                    })),
                            )
                            .child(
                                PlateToolbarIconButton::new("unlink", PlateIconName::Unlink)
                                    .tooltip("Remove link")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor
                                                .set_link_at_cursor_or_selection(None, window, cx);
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(PlateToolbarSeparator)
                            .child({
                                let editor = self.editor.clone();
                                let active_kind = editor.read(cx).active_block_kind();
                                let trigger_label: SharedString = match active_kind {
                                    Some(BlockKind::Heading { level }) => {
                                        format!("Heading\u{00A0}{level}").into()
                                    }
                                    Some(BlockKind::Paragraph) => "Text".into(),
                                    Some(BlockKind::Quote) => "Quote".into(),
                                    Some(BlockKind::UnorderedListItem) => "Bulleted list".into(),
                                    Some(BlockKind::OrderedListItem) => "Numbered list".into(),
                                    Some(BlockKind::Todo { .. }) => "To-do list".into(),
                                    Some(BlockKind::Divider) => "Divider".into(),
                                    None => "Turn into".into(),
                                };

                                Popover::new("richtext-turn-into-menu")
                                    .appearance(false)
                                    .trigger(
                                        PlateToolbarDropdownButton::new(
                                            "richtext-turn-into-menu-trigger",
                                        )
                                        .tooltip("Turn into")
                                        .min_width(px(140.))
                                        .child(trigger_label)
                                        .on_click(|_, _, _| {}),
                                    )
                                    .content(move |_, _window, cx| {
                                        let theme = cx.theme();
                                        let popover = cx.entity();

	                                        let active_kind = editor.read(cx).active_block_kind();
	                                        let editor_for_items = editor.clone();
	                                        let popover_for_items = popover.clone();

	                                        #[derive(Clone, Copy)]
	                                        enum TurnIntoAction {
	                                            SetKind(BlockKind),
	                                            SetOrderedListFromSelection,
	                                        }

	                                        let make_item = move |id: &'static str,
	                                                              prefix: &'static str,
	                                                              label: &'static str,
	                                                              action: TurnIntoAction,
	                                                              selected: bool,
	                                                              disabled: bool| {
	                                            let editor = editor_for_items.clone();
	                                            let popover = popover_for_items.clone();

		                                            div()
		                                                .id(id)
		                                                .flex()
		                                                .items_center()
		                                                .h(px(32.))
		                                                .w_full()
		                                                .min_w(px(180.))
		                                                .px(px(8.))
		                                                .text_size(px(12.))
		                                                .font_weight(FontWeight::MEDIUM)
		                                                .rounded(px(4.))
		                                                .bg(theme.transparent)
		                                                .text_color(theme.popover_foreground)
		                                                .when(disabled, |this| {
		                                                    this.opacity(0.5).cursor_not_allowed()
	                                                })
	                                                .when(!disabled, |this| {
	                                                    this.cursor_pointer()
	                                                        .hover(|this| {
	                                                            this.bg(theme.accent).text_color(
	                                                                theme.accent_foreground,
	                                                            )
	                                                        })
	                                                        .active(|this| {
	                                                            this.bg(theme.accent).text_color(
	                                                                theme.accent_foreground,
	                                                            )
	                                                        })
	                                                })
		                                                .when(!disabled && selected, |this| {
		                                                    this.bg(theme.accent)
		                                                        .text_color(theme.accent_foreground)
		                                                })
		                                                .child(
		                                                    div()
		                                                        .flex()
		                                                        .flex_1()
		                                                        .min_w(px(0.))
		                                                        .items_center()
		                                                        .gap(px(8.))
		                                                        .child(
		                                                            div()
		                                                                .flex_none()
		                                                                .w(px(32.))
		                                                                .text_center()
		                                                                .text_size(px(12.))
		                                                                .font_weight(FontWeight::SEMIBOLD)
		                                                                .child(prefix),
		                                                        )
		                                                        .child(
		                                                            div()
		                                                                .flex_1()
		                                                                .min_w(px(0.))
		                                                                .child(label),
		                                                        ),
		                                                )
		                                                .when(selected, |this| {
		                                                    this.child(
		                                                        div()
		                                                            .text_size(px(12.))
                                                            .font_weight(FontWeight::SEMIBOLD)
	                                                            .child("✓"),
	                                                    )
	                                                })
	                                                .when(!disabled, |this| {
	                                                    this.on_mouse_down(
	                                                        MouseButton::Left,
	                                                        move |_, window, cx| {
	                                                            window.prevent_default();

	                                                            match action {
	                                                                TurnIntoAction::SetKind(kind) => {
	                                                                    editor.update(cx, |editor, cx| {
	                                                                        editor.set_block_kind(
	                                                                            kind, window, cx,
	                                                                        );
	                                                                    });
	                                                                }
	                                                                TurnIntoAction::SetOrderedListFromSelection => {
	                                                                    let style = editor
	                                                                        .read(cx)
	                                                                        .active_ordered_list_style()
	                                                                        .unwrap_or(
	                                                                            OrderedListStyle::Decimal,
	                                                                        );
	                                                                    editor.update(cx, |editor, cx| {
	                                                                        editor.set_ordered_list_style(
	                                                                            style, window, cx,
	                                                                        );
	                                                                    });
	                                                                }
	                                                            }

	                                                            let handle =
	                                                                editor.read(cx).focus_handle();
	                                                            window.focus(&handle);
	                                                            popover.update(cx, |state, cx| {
	                                                                state.dismiss(window, cx);
	                                                            });
	                                                        },
	                                                    )
	                                                })
	                                        };

	                                        div()
	                                            .p(px(4.))
	                                            .bg(theme.popover)
	                                            .border_1()
	                                            .border_color(theme.border)
	                                            .rounded(theme.radius)
	                                            .shadow_md()
	                                            .w(px(220.))
	                                            .flex()
	                                            .flex_col()
	                                            .gap(px(2.))
	                                            .child(
	                                                div()
                                                    .px(px(8.))
                                                    .py(px(6.))
                                                    .text_size(px(10.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme.muted_foreground)
                                                    .child("Turn into"),
                                            )
	                                            .child(make_item(
	                                                "richtext-turn-into-text",
	                                                "P",
	                                                "Text",
	                                                TurnIntoAction::SetKind(BlockKind::Paragraph),
	                                                matches!(active_kind, Some(BlockKind::Paragraph)),
	                                                false,
	                                            ))
	                                            .child(make_item(
	                                                "richtext-turn-into-h1",
	                                                "H1",
	                                                "Heading 1",
	                                                TurnIntoAction::SetKind(BlockKind::Heading { level: 1 }),
	                                                matches!(
	                                                    active_kind,
	                                                    Some(BlockKind::Heading { level: 1 })
	                                                ),
	                                                false,
	                                            ))
	                                            .child(make_item(
	                                                "richtext-turn-into-h2",
	                                                "H2",
	                                                "Heading 2",
	                                                TurnIntoAction::SetKind(BlockKind::Heading { level: 2 }),
	                                                matches!(
	                                                    active_kind,
	                                                    Some(BlockKind::Heading { level: 2 })
	                                                ),
	                                                false,
	                                            ))
	                                            .child(make_item(
	                                                "richtext-turn-into-h3",
	                                                "H3",
	                                                "Heading 3",
	                                                TurnIntoAction::SetKind(BlockKind::Heading { level: 3 }),
	                                                matches!(
	                                                    active_kind,
	                                                    Some(BlockKind::Heading { level: 3 })
	                                                ),
	                                                false,
	                                            ))
	                                            .child(make_item(
	                                                "richtext-turn-into-h4",
	                                                "H4",
	                                                "Heading 4",
	                                                TurnIntoAction::SetKind(BlockKind::Heading { level: 4 }),
	                                                matches!(
	                                                    active_kind,
	                                                    Some(BlockKind::Heading { level: 4 })
	                                                ),
	                                                false,
	                                            ))
	                                            .child(make_item(
	                                                "richtext-turn-into-h5",
	                                                "H5",
	                                                "Heading 5",
	                                                TurnIntoAction::SetKind(BlockKind::Heading { level: 5 }),
	                                                matches!(
	                                                    active_kind,
	                                                    Some(BlockKind::Heading { level: 5 })
	                                                ),
	                                                false,
	                                            ))
	                                            .child(make_item(
	                                                "richtext-turn-into-h6",
	                                                "H6",
	                                                "Heading 6",
	                                                TurnIntoAction::SetKind(BlockKind::Heading { level: 6 }),
	                                                matches!(
	                                                    active_kind,
	                                                    Some(BlockKind::Heading { level: 6 })
	                                                ),
	                                                false,
	                                            ))
	                                            .child(div().h(px(1.)).bg(theme.border).my(px(4.)))
	                                            .child(make_item(
	                                                "richtext-turn-into-ul",
	                                                "•",
	                                                "Bulleted list",
	                                                TurnIntoAction::SetKind(
	                                                    BlockKind::UnorderedListItem,
	                                                ),
	                                                matches!(
	                                                    active_kind,
	                                                    Some(BlockKind::UnorderedListItem)
	                                                ),
	                                                false,
	                                            ))
	                                            .child(make_item(
	                                                "richtext-turn-into-ol",
	                                                "1.",
	                                                "Numbered list",
	                                                TurnIntoAction::SetOrderedListFromSelection,
	                                                matches!(
	                                                    active_kind,
	                                                    Some(BlockKind::OrderedListItem)
	                                                ),
	                                                false,
	                                            ))
	                                            .child(make_item(
	                                                "richtext-turn-into-todo",
	                                                "☐",
	                                                "To-do list",
	                                                TurnIntoAction::SetKind(BlockKind::Todo {
	                                                    checked: false,
	                                                }),
	                                                matches!(
	                                                    active_kind,
	                                                    Some(BlockKind::Todo { .. })
	                                                ),
	                                                false,
	                                            ))
	                                            .child(make_item(
	                                                "richtext-turn-into-toggle",
	                                                "▸",
	                                                "Toggle list",
	                                                TurnIntoAction::SetKind(BlockKind::Paragraph),
	                                                false,
	                                                true,
	                                            ))
	                                            .child(make_item(
	                                                "richtext-turn-into-code",
	                                                "</>",
	                                                "Code",
	                                                TurnIntoAction::SetKind(BlockKind::Paragraph),
	                                                false,
	                                                true,
	                                            ))
	                                            .child(make_item(
	                                                "richtext-turn-into-quote",
	                                                "❝",
	                                                "Quote",
	                                                TurnIntoAction::SetKind(BlockKind::Quote),
	                                                matches!(active_kind, Some(BlockKind::Quote)),
	                                                false,
	                                            ))
	                                            .child(make_item(
	                                                "richtext-turn-into-columns",
	                                                "|||",
	                                                "3 columns",
	                                                TurnIntoAction::SetKind(BlockKind::Paragraph),
	                                                false,
	                                                true,
	                                            ))
	                                    })
	                            })
                            .child(PlateToolbarSeparator)
                            .child(
                                PlateToolbarButton::new("size-sm")
                                    .tooltip("Text size: Small")
                                    .child("A-")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_size(BlockTextSize::Small, window, cx);
                                        });
                                    })),
                            )
                            .child(
                                PlateToolbarButton::new("size-md")
                                    .tooltip("Text size: Normal")
                                    .child("A")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_size(
                                                BlockTextSize::Normal,
                                                window,
                                                cx,
                                            );
                                        });
                                    })),
                            )
                            .child(
                                PlateToolbarButton::new("size-lg")
                                    .tooltip("Text size: Large")
                                    .child("A+")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_size(BlockTextSize::Large, window, cx);
                                        });
                                    })),
                            )
                            .child(PlateToolbarSeparator)
                            .child({
                                let editor = self.editor.clone();
                                let active_align = editor.read(cx).active_block_align();
                                let trigger_icon = match active_align {
                                    BlockAlign::Left => PlateIconName::AlignLeft,
                                    BlockAlign::Center => PlateIconName::AlignCenter,
                                    BlockAlign::Right => PlateIconName::AlignRight,
                                };

                                Popover::new("richtext-align-menu")
                                    .appearance(false)
                                    .trigger(
                                        PlateToolbarDropdownButton::new(
                                            "richtext-align-menu-trigger",
                                        )
                                        .tooltip("Align")
                                        .child(trigger_icon)
                                        .on_click(|_, _, _| {}),
                                    )
                                    .content(move |_, _window, cx| {
                                        let theme = cx.theme();
                                        let popover = cx.entity();

                                        let active_align = editor.read(cx).active_block_align();
                                        let editor_for_items = editor.clone();
                                        let popover_for_items = popover.clone();
                                        let make_item =
                                            move |id: &'static str,
                                                  icon: PlateIconName,
                                                  align: BlockAlign,
                                                  disabled: bool| {
                                                let editor = editor_for_items.clone();
                                                let popover = popover_for_items.clone();

                                                div()
                                                    .id(id)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .h(px(32.))
                                                    .w(px(32.))
                                                    .rounded(px(4.))
                                                    .bg(theme.transparent)
                                                    .text_color(theme.popover_foreground)
                                                    .when(disabled, |this| {
                                                        this.opacity(0.5).cursor_not_allowed()
                                                    })
                                                    .when(!disabled, |this| {
                                                        this.cursor_pointer()
                                                            .hover(|this| {
                                                                this.bg(theme.accent).text_color(
                                                                    theme.accent_foreground,
                                                                )
                                                            })
                                                            .active(|this| {
                                                                this.bg(theme.accent).text_color(
                                                                    theme.accent_foreground,
                                                                )
                                                            })
                                                            .on_mouse_down(
                                                                MouseButton::Left,
                                                                move |_, window, cx| {
                                                                    window.prevent_default();
                                                                    editor.update(cx, |editor, cx| {
                                                                        editor.set_block_align(
                                                                            align, window, cx,
                                                                        );
                                                                    });
                                                                    let handle = editor
                                                                        .read(cx)
                                                                        .focus_handle();
                                                                    window.focus(&handle);
                                                                    popover.update(cx, |state, cx| {
                                                                        state.dismiss(window, cx);
                                                                    });
                                                                },
                                                            )
                                                    })
                                                    .when(!disabled && active_align == align, |this| {
                                                        this.bg(theme.accent)
                                                            .text_color(theme.accent_foreground)
                                                    })
                                                    .child(gpui_component::Icon::new(icon))
                                            };

                                        div()
                                            .p(px(4.))
                                            .bg(theme.popover)
                                            .border_1()
                                            .border_color(theme.border)
                                            .rounded(theme.radius)
                                            .shadow_md()
                                            .flex()
                                            .flex_col()
                                            .gap(px(4.))
                                            .child(make_item(
                                                "richtext-align-left",
                                                PlateIconName::AlignLeft,
                                                BlockAlign::Left,
                                                false,
                                            ))
                                            .child(make_item(
                                                "richtext-align-center",
                                                PlateIconName::AlignCenter,
                                                BlockAlign::Center,
                                                false,
                                            ))
                                            .child(make_item(
                                                "richtext-align-right",
                                                PlateIconName::AlignRight,
                                                BlockAlign::Right,
                                                false,
                                            ))
                                            .child(make_item(
                                                "richtext-align-justify",
                                                PlateIconName::AlignJustify,
                                                BlockAlign::Left,
                                                true,
                                            ))
                                    })
                            })
                            .child(PlateToolbarSeparator)
                            .child(
                                PlateToolbarIconButton::new("ul", PlateIconName::List)
                                    .tooltip("Bulleted list")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_list(
                                                BlockKind::UnorderedListItem,
                                                window,
                                                cx,
                                            );
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(0.))
                                    .child({
                                        let editor = self.editor.clone();
                                        let ordered_list_active = matches!(
                                            editor.read(cx).active_block_kind(),
                                            Some(BlockKind::OrderedListItem)
                                        );

                                        PlateToolbarButton::new("ordered-list")
                                            .rounding(PlateToolbarRounding::Left)
                                            .tooltip("Numbered list")
                                            .selected(ordered_list_active)
                                            .child(PlateIconName::ListOrdered)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                let style = this
                                                    .editor
                                                    .read(cx)
                                                    .active_ordered_list_style()
                                                    .unwrap_or(OrderedListStyle::Decimal);
                                                this.editor.update(cx, |editor, cx| {
                                                    editor.toggle_ordered_list_any(style, window, cx);
                                                });
                                                let handle = this.editor.read(cx).focus_handle();
                                                window.focus(&handle);
                                            }))
                                    })
                                    .child({
                                        let editor = self.editor.clone();
                                        let ordered_list_active = matches!(
                                            editor.read(cx).active_block_kind(),
                                            Some(BlockKind::OrderedListItem)
                                        );

                                        Popover::new("ordered-list-style-menu")
                                            .appearance(false)
                                            .trigger(
                                                PlateToolbarButton::new("ordered-list-style-trigger")
                                                    .rounding(PlateToolbarRounding::Right)
                                                    .min_width(px(16.))
                                                    .padding_x(px(0.))
                                                    .selected(ordered_list_active)
                                                    .default_text_color(theme.muted_foreground)
                                                    .child(
                                                        gpui_component::Icon::new(
                                                            PlateIconName::ChevronDown,
                                                        )
                                                        .size_3p5(),
                                                    )
                                                    .on_click(|_, _, _| {}),
                                            )
                                            .content(move |_, _window, cx| {
                                                let theme = cx.theme();
                                                let popover = cx.entity();

                                                let active_style = editor
                                                    .read(cx)
                                                    .active_ordered_list_style()
                                                    .unwrap_or(OrderedListStyle::Decimal);

                                                let editor_for_items = editor.clone();
                                                let popover_for_items = popover.clone();

	                                                let make_item = move |id: &'static str,
	                                                                      label: &'static str,
	                                                                      style: OrderedListStyle| {
                                                    let editor = editor_for_items.clone();
                                                    let popover = popover_for_items.clone();
                                                    let selected = active_style == style;

	                                                    div()
	                                                        .id(id)
	                                                        .flex()
	                                                        .items_center()
	                                                        .h(px(32.))
	                                                        .w_full()
	                                                        .px(px(8.))
	                                                        .text_size(px(12.))
	                                                        .font_weight(FontWeight::MEDIUM)
	                                                        .rounded(px(4.))
	                                                        .bg(theme.transparent)
	                                                        .text_color(theme.popover_foreground)
	                                                        .cursor_pointer()
                                                        .hover(|this| {
                                                            this.bg(theme.accent)
                                                                .text_color(theme.accent_foreground)
                                                        })
                                                        .active(|this| {
                                                            this.bg(theme.accent)
                                                                .text_color(theme.accent_foreground)
                                                        })
                                                        .when(selected, |this| {
                                                            this.bg(theme.accent)
                                                                .text_color(theme.accent_foreground)
                                                        })
                                                        .child(label)
                                                        .on_mouse_down(
                                                            MouseButton::Left,
                                                            move |_, window, cx| {
                                                                window.prevent_default();
                                                                editor.update(cx, |editor, cx| {
                                                                    editor.set_ordered_list_style(
                                                                        style, window, cx,
                                                                    );
                                                                });
                                                                let handle =
                                                                    editor.read(cx).focus_handle();
                                                                window.focus(&handle);
                                                                popover.update(cx, |state, cx| {
                                                                    state.dismiss(window, cx);
                                                                });
                                                            },
                                                        )
                                                };

	                                                div()
	                                                    .p(px(4.))
	                                                    .bg(theme.popover)
	                                                    .border_1()
	                                                    .border_color(theme.border)
	                                                    .rounded(theme.radius)
	                                                    .shadow_md()
	                                                    .w(px(240.))
	                                                    .flex()
	                                                    .flex_col()
	                                                    .gap(px(2.))
	                                                    .child(make_item(
	                                                        "ordered-list-style-decimal",
                                                        "Decimal (1, 2, 3)",
                                                        OrderedListStyle::Decimal,
                                                    ))
                                                    .child(make_item(
                                                        "ordered-list-style-lower-alpha",
                                                        "Lower Alpha (a, b, c)",
                                                        OrderedListStyle::LowerAlpha,
                                                    ))
                                                    .child(make_item(
                                                        "ordered-list-style-upper-alpha",
                                                        "Upper Alpha (A, B, C)",
                                                        OrderedListStyle::UpperAlpha,
                                                    ))
                                                    .child(make_item(
                                                        "ordered-list-style-lower-roman",
                                                        "Lower Roman (i, ii, iii)",
                                                        OrderedListStyle::LowerRoman,
                                                    ))
                                                    .child(make_item(
                                                        "ordered-list-style-upper-roman",
                                                        "Upper Roman (I, II, III)",
                                                        OrderedListStyle::UpperRoman,
                                                    ))
                                            })
                                    }),
                            )
                            .child(
                                PlateToolbarIconButton::new("todo", PlateIconName::ListTodo)
                                    .tooltip("To-do list")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_todo_list(window, cx);
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(
                                PlateToolbarButton::new("quote")
                                    .tooltip("Quote")
                                    .child("❝")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_kind(BlockKind::Quote, window, cx);
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(
                                PlateToolbarButton::new("divider")
                                    .tooltip("Divider")
                                    .child("—")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.insert_divider(window, cx);
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(PlateToolbarSeparator)
                            .child(
                                PlateToolbarColorPicker::new("text-color", PlateIconName::Baseline)
                                    .tooltip("Text color")
                                    .value(self.editor.read(cx).active_text_color())
                                    .on_change({
                                        let editor = self.editor.clone();
                                        move |color, window, cx| {
                                            editor.update(cx, |editor, cx| {
                                                editor.set_text_color(color, window, cx);
                                            });
                                            let handle = editor.read(cx).focus_handle();
                                            window.focus(&handle);
                                        }
                                    }),
                            )
                            .child(
                                PlateToolbarColorPicker::new(
                                    "highlight-color",
                                    PlateIconName::PaintBucket,
                                )
                                .tooltip("Highlight color")
                                .value(self.editor.read(cx).active_highlight_color())
                                .on_change({
                                    let editor = self.editor.clone();
                                    move |color, window, cx| {
                                        editor.update(cx, |editor, cx| {
                                            editor.set_highlight_color(color, window, cx);
                                        });
                                        let handle = editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    }
                                }),
                            )
                            .when(false, |this| {
                                this.child(PlateToolbarSeparator)
                            .child({
                                let editor = self.editor.clone();
                                let theme_name = cx.theme().theme_name().clone();

                                Popover::new("richtext-themes-menu")
                                    .appearance(false)
                                    .trigger(
                                        PlateToolbarDropdownButton::new(
                                            "richtext-themes-menu-trigger",
                                        )
                                        .tooltip("Themes")
                                        .min_width(px(140.))
                                        .child(theme_name)
                                        .on_click(|_, _, _| {}),
                                    )
                                    .content(move |_, _window, cx| {
                                        let theme = cx.theme();
                                        let popover = cx.entity();

	                                        let themes = ThemeRegistry::global(cx).sorted_themes();
	                                        let current_name = cx.theme().theme_name().clone();
	                                        let is_dark = cx.theme().mode.is_dark();

	                                        let editor_for_items = editor.clone();
	                                        let popover_for_items = popover.clone();

	                                        #[derive(Clone)]
	                                        enum ThemeMenuAction {
	                                            SwitchTheme(SharedString),
	                                            SwitchMode(ThemeMode),
	                                        }

	                                        let make_item = move |id: SharedString,
	                                                              label: SharedString,
	                                                              selected: bool,
	                                                              action: ThemeMenuAction| {
	                                            let popover = popover_for_items.clone();
	                                            let editor = editor_for_items.clone();

	                                            div()
	                                                .id(id)
                                                .flex()
                                                .items_center()
                                                .h(px(32.))
                                                .w_full()
                                                .px(px(8.))
                                                .text_size(px(12.))
                                                .font_weight(FontWeight::MEDIUM)
                                                .rounded(px(4.))
                                                .bg(theme.transparent)
                                                .text_color(theme.popover_foreground)
                                                .cursor_pointer()
                                                .hover(|this| {
                                                    this.bg(theme.accent)
                                                        .text_color(theme.accent_foreground)
                                                })
                                                .active(|this| {
                                                    this.bg(theme.accent)
                                                        .text_color(theme.accent_foreground)
                                                })
                                                .when(selected, |this| {
                                                    this.bg(theme.accent)
                                                        .text_color(theme.accent_foreground)
                                                })
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w(px(0.))
                                                        .child(label),
                                                )
                                                .when(selected, |this| {
                                                    this.child(
                                                        div()
                                                            .text_size(px(12.))
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .child("✓"),
                                                    )
	                                                })
	                                                .on_mouse_down(
	                                                    MouseButton::Left,
	                                                    move |_, window, cx| {
	                                                        window.prevent_default();
	                                                        match action.clone() {
	                                                            ThemeMenuAction::SwitchTheme(
	                                                                theme_name,
	                                                            ) => {
	                                                                if let Some(theme_config) =
	                                                                    ThemeRegistry::global(cx)
	                                                                        .themes()
	                                                                        .get(&theme_name)
	                                                                        .cloned()
	                                                                {
	                                                                    Theme::global_mut(cx)
	                                                                        .apply_config(
	                                                                            &theme_config,
	                                                                        );
	                                                                }
	                                                                cx.refresh_windows();
	                                                            }
	                                                            ThemeMenuAction::SwitchMode(
	                                                                mode,
	                                                            ) => {
	                                                                Theme::change(mode, None, cx);
	                                                                cx.refresh_windows();
	                                                            }
	                                                        }

	                                                        let handle =
	                                                            editor.read(cx).focus_handle();
	                                                        window.focus(&handle);
	                                                        popover.update(cx, |state, cx| {
                                                            state.dismiss(window, cx);
                                                        });
                                                    },
                                                )
                                        };

                                        let mut menu = div()
                                            .p(px(4.))
                                            .bg(theme.popover)
                                            .border_1()
                                            .border_color(theme.border)
                                            .rounded(theme.radius)
                                            .shadow_md()
                                            .w(px(240.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.))
                                            .child(
                                                div()
                                                    .px(px(8.))
                                                    .py(px(6.))
                                                    .text_size(px(10.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme.muted_foreground)
                                                    .child("Themes"),
                                            );

                                        if themes.is_empty() {
                                            menu = menu.child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .h(px(32.))
                                                    .px(px(8.))
                                                    .rounded(px(4.))
                                                    .text_size(px(12.))
                                                    .text_color(theme.muted_foreground)
                                                    .child("No themes found"),
                                            );
                                        } else {
	                                            for (index, theme_config) in themes.into_iter().enumerate()
	                                            {
	                                                let name = theme_config.name.clone();
	                                                let selected = current_name == name;
	                                                menu = menu.child(make_item(
	                                                    format!("richtext-theme-{}", index).into(),
	                                                    name.clone(),
	                                                    selected,
	                                                    ThemeMenuAction::SwitchTheme(name.clone()),
	                                                ));
	                                            }
	                                        }

                                        menu = menu
                                            .child(
                                                div()
                                                    .h(px(1.))
                                                    .bg(theme.border)
                                                    .my(px(4.)),
                                            )
                                            .child(
                                                div()
                                                    .px(px(8.))
                                                    .py(px(6.))
                                                    .text_size(px(10.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme.muted_foreground)
                                                    .child("Appearance"),
                                            )
	                                            .child(make_item(
	                                                "richtext-theme-mode-light".into(),
	                                                "Light".into(),
	                                                !is_dark,
	                                                ThemeMenuAction::SwitchMode(ThemeMode::Light),
	                                            ))
	                                            .child(make_item(
	                                                "richtext-theme-mode-dark".into(),
	                                                "Dark".into(),
	                                                is_dark,
	                                                ThemeMenuAction::SwitchMode(ThemeMode::Dark),
	                                            ));

                                        menu
                                    })
                            })
                            .child({
                                let editor = self.editor.clone();
                                Popover::new("richtext-settings-menu")
                                    .appearance(false)
                                    .trigger(
                                        PlateToolbarIconButton::new(
                                            "richtext-settings-menu-trigger",
                                            IconName::Settings2,
                                        )
                                        .tooltip("Settings")
                                        .on_click(|_, _, _| {}),
                                    )
                                    .content(move |_, _window, cx| {
                                        let theme = cx.theme();
                                        let popover = cx.entity();

                                        let font_size = cx.theme().font_size.as_f32() as i32;
                                        let radius = cx.theme().radius.as_f32() as i32;
                                        let scroll_show = cx.theme().scrollbar_show;

                                        let editor_for_items = editor.clone();
                                        let popover_for_items = popover.clone();

                                        let make_item = move |id: SharedString,
                                                              label: SharedString,
                                                              selected: bool,
                                                              on_click: Box<
                                            dyn Fn(&mut Window, &mut App) + 'static,
                                        >| {
                                            let popover = popover_for_items.clone();
                                            let editor = editor_for_items.clone();

                                            div()
                                                .id(id)
                                                .flex()
                                                .items_center()
                                                .h(px(32.))
                                                .w_full()
                                                .px(px(8.))
                                                .text_size(px(12.))
                                                .font_weight(FontWeight::MEDIUM)
                                                .rounded(px(4.))
                                                .bg(theme.transparent)
                                                .text_color(theme.popover_foreground)
                                                .cursor_pointer()
                                                .hover(|this| {
                                                    this.bg(theme.accent)
                                                        .text_color(theme.accent_foreground)
                                                })
                                                .active(|this| {
                                                    this.bg(theme.accent)
                                                        .text_color(theme.accent_foreground)
                                                })
                                                .when(selected, |this| {
                                                    this.bg(theme.accent)
                                                        .text_color(theme.accent_foreground)
                                                })
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w(px(0.))
                                                        .child(label),
                                                )
                                                .when(selected, |this| {
                                                    this.child(
                                                        div()
                                                            .text_size(px(12.))
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .child("✓"),
                                                    )
                                                })
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    move |_, window, cx| {
                                                        window.prevent_default();
                                                        (on_click)(window, cx);

                                                        let handle =
                                                            editor.read(cx).focus_handle();
                                                        window.focus(&handle);
                                                        popover.update(cx, |state, cx| {
                                                            state.dismiss(window, cx);
                                                        });
                                                    },
                                                )
                                        };

                                        div()
                                            .p(px(4.))
                                            .bg(theme.popover)
                                            .border_1()
                                            .border_color(theme.border)
                                            .rounded(theme.radius)
                                            .shadow_md()
                                            .w(px(240.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.))
                                            .child(
                                                div()
                                                    .px(px(8.))
                                                    .py(px(6.))
                                                    .text_size(px(10.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme.muted_foreground)
                                                    .child("Font Size"),
                                            )
                                            .child(make_item(
                                                "richtext-settings-font-large".into(),
                                                "Large".into(),
                                                font_size == 18,
                                                Box::new(move |_, cx| {
                                                    Theme::global_mut(cx).font_size = px(18.);
                                                    cx.refresh_windows();
                                                }),
                                            ))
                                            .child(make_item(
                                                "richtext-settings-font-medium".into(),
                                                "Medium (default)".into(),
                                                font_size == 16,
                                                Box::new(move |_, cx| {
                                                    Theme::global_mut(cx).font_size = px(16.);
                                                    cx.refresh_windows();
                                                }),
                                            ))
                                            .child(make_item(
                                                "richtext-settings-font-small".into(),
                                                "Small".into(),
                                                font_size == 14,
                                                Box::new(move |_, cx| {
                                                    Theme::global_mut(cx).font_size = px(14.);
                                                    cx.refresh_windows();
                                                }),
                                            ))
                                            .child(
                                                div()
                                                    .h(px(1.))
                                                    .bg(theme.border)
                                                    .my(px(4.)),
                                            )
                                            .child(
                                                div()
                                                    .px(px(8.))
                                                    .py(px(6.))
                                                    .text_size(px(10.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme.muted_foreground)
                                                    .child("Border Radius"),
                                            )
                                            .child(make_item(
                                                "richtext-settings-radius-8".into(),
                                                "8px".into(),
                                                radius == 8,
                                                Box::new(move |_, cx| {
                                                    Theme::global_mut(cx).radius = px(8.);
                                                    cx.refresh_windows();
                                                }),
                                            ))
                                            .child(make_item(
                                                "richtext-settings-radius-6".into(),
                                                "6px (default)".into(),
                                                radius == 6,
                                                Box::new(move |_, cx| {
                                                    Theme::global_mut(cx).radius = px(6.);
                                                    cx.refresh_windows();
                                                }),
                                            ))
                                            .child(make_item(
                                                "richtext-settings-radius-4".into(),
                                                "4px".into(),
                                                radius == 4,
                                                Box::new(move |_, cx| {
                                                    Theme::global_mut(cx).radius = px(4.);
                                                    cx.refresh_windows();
                                                }),
                                            ))
                                            .child(make_item(
                                                "richtext-settings-radius-0".into(),
                                                "0px".into(),
                                                radius == 0,
                                                Box::new(move |_, cx| {
                                                    Theme::global_mut(cx).radius = px(0.);
                                                    cx.refresh_windows();
                                                }),
                                            ))
                                            .child(
                                                div()
                                                    .h(px(1.))
                                                    .bg(theme.border)
                                                    .my(px(4.)),
                                            )
                                            .child(
                                                div()
                                                    .px(px(8.))
                                                    .py(px(6.))
                                                    .text_size(px(10.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme.muted_foreground)
                                                    .child("Scrollbar"),
                                            )
                                            .child(make_item(
                                                "richtext-settings-scroll-scrolling".into(),
                                                "Scrolling to show".into(),
                                                scroll_show == ScrollbarShow::Scrolling,
                                                Box::new(move |_, cx| {
                                                    Theme::global_mut(cx).scrollbar_show =
                                                        ScrollbarShow::Scrolling;
                                                    cx.refresh_windows();
                                                }),
                                            ))
                                            .child(make_item(
                                                "richtext-settings-scroll-hover".into(),
                                                "Hover to show".into(),
                                                scroll_show == ScrollbarShow::Hover,
                                                Box::new(move |_, cx| {
                                                    Theme::global_mut(cx).scrollbar_show =
                                                        ScrollbarShow::Hover;
                                                    cx.refresh_windows();
                                                }),
                                            ))
                                            .child(make_item(
                                                "richtext-settings-scroll-always".into(),
                                                "Always show".into(),
                                                scroll_show == ScrollbarShow::Always,
                                                Box::new(move |_, cx| {
                                                    Theme::global_mut(cx).scrollbar_show =
                                                        ScrollbarShow::Always;
                                                    cx.refresh_windows();
                                                }),
                                            ))
                                    })
                            })
                            })
                    ),
            )
            .child(
                div().flex_1().min_h(px(0.)).p(px(16.)).child(
                    div().flex().flex_row().justify_center().size_full().child(
                        div()
                            .w_full()
                            .max_w(px(960.))
                            .h_full()
                            .min_h(px(0.))
                            .bg(theme.background)
                            .border_1()
                            .border_color(theme.border)
                            .rounded(px(12.))
                            .relative()
                            .child(
                                RichTextEditor::new(&self.editor)
                                    .bordered(false)
                                    .padding(px(24.))
                                    .size_full(),
                            ),
                    ),
                ),
            )
            .when_some(
                self.editor.read(cx).active_link_overlay(),
                |this, overlay| {
                    let url_for_edit = overlay.url.clone();
                    let url_for_open = overlay.url.clone();
                    let anchor = overlay.anchor;
                    let theme = cx.theme();
                    let make_button = |id: &'static str, label: &'static str| {
                        div()
                            .id(id)
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .h(px(32.))
                            .rounded(px(6.))
                            .px(px(12.))
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .bg(theme.transparent)
                            .text_color(theme.popover_foreground)
                            .hover(|this| this.bg(theme.accent).text_color(theme.accent_foreground))
                            .active(|this| {
                                this.bg(theme.accent).text_color(theme.accent_foreground)
                            })
                            .child(label)
                    };
                    this.child(
                        div()
                            .absolute()
                            .top(anchor.y)
                            .left(anchor.x)
                            .block_mouse_except_scroll()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .bg(theme.popover)
                            .border_1()
                            .border_color(theme.border)
                            .rounded(theme.radius)
                            .p(px(4.))
                            .shadow_md()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.))
                            .child(make_button("link-toolbar-edit", "Edit link").on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.open_link_dialog(url_for_edit.clone(), window, cx);
                                }),
                            ))
                            .child(make_button("link-toolbar-open", "Open").on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |_this, _, _window, cx| {
                                    cx.open_url(&url_for_open);
                                }),
                            ))
                            .child(div().w(px(1.)).h(px(22.)).bg(theme.border))
                            .child(make_button("link-toolbar-unlink", "Unlink").on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.editor.update(cx, |editor, cx| {
                                        editor.set_link_at_cursor_or_selection(None, window, cx);
                                    });
                                    let handle = this.editor.read(cx).focus_handle();
                                    window.focus(&handle);
                                }),
                            )),
                    )
                },
            )
            .when_some(Root::render_sheet_layer(window, cx), |this, layer| {
                this.child(layer)
            })
            .when_some(Root::render_dialog_layer(window, cx), |this, layer| {
                this.child(layer)
            })
            .when_some(
                Root::render_notification_layer(window, cx),
                |this, layer| this.child(layer),
            )
    }
}
