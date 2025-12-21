use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::ActiveTheme as _;
use gpui_component::Root;
use gpui_component::WindowExt as _;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputState};
use gpui_component::notification::Notification;
use gpui_component::tooltip::Tooltip;
use gpui_rich_text::{
    BlockAlign, BlockFormat, BlockKind, BlockNode, BlockTextSize, InlineNode, InlineStyle,
    OrderedListStyle, RichTextDocument, RichTextEditor, RichTextState, RichTextTheme,
    RichTextValue, SlateNode, TextNode, ToggleBold, ToggleItalic, ToggleStrikethrough,
    ToggleUnderline,
};

fn demo_richtext_value(theme: &gpui_component::Theme) -> RichTextValue {
    fn run(text: impl Into<String>, style: InlineStyle) -> InlineNode {
        InlineNode::Text(TextNode {
            text: text.into(),
            style,
        })
    }

    fn block(kind: BlockKind, size: BlockTextSize, runs: Vec<InlineNode>) -> BlockNode {
        let mut block = BlockNode {
            format: BlockFormat {
                kind,
                size,
                ..Default::default()
            },
            inlines: runs,
        };
        block.normalize();
        block
    }

    let normal = InlineStyle::default();
    let bold = InlineStyle {
        bold: true,
        ..InlineStyle::default()
    };
    let italic = InlineStyle {
        italic: true,
        ..InlineStyle::default()
    };
    let underline = InlineStyle {
        underline: true,
        ..InlineStyle::default()
    };
    let strike = InlineStyle {
        strikethrough: true,
        ..InlineStyle::default()
    };
    let red = InlineStyle {
        fg: Some(theme.red),
        ..InlineStyle::default()
    };
    let blue = InlineStyle {
        fg: Some(theme.blue),
        ..InlineStyle::default()
    };
    let highlight = InlineStyle {
        bg: Some(theme.yellow_light),
        ..InlineStyle::default()
    };
    let link = InlineStyle {
        fg: Some(theme.blue),
        link: Some("https://github.com/zed-industries/zed".to_string()),
        ..InlineStyle::default()
    };

    let mut blocks = Vec::new();

    blocks.push(block(
        BlockKind::Heading { level: 1 },
        BlockTextSize::Normal,
        vec![run("GPUI Rich Text Editor", bold.clone())],
    ));

    blocks.push(block(
        BlockKind::Paragraph,
        BlockTextSize::Normal,
        vec![
            run("This document demonstrates ", normal.clone()),
            run("headings", bold.clone()),
            run(", ", normal.clone()),
            run("inline marks", italic.clone()),
            run(", lists, quotes, dividers and ", normal.clone()),
            run("to-do items", underline.clone()),
            run(".", normal.clone()),
        ],
    ));

    blocks.push(block(
        BlockKind::Heading { level: 2 },
        BlockTextSize::Normal,
        vec![run("Headings", bold.clone())],
    ));
    blocks.push(block(
        BlockKind::Heading { level: 3 },
        BlockTextSize::Normal,
        vec![run("Heading 3", normal.clone())],
    ));
    blocks.push(block(
        BlockKind::Heading { level: 4 },
        BlockTextSize::Normal,
        vec![run("Heading 4", normal.clone())],
    ));
    blocks.push(block(
        BlockKind::Heading { level: 5 },
        BlockTextSize::Normal,
        vec![run("Heading 5", normal.clone())],
    ));
    blocks.push(block(
        BlockKind::Heading { level: 6 },
        BlockTextSize::Normal,
        vec![run("Heading 6", normal.clone())],
    ));

    blocks.push(block(
        BlockKind::Heading { level: 2 },
        BlockTextSize::Normal,
        vec![run("Inline Marks", bold.clone())],
    ));
    blocks.push(block(
        BlockKind::Paragraph,
        BlockTextSize::Normal,
        vec![
            run("Bold", bold.clone()),
            run(" / ", normal.clone()),
            run("Italic", italic.clone()),
            run(" / ", normal.clone()),
            run("Underline", underline.clone()),
            run(" / ", normal.clone()),
            run("Strikethrough", strike.clone()),
        ],
    ));
    blocks.push(block(
        BlockKind::Paragraph,
        BlockTextSize::Small,
        vec![
            run("Colors: ", normal.clone()),
            run("Red", red.clone()),
            run(", ", normal.clone()),
            run("Blue", blue.clone()),
            run(".  Highlight: ", normal.clone()),
            run("Yellow", highlight.clone()),
            run(".", normal.clone()),
        ],
    ));
    blocks.push(block(
        BlockKind::Paragraph,
        BlockTextSize::Normal,
        vec![
            run("Link (Cmd/Ctrl-click to open): ", normal.clone()),
            run("Zed Editor", link.clone()),
        ],
    ));

    blocks.push(block(
        BlockKind::Heading { level: 2 },
        BlockTextSize::Normal,
        vec![run("Alignment", bold.clone())],
    ));
    blocks.push(block(
        BlockKind::Paragraph,
        BlockTextSize::Normal,
        vec![run("Left aligned paragraph (default).", normal.clone())],
    ));

    let mut center = block(
        BlockKind::Paragraph,
        BlockTextSize::Normal,
        vec![run("Center aligned paragraph.", normal.clone())],
    );
    center.format.align = BlockAlign::Center;
    blocks.push(center);

    let mut right = block(
        BlockKind::Paragraph,
        BlockTextSize::Normal,
        vec![run("Right aligned paragraph.", normal.clone())],
    );
    right.format.align = BlockAlign::Right;
    blocks.push(right);

    blocks.push(block(
        BlockKind::Heading { level: 2 },
        BlockTextSize::Normal,
        vec![run("Lists", bold.clone())],
    ));
    blocks.push(block(
        BlockKind::UnorderedListItem,
        BlockTextSize::Normal,
        vec![run("Bulleted item", normal.clone())],
    ));
    blocks.push(block(
        BlockKind::UnorderedListItem,
        BlockTextSize::Normal,
        vec![
            run("Bulleted item with ", normal.clone()),
            run("bold", bold.clone()),
        ],
    ));

    blocks.push(block(
        BlockKind::Paragraph,
        BlockTextSize::Small,
        vec![run("Numbered list styles:", normal.clone())],
    ));

    let mut ol_decimal_1 = block(
        BlockKind::OrderedListItem,
        BlockTextSize::Normal,
        vec![run("Decimal (1, 2, 3)", normal.clone())],
    );
    ol_decimal_1.format.ordered_list_style = OrderedListStyle::Decimal;
    blocks.push(ol_decimal_1);
    let mut ol_decimal_2 = block(
        BlockKind::OrderedListItem,
        BlockTextSize::Normal,
        vec![run("Still decimal", normal.clone())],
    );
    ol_decimal_2.format.ordered_list_style = OrderedListStyle::Decimal;
    blocks.push(ol_decimal_2);

    blocks.push(block(
        BlockKind::Paragraph,
        BlockTextSize::Small,
        vec![run("Lower alpha:", normal.clone())],
    ));
    let mut ol_la_1 = block(
        BlockKind::OrderedListItem,
        BlockTextSize::Normal,
        vec![run("Lower alpha (a, b, c)", normal.clone())],
    );
    ol_la_1.format.ordered_list_style = OrderedListStyle::LowerAlpha;
    blocks.push(ol_la_1);
    let mut ol_la_2 = block(
        BlockKind::OrderedListItem,
        BlockTextSize::Normal,
        vec![run("Still lower alpha", normal.clone())],
    );
    ol_la_2.format.ordered_list_style = OrderedListStyle::LowerAlpha;
    blocks.push(ol_la_2);

    blocks.push(block(
        BlockKind::Paragraph,
        BlockTextSize::Small,
        vec![run("Upper alpha:", normal.clone())],
    ));
    let mut ol_ua_1 = block(
        BlockKind::OrderedListItem,
        BlockTextSize::Normal,
        vec![run("Upper alpha (A, B, C)", normal.clone())],
    );
    ol_ua_1.format.ordered_list_style = OrderedListStyle::UpperAlpha;
    blocks.push(ol_ua_1);
    let mut ol_ua_2 = block(
        BlockKind::OrderedListItem,
        BlockTextSize::Normal,
        vec![run("Still upper alpha", normal.clone())],
    );
    ol_ua_2.format.ordered_list_style = OrderedListStyle::UpperAlpha;
    blocks.push(ol_ua_2);

    blocks.push(block(
        BlockKind::Paragraph,
        BlockTextSize::Small,
        vec![run("Lower roman:", normal.clone())],
    ));
    let mut ol_lr_1 = block(
        BlockKind::OrderedListItem,
        BlockTextSize::Normal,
        vec![run("Lower roman (i, ii, iii)", normal.clone())],
    );
    ol_lr_1.format.ordered_list_style = OrderedListStyle::LowerRoman;
    blocks.push(ol_lr_1);
    let mut ol_lr_2 = block(
        BlockKind::OrderedListItem,
        BlockTextSize::Normal,
        vec![run("Still lower roman", normal.clone())],
    );
    ol_lr_2.format.ordered_list_style = OrderedListStyle::LowerRoman;
    blocks.push(ol_lr_2);

    blocks.push(block(
        BlockKind::Paragraph,
        BlockTextSize::Small,
        vec![run("Upper roman:", normal.clone())],
    ));
    let mut ol_ur_1 = block(
        BlockKind::OrderedListItem,
        BlockTextSize::Normal,
        vec![run("Upper roman (I, II, III)", normal.clone())],
    );
    ol_ur_1.format.ordered_list_style = OrderedListStyle::UpperRoman;
    blocks.push(ol_ur_1);
    let mut ol_ur_2 = block(
        BlockKind::OrderedListItem,
        BlockTextSize::Normal,
        vec![run("Still upper roman", normal.clone())],
    );
    ol_ur_2.format.ordered_list_style = OrderedListStyle::UpperRoman;
    blocks.push(ol_ur_2);

    blocks.push(block(
        BlockKind::Heading { level: 2 },
        BlockTextSize::Normal,
        vec![run("To-do List", bold.clone())],
    ));
    blocks.push(block(
        BlockKind::Todo { checked: false },
        BlockTextSize::Normal,
        vec![run("Unchecked task", normal.clone())],
    ));
    blocks.push(block(
        BlockKind::Todo { checked: true },
        BlockTextSize::Normal,
        vec![run("Checked task", normal.clone())],
    ));

    blocks.push(block(
        BlockKind::Heading { level: 2 },
        BlockTextSize::Normal,
        vec![run("Quote", bold.clone())],
    ));
    blocks.push(block(
        BlockKind::Quote,
        BlockTextSize::Normal,
        vec![run(
            "Blockquotes are perfect for highlighting important information.",
            normal.clone(),
        )],
    ));
    blocks.push(block(
        BlockKind::Quote,
        BlockTextSize::Normal,
        vec![run("They can span multiple paragraphs.", normal.clone())],
    ));

    blocks.push(block(BlockKind::Divider, BlockTextSize::Normal, vec![]));

    blocks.push(block(
        BlockKind::Paragraph,
        BlockTextSize::Large,
        vec![run("Divider above.", normal.clone())],
    ));

    RichTextValue::from_document(&RichTextDocument { blocks })
}

