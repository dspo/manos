use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::ActiveTheme as _;
use gpui_component::Disableable as _;
use gpui_component::Selectable as _;
use gpui_component::WindowExt as _;
use gpui_component::input::InputState;
use gpui_component::notification::Notification;
use gpui_component::popover::Popover;
use gpui_component_extras::plate_toolbar::{
    PlateIconName, PlateToolbarColorPicker, PlateToolbarDropdownButton, PlateToolbarIconButton,
    PlateToolbarSeparator, PlateToolbarSplitButton, PlateToolbarStepper,
};
use gpui_rich_text::BlockAlign;

pub struct PlateToolbarButtonsStory {
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    align: BlockAlign,
    text_color: Option<Hsla>,
    highlight_color: Option<Hsla>,
    list_collapsed: bool,
    font_size: i32,
    font_size_input: Entity<InputState>,
}

impl PlateToolbarButtonsStory {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let font_size = 16;
        let font_size_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(font_size.to_string(), window, cx);
            state
        });

        Self {
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            align: BlockAlign::Left,
            text_color: None,
            highlight_color: None,
            list_collapsed: true,
            font_size,
            font_size_input,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn set_font_size(&mut self, next: i32, window: &mut Window, cx: &mut Context<Self>) {
        let next = next.clamp(8, 72);
        self.font_size = next;
        self.font_size_input.update(cx, |state, cx| {
            state.set_value(next.to_string(), window, cx);
        });
    }

    fn notify(&self, message: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
        window.push_notification(Notification::new().message(message), cx);
    }
}

