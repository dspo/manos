use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::ActiveTheme as _;
use gpui_component::Disableable as _;
use gpui_component::Selectable as _;
use gpui_component::WindowExt as _;
use gpui_component::input::InputState;
use gpui_component::notification::Notification;
use gpui_component_extras::plate_toolbar::{
    PlateIconName, PlateToolbarDropdownButton, PlateToolbarIconButton, PlateToolbarSeparator,
    PlateToolbarSplitButton, PlateToolbarStepper,
};

pub struct PlateToolbarButtonsStory {
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
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
                        PlateToolbarDropdownButton::new("baseline")
                            .tooltip("Text style (dropdown trigger)")
                            .child(PlateIconName::Baseline)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Text style (dropdown trigger)", window, cx);
                            })),
                    )
                    .child(
                        PlateToolbarDropdownButton::new("paint-bucket")
                            .tooltip("Colors (dropdown trigger)")
                            .child(PlateIconName::PaintBucket)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Colors (dropdown trigger)", window, cx);
                            })),
                    ),
            )
            .child(PlateToolbarSeparator)
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        PlateToolbarDropdownButton::new("align")
                            .tooltip("Align (dropdown trigger)")
                            .child(PlateIconName::AlignLeft)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify("Align (dropdown trigger)", window, cx);
                            })),
                    )
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