pub struct RichTextExample {
    editor: Entity<RichTextState>,
    file_path: Option<PathBuf>,
    link_input: Entity<InputState>,
}

impl RichTextExample {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
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
            editor,
            file_path: None,
            link_input,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
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
        let red = theme.red;
        let green = theme.green;
        let blue = theme.blue;
        let yellow_light = theme.yellow_light;
        let cyan_light = theme.cyan_light;
        let magenta_light = theme.magenta_light;

        let toolbar_button = |id: &'static str, label: &'static str, selected: bool| {
            div()
                .id(id)
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_center()
                .h(px(32.))
                .min_w(px(32.))
                .px(px(6.))
                .rounded(px(6.))
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .bg(theme.transparent)
                .text_color(theme.foreground)
                .hover(|this| this.bg(theme.muted).text_color(theme.muted_foreground))
                .active(|this| this.bg(theme.accent).text_color(theme.accent_foreground))
                .when(selected, |this| {
                    this.bg(theme.accent).text_color(theme.accent_foreground)
                })
                .child(label)
        };
        let toolbar_sep = || div().mx(px(6.)).h(px(18.)).w(px(1.)).bg(theme.border);

        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(theme.muted)
            .child(
                div()
                    .w_full()
                    .flex_none()
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .p(px(8.))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(4.))
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Rich Text"),
                            )
                            .child(div().w(px(12.)))
                            .child(
                                Button::new("open")
                                    .ghost()
                                    .label("Open")
                                    .tooltip("Open from JSON file")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_from_file(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("save")
                                    .ghost()
                                    .label("Save")
                                    .tooltip("Save to JSON file")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_to_file(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("save-as")
                                    .ghost()
                                    .label("Save As")
                                    .tooltip("Save to a new JSON file")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_as(window, cx);
                                    })),
                            )
                            .child(toolbar_sep())
                            .child(
                                toolbar_button(
                                    "bold",
                                    "B",
                                    self.editor.read(cx).bold_mark_active(),
                                )
                                .tooltip(|window, cx| {
                                    Tooltip::new("Bold")
                                        .action(&ToggleBold, Some("RichText"))
                                        .build(window, cx)
                                })
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_bold_mark(window, cx);
                                        });
                                    },
                                )),
                            )
                            .child(
                                toolbar_button(
                                    "italic",
                                    "I",
                                    self.editor.read(cx).italic_mark_active(),
                                )
                                .tooltip(|window, cx| {
                                    Tooltip::new("Italic")
                                        .action(&ToggleItalic, Some("RichText"))
                                        .build(window, cx)
                                })
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_italic_mark(window, cx);
                                        });
                                    },
                                )),
                            )
                            .child(
                                toolbar_button(
                                    "underline",
                                    "U",
                                    self.editor.read(cx).underline_mark_active(),
                                )
                                .tooltip(|window, cx| {
                                    Tooltip::new("Underline")
                                        .action(&ToggleUnderline, Some("RichText"))
                                        .build(window, cx)
                                })
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_underline_mark(window, cx);
                                        });
                                    },
                                )),
                            )
                            .child(
                                toolbar_button(
                                    "strike",
                                    "S",
                                    self.editor.read(cx).strikethrough_mark_active(),
                                )
                                .tooltip(|window, cx| {
                                    Tooltip::new("Strikethrough")
                                        .action(&ToggleStrikethrough, Some("RichText"))
                                        .build(window, cx)
                                })
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_strikethrough_mark(window, cx);
                                        });
                                    },
                                )),
                            )
                            .child(toolbar_sep())
                            .child(
                                toolbar_button(
                                    "link",
                                    "Link",
                                    self.editor.read(cx).current_link_url().is_some(),
                                )
                                .tooltip(|window, cx| {
                                    Tooltip::new("Set or edit link URL").build(window, cx)
                                })
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, window, cx| {
                                        let current = this.editor.read(cx).current_link_url();
                                        let initial = current.unwrap_or_default();
                                        this.open_link_dialog(initial, window, cx);
                                    }),
                                ),
                            )
                            .child(
                                toolbar_button("unlink", "Unlink", false)
                                    .tooltip(|window, cx| {
                                        Tooltip::new("Remove link").build(window, cx)
                                    })
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor
                                                .set_link_at_cursor_or_selection(None, window, cx);
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(toolbar_sep())
                            .child(
                                Button::new("h1")
                                    .ghost()
                                    .label("H1")
                                    .tooltip("Heading 1")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_kind(
                                                BlockKind::Heading { level: 1 },
                                                window,
                                                cx,
                                            );
                                        });
                                    })),
                            )
                            .child(
                                Button::new("h2")
                                    .ghost()
                                    .label("H2")
                                    .tooltip("Heading 2")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_kind(
                                                BlockKind::Heading { level: 2 },
                                                window,
                                                cx,
                                            );
                                        });
                                    })),
                            )
                            .child(
                                Button::new("h3")
                                    .ghost()
                                    .label("H3")
                                    .tooltip("Heading 3")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_kind(
                                                BlockKind::Heading { level: 3 },
                                                window,
                                                cx,
                                            );
                                        });
                                    })),
                            )
                            .child(
                                Button::new("h4")
                                    .ghost()
                                    .label("H4")
                                    .tooltip("Heading 4")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_kind(
                                                BlockKind::Heading { level: 4 },
                                                window,
                                                cx,
                                            );
                                        });
                                    })),
                            )
                            .child(
                                Button::new("h5")
                                    .ghost()
                                    .label("H5")
                                    .tooltip("Heading 5")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_kind(
                                                BlockKind::Heading { level: 5 },
                                                window,
                                                cx,
                                            );
                                        });
                                    })),
                            )
                            .child(
                                Button::new("h6")
                                    .ghost()
                                    .label("H6")
                                    .tooltip("Heading 6")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_kind(
                                                BlockKind::Heading { level: 6 },
                                                window,
                                                cx,
                                            );
                                        });
                                    })),
                            )
                            .child(
                                Button::new("p")
                                    .ghost()
                                    .label("P")
                                    .tooltip("Paragraph")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_kind(BlockKind::Paragraph, window, cx);
                                        });
                                    })),
                            )
                            .child(div().w(px(12.)))
                            .child(
                                Button::new("size-sm")
                                    .ghost()
                                    .label("A-")
                                    .tooltip("Text size: Small")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_size(BlockTextSize::Small, window, cx);
                                        });
                                    })),
                            )
                            .child(
                                Button::new("size-md")
                                    .ghost()
                                    .label("A")
                                    .tooltip("Text size: Normal")
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
                                Button::new("size-lg")
                                    .ghost()
                                    .label("A+")
                                    .tooltip("Text size: Large")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_size(BlockTextSize::Large, window, cx);
                                        });
                                    })),
                            )
                            .child(div().w(px(12.)))
                            .child(
                                Button::new("align-left")
                                    .ghost()
                                    .label("L")
                                    .tooltip("Align left")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_align(BlockAlign::Left, window, cx);
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(
                                Button::new("align-center")
                                    .ghost()
                                    .label("C")
                                    .tooltip("Align center")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_align(BlockAlign::Center, window, cx);
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(
                                Button::new("align-right")
                                    .ghost()
                                    .label("R")
                                    .tooltip("Align right")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_align(BlockAlign::Right, window, cx);
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(div().w(px(12.)))
                            .child(
                                Button::new("ul")
                                    .ghost()
                                    .label("•")
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
                                Button::new("ol-dec")
                                    .ghost()
                                    .label("1.")
                                    .tooltip("Numbered list: Decimal")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_ordered_list(
                                                OrderedListStyle::Decimal,
                                                window,
                                                cx,
                                            );
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(
                                Button::new("ol-la")
                                    .ghost()
                                    .label("a.")
                                    .tooltip("Numbered list: Lower alpha")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_ordered_list(
                                                OrderedListStyle::LowerAlpha,
                                                window,
                                                cx,
                                            );
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(
                                Button::new("ol-ua")
                                    .ghost()
                                    .label("A.")
                                    .tooltip("Numbered list: Upper alpha")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_ordered_list(
                                                OrderedListStyle::UpperAlpha,
                                                window,
                                                cx,
                                            );
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(
                                Button::new("ol-lr")
                                    .ghost()
                                    .label("i.")
                                    .tooltip("Numbered list: Lower roman")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_ordered_list(
                                                OrderedListStyle::LowerRoman,
                                                window,
                                                cx,
                                            );
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(
                                Button::new("ol-ur")
                                    .ghost()
                                    .label("I.")
                                    .tooltip("Numbered list: Upper roman")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_ordered_list(
                                                OrderedListStyle::UpperRoman,
                                                window,
                                                cx,
                                            );
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(
                                Button::new("todo")
                                    .ghost()
                                    .label("☐")
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
                                Button::new("quote")
                                    .ghost()
                                    .label("❝")
                                    .tooltip("Quote")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_kind(BlockKind::Quote, window, cx);
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(
                                Button::new("divider")
                                    .ghost()
                                    .label("—")
                                    .tooltip("Divider")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.insert_divider(window, cx);
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(div().w(px(12.)))
                            .child(
                                Button::new("color-clear")
                                    .ghost()
                                    .label("Tx")
                                    .tooltip("Text color: Clear")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_text_color(None, window, cx);
                                        });
                                    })),
                            )
                            .child(
                                Button::new("color-red")
                                    .ghost()
                                    .label("R")
                                    .tooltip("Text color: Red")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let color = red;
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_text_color(Some(color), window, cx);
                                        });
                                    })),
                            )
                            .child(
                                Button::new("color-green")
                                    .ghost()
                                    .label("G")
                                    .tooltip("Text color: Green")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let color = green;
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_text_color(Some(color), window, cx);
                                        });
                                    })),
                            )
                            .child(
                                Button::new("color-blue")
                                    .ghost()
                                    .label("B")
                                    .tooltip("Text color: Blue")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let color = blue;
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_text_color(Some(color), window, cx);
                                        });
                                    })),
                            )
                            .child(div().w(px(12.)))
                            .child(
                                Button::new("hl-clear")
                                    .ghost()
                                    .label("HL")
                                    .tooltip("Highlight: Clear")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_highlight_color(None, window, cx);
                                        });
                                    })),
                            )
                            .child(
                                Button::new("hl-yellow")
                                    .ghost()
                                    .label("Y")
                                    .tooltip("Highlight: Yellow")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let color = yellow_light;
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_highlight_color(Some(color), window, cx);
                                        });
                                    })),
                            )
                            .child(
                                Button::new("hl-cyan")
                                    .ghost()
                                    .label("C")
                                    .tooltip("Highlight: Cyan")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let color = cyan_light;
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_highlight_color(Some(color), window, cx);
                                        });
                                    })),
                            )
                            .child(
                                Button::new("hl-magenta")
                                    .ghost()
                                    .label("M")
                                    .tooltip("Highlight: Magenta")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let color = magenta_light;
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_highlight_color(Some(color), window, cx);
                                        });
                                    })),
                            ),
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
