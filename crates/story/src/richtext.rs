use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::ActiveTheme as _;
use gpui_rich_text::{
    BlockKind, BlockTextSize, RichTextEditor, RichTextState, RichTextTheme, ToggleBold,
    ToggleItalic, ToggleStrikethrough, ToggleUnderline,
};

const DEFAULT_TEXT: &str = "GPUI Rich Text Editor\n\n\
This is a pure-GPUI WYSIWYG rich text editor.\n\
Select text, then apply formatting from the toolbar.\n\n\
Try:\n\
bold / italic / underline / strikethrough\n\
headings and paragraph sizes\n\
text color and highlight\n\
unordered and ordered lists\n";

pub struct RichTextExample {
    editor: Entity<RichTextState>,
}

impl RichTextExample {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let theme = cx.theme();
        let rich_text_theme = RichTextTheme {
            background: theme.background,
            border: theme.border,
            radius: theme.radius,
            foreground: theme.foreground,
            muted_foreground: theme.muted_foreground,
            selection: theme.selection,
        };

        let editor = cx.new(|cx| {
            RichTextState::new(window, cx)
                .theme(rich_text_theme)
                .default_value(DEFAULT_TEXT)
        });
        Self { editor }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
}

impl Render for RichTextExample {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let red = theme.red;
        let green = theme.green;
        let blue = theme.blue;
        let yellow_light = theme.yellow_light;
        let cyan_light = theme.cyan_light;
        let magenta_light = theme.magenta_light;

        div()
            .size_full()
            .bg(theme.muted)
            .child(
                div()
                    .w_full()
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
                                Button::new("bold")
                                    .ghost()
                                    .label("B")
                                    .tooltip_with_action("Bold", &ToggleBold, Some("RichText"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_bold_mark(window, cx);
                                        });
                                    })),
                            )
                            .child(
                                Button::new("italic")
                                    .ghost()
                                    .label("I")
                                    .tooltip_with_action("Italic", &ToggleItalic, Some("RichText"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_italic_mark(window, cx);
                                        });
                                    })),
                            )
                            .child(
                                Button::new("underline")
                                    .ghost()
                                    .label("U")
                                    .tooltip_with_action(
                                        "Underline",
                                        &ToggleUnderline,
                                        Some("RichText"),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_underline_mark(window, cx);
                                        });
                                    })),
                            )
                            .child(
                                Button::new("strike")
                                    .ghost()
                                    .label("S")
                                    .tooltip_with_action(
                                        "Strikethrough",
                                        &ToggleStrikethrough,
                                        Some("RichText"),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_strikethrough_mark(window, cx);
                                        });
                                    })),
                            )
                            .child(div().w(px(12.)))
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
                                    })),
                            )
                            .child(
                                Button::new("ol")
                                    .ghost()
                                    .label("1.")
                                    .tooltip("Numbered list")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| {
                                            editor.toggle_list(
                                                BlockKind::OrderedListItem,
                                                window,
                                                cx,
                                            );
                                        });
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
                div().flex_1().p(px(16.)).child(
                    div().flex().flex_row().justify_center().size_full().child(
                        div()
                            .w_full()
                            .max_w(px(960.))
                            .h_full()
                            .bg(theme.background)
                            .border_1()
                            .border_color(theme.border)
                            .rounded(px(12.))
                            .child(
                                RichTextEditor::new(&self.editor)
                                    .bordered(false)
                                    .padding(px(24.))
                                    .size_full(),
                            ),
                    ),
                ),
            )
    }
}
