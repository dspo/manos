use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::ActiveTheme as _;
use gpui_component::Root;
use gpui_component::Selectable as _;
use gpui_component::WindowExt as _;
use gpui_component::input::{Input, InputState};
use gpui_component::notification::Notification;
use gpui_component::popover::Popover;
use gpui_component_extras::plate_toolbar::{
    PlateIconName, PlateToolbarButton, PlateToolbarColorPicker, PlateToolbarDropdownButton,
    PlateToolbarIconButton, PlateToolbarSeparator,
};
use gpui_rich_text::{
    BlockAlign, BlockFormat, BlockKind, BlockNode, BlockTextSize, InlineNode, InlineStyle,
    OrderedListStyle, RichTextDocument, RichTextEditor, RichTextState, RichTextTheme,
    RichTextValue, SlateNode, TextNode,
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
                            .flex_wrap()
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
                                PlateToolbarButton::new("open")
                                    .tooltip("Open from JSON file")
                                    .child("Open")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_from_file(window, cx);
                                    })),
                            )
                            .child(
                                PlateToolbarButton::new("save")
                                    .tooltip("Save to JSON file")
                                    .child("Save")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_to_file(window, cx);
                                    })),
                            )
                            .child(
                                PlateToolbarButton::new("save-as")
                                    .tooltip("Save to a new JSON file")
                                    .child("Save As")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_as(window, cx);
                                    })),
                            )
                            .child(PlateToolbarSeparator)
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
                            .child(
                                PlateToolbarButton::new("h1")
                                    .tooltip("Heading 1")
                                    .child("H1")
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
                                PlateToolbarButton::new("h2")
                                    .tooltip("Heading 2")
                                    .child("H2")
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
                                PlateToolbarButton::new("h3")
                                    .tooltip("Heading 3")
                                    .child("H3")
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
                                PlateToolbarButton::new("h4")
                                    .tooltip("Heading 4")
                                    .child("H4")
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
                                PlateToolbarButton::new("h5")
                                    .tooltip("Heading 5")
                                    .child("H5")
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
                                PlateToolbarButton::new("h6")
                                    .tooltip("Heading 6")
                                    .child("H6")
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
                                PlateToolbarButton::new("p")
                                    .tooltip("Paragraph")
                                    .child("P")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block_kind(BlockKind::Paragraph, window, cx);
                                        });
                                    })),
                            )
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
                                PlateToolbarIconButton::new("ol-dec", PlateIconName::ListOrdered)
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
                                PlateToolbarButton::new("ol-la")
                                    .tooltip("Numbered list: Lower alpha")
                                    .child("a.")
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
                                PlateToolbarButton::new("ol-ua")
                                    .tooltip("Numbered list: Upper alpha")
                                    .child("A.")
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
                                PlateToolbarButton::new("ol-lr")
                                    .tooltip("Numbered list: Lower roman")
                                    .child("i.")
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
                                PlateToolbarButton::new("ol-ur")
                                    .tooltip("Numbered list: Upper roman")
                                    .child("I.")
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