impl Render for PlateToolbarButtonsStory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let this = cx.entity();

        let toolbar = div()
            .flex()
            .flex_row()
            .items_center()
            .flex_wrap()
            .gap(px(0.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        PlateToolbarIconButton::new("undo", PlateIconName::Undo2)
                            .tooltip("Undo")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Undo", window, cx);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("redo", PlateIconName::Redo2)
                            .tooltip("Redo (disabled)")
                            .disabled(true),
                    ),
            )
            .child(PlateToolbarSeparator)
            .child(
                div().flex().items_center().child(
                    PlateToolbarIconButton::new("wand", PlateIconName::WandSparkles)
                        .tooltip("Wand sparkles")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.notify("Wand sparkles", window, cx);
                        })),
                ),
            )
            .child(PlateToolbarSeparator)
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        PlateToolbarDropdownButton::new("insert-down")
                            .tooltip("Insert below")
                            .child(PlateIconName::ArrowDownToLine)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Insert below (dropdown trigger)", window, cx);
                            })),
                    )
                    .child(
                        PlateToolbarDropdownButton::new("insert-up")
                            .tooltip("Insert above")
                            .child(PlateIconName::ArrowUpToLine)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Insert above (dropdown trigger)", window, cx);
                            })),
                    ),
            )
            .child(PlateToolbarSeparator)
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        PlateToolbarDropdownButton::new("add")
                            .tooltip("Add (dropdown trigger)")
                            .child(PlateIconName::Plus)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Add (dropdown trigger)", window, cx);
                            })),
                    )
                    .child(
                        PlateToolbarDropdownButton::new("toggle-list")
                            .tooltip("Toggle list (dropdown trigger)")
                            .min_width(px(125.))
                            .child("Toggle list")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Toggle list (dropdown trigger)", window, cx);
                            })),
                    )
                    .child(
                        PlateToolbarStepper::new("font-size", &self.font_size_input)
                            .on_decrement(cx.listener(|this, _, window, cx| {
                                let next = this.font_size.saturating_sub(1);
                                this.set_font_size(next, window, cx);
                            }))
                            .on_increment(cx.listener(|this, _, window, cx| {
                                let next = this.font_size.saturating_add(1);
                                this.set_font_size(next, window, cx);
                            })),
                    ),
            )
            .child(PlateToolbarSeparator)
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        PlateToolbarIconButton::new("bold", PlateIconName::Bold)
                            .tooltip("Bold")
                            .selected(self.bold)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.bold = !this.bold;
                                let handle = cx.focus_handle();
                                handle.focus(window);
                                cx.notify();
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("italic", PlateIconName::Italic)
                            .tooltip("Italic")
                            .selected(self.italic)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.italic = !this.italic;
                                let handle = cx.focus_handle();
                                handle.focus(window);
                                cx.notify();
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("underline", PlateIconName::Underline)
                            .tooltip("Underline")
                            .selected(self.underline)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.underline = !this.underline;
                                let handle = cx.focus_handle();
                                handle.focus(window);
                                cx.notify();
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("strikethrough", PlateIconName::Strikethrough)
                            .tooltip("Strikethrough")
                            .selected(self.strikethrough)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.strikethrough = !this.strikethrough;
                                let handle = cx.focus_handle();
                                handle.focus(window);
                                cx.notify();
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("code-xml", PlateIconName::CodeXml)
                            .tooltip("Code")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Code", window, cx);
                            })),
                    )
                    .child(
                        PlateToolbarColorPicker::new("text-color", PlateIconName::Baseline)
                            .tooltip("Text color")
                            .value(self.text_color)
                            .on_change({
                                let this = this.clone();
                                move |color, window, cx| {
                                    this.update(cx, |this, cx| {
                                        this.text_color = color;
                                        cx.notify();
                                    });
                                    window.push_notification(
                                        Notification::new().message("Text color changed"),
                                        cx,
                                    );
                                }
                            }),
                    )
                    .child(
                        PlateToolbarColorPicker::new("highlight-color", PlateIconName::PaintBucket)
                            .tooltip("Highlight color")
                            .value(self.highlight_color)
                            .on_change({
                                let this = this.clone();
                                move |color, window, cx| {
                                    this.update(cx, |this, cx| {
                                        this.highlight_color = color;
                                        cx.notify();
                                    });
                                    window.push_notification(
                                        Notification::new().message("Highlight color changed"),
                                        cx,
                                    );
                                }
                            }),
                    ),
            )
            .child(PlateToolbarSeparator)
            .child(
                div()
                    .flex()
                    .items_center()
                    .child({
                        let active_align = self.align;
                        let trigger_icon = match active_align {
                            BlockAlign::Left => PlateIconName::AlignLeft,
                            BlockAlign::Center => PlateIconName::AlignCenter,
                            BlockAlign::Right => PlateIconName::AlignRight,
                        };

                        Popover::new("plate-toolbar-align-menu")
                            .appearance(false)
                            .trigger(
                                PlateToolbarDropdownButton::new("plate-toolbar-align-trigger")
                                    .tooltip("Align")
                                    .child(trigger_icon)
                                    .on_click(|_, _, _| {}),
                            )
                            .content(move |_, _window, cx| {
                                let theme = cx.theme();
                                let popover = cx.entity();

                                let story_for_items = this.clone();
                                let make_item =
                                    move |id: &'static str,
                                          icon: PlateIconName,
                                          align: BlockAlign,
                                          disabled: bool| {
                                        let popover = popover.clone();
                                        let story = story_for_items.clone();

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
                                            .when(!disabled, |el| {
                                                el.cursor_pointer()
                                                    .hover(|this| {
                                                        this.bg(theme.accent)
                                                            .text_color(theme.accent_foreground)
                                                    })
                                                    .active(|this| {
                                                        this.bg(theme.accent)
                                                            .text_color(theme.accent_foreground)
                                                    })
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        move |_, window, cx| {
                                                            window.prevent_default();
                                                            story.update(cx, |this, cx| {
                                                                this.align = align;
                                                                cx.notify();
                                                            });
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
                                        "plate-toolbar-align-left",
                                        PlateIconName::AlignLeft,
                                        BlockAlign::Left,
                                        false,
                                    ))
                                    .child(make_item(
                                        "plate-toolbar-align-center",
                                        PlateIconName::AlignCenter,
                                        BlockAlign::Center,
                                        false,
                                    ))
                                    .child(make_item(
                                        "plate-toolbar-align-right",
                                        PlateIconName::AlignRight,
                                        BlockAlign::Right,
                                        false,
                                    ))
                                    .child(make_item(
                                        "plate-toolbar-align-justify",
                                        PlateIconName::AlignJustify,
                                        BlockAlign::Left,
                                        true,
                                    ))
                            })
                    })
                    .child(
                        PlateToolbarSplitButton::new("list-ordered", PlateIconName::ListOrdered)
                            .tooltip("Ordered list")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Ordered list (main)", window, cx);
                            }))
                            .on_click_caret(cx.listener(|this, _, window, cx| {
                                this.notify("Ordered list (dropdown)", window, cx);
                            })),
                    )
                    .child(
                        PlateToolbarSplitButton::new("list", PlateIconName::List)
                            .tooltip("Bulleted list")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Bulleted list (main)", window, cx);
                            }))
                            .on_click_caret(cx.listener(|this, _, window, cx| {
                                this.notify("Bulleted list (dropdown)", window, cx);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("list-todo", PlateIconName::ListTodo)
                            .tooltip("Todo list")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Todo list", window, cx);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("list-collapse", PlateIconName::ListCollapse)
                            .tooltip("Collapse list")
                            .selected(self.list_collapsed)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.list_collapsed = !this.list_collapsed;
                                cx.notify();
                            })),
                    ),
            )
            .child(PlateToolbarSeparator)
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        PlateToolbarIconButton::new("link", PlateIconName::Link)
                            .tooltip("Link")
                            .selected(true),
                    )
                    .child(
                        PlateToolbarDropdownButton::new("table")
                            .tooltip("Table (dropdown trigger)")
                            .child(PlateIconName::Table)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Table (dropdown trigger)", window, cx);
                            })),
                    )
                    .child(
                        PlateToolbarDropdownButton::new("emoji")
                            .tooltip("Emoji (dropdown trigger)")
                            .child(PlateIconName::Smile)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Emoji (dropdown trigger)", window, cx);
                            })),
                    ),
            )
            .child(PlateToolbarSeparator)
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        PlateToolbarSplitButton::new("image", PlateIconName::Image)
                            .tooltip("Insert image")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Insert image (main)", window, cx);
                            }))
                            .on_click_caret(cx.listener(|this, _, window, cx| {
                                this.notify("Insert image (dropdown)", window, cx);
                            })),
                    )
                    .child(
                        PlateToolbarSplitButton::new("film", PlateIconName::Film)
                            .tooltip("Insert video")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Insert video (main)", window, cx);
                            }))
                            .on_click_caret(cx.listener(|this, _, window, cx| {
                                this.notify("Insert video (dropdown)", window, cx);
                            })),
                    )
                    .child(
                        PlateToolbarSplitButton::new("audio", PlateIconName::AudioLines)
                            .tooltip("Insert audio")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Insert audio (main)", window, cx);
                            }))
                            .on_click_caret(cx.listener(|this, _, window, cx| {
                                this.notify("Insert audio (dropdown)", window, cx);
                            })),
                    )
                    .child(
                        PlateToolbarSplitButton::new("file-up", PlateIconName::FileUp)
                            .tooltip("Upload file")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Upload file (main)", window, cx);
                            }))
                            .on_click_caret(cx.listener(|this, _, window, cx| {
                                this.notify("Upload file (dropdown)", window, cx);
                            })),
                    ),
            )
            .child(PlateToolbarSeparator)
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        PlateToolbarDropdownButton::new("wrap-text")
                            .tooltip("Wrap text (dropdown trigger)")
                            .child(PlateIconName::WrapText)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Wrap text (dropdown trigger)", window, cx);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new(
                            "indent-decrease",
                            PlateIconName::IndentDecrease,
                        )
                        .tooltip("Indent decrease")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.notify("Indent decrease", window, cx);
                        })),
                    )
                    .child(
                        PlateToolbarIconButton::new(
                            "indent-increase",
                            PlateIconName::IndentIncrease,
                        )
                        .tooltip("Indent increase")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.notify("Indent increase", window, cx);
                        })),
                    )
                    .child(
                        PlateToolbarIconButton::new("ellipsis", PlateIconName::Ellipsis)
                            .tooltip("More")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("More", window, cx);
                            })),
                    ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        PlateToolbarIconButton::new("highlighter", PlateIconName::Highlighter)
                            .tooltip("Highlight")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Highlight", window, cx);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new(
                            "message-square-text",
                            PlateIconName::MessageSquareText,
                        )
                        .tooltip("Comment")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.notify("Comment", window, cx);
                        })),
                    )
                    .child(PlateToolbarSeparator)
                    .child(
                        PlateToolbarDropdownButton::new("editing")
                            .tooltip("Mode (dropdown trigger)")
                            .child(PlateIconName::Pen)
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .when(window.viewport_size().width < px(640.), |this| {
                                        this.hidden()
                                    })
                                    .child("Editing"),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Mode (dropdown trigger)", window, cx);
                            })),
                    ),
            );

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.muted)
            .p(px(16.))
            .gap(px(12.))
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Plate.js Toolbar Buttons (GPUI)"),
            )
            .child(
                div()
                    .rounded(theme.radius)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.background)
                    .p(px(8.))
                    .child(toolbar),
            )
    }
}
