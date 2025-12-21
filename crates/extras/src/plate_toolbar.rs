use std::rc::Rc;

use gpui::InteractiveElement as _;
use gpui::StatefulInteractiveElement as _;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, ClickEvent, ElementId, Hsla, IntoElement, MouseButton, ParentElement, RenderOnce,
    SharedString, StyleRefinement, Styled, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use gpui_component::Disableable;
use gpui_component::Selectable;
use gpui_component::StyledExt as _;
use gpui_component::input::{Input, InputState};
use gpui_component::tooltip::Tooltip;
use gpui_component::{Icon, IconNamed};

#[derive(IntoElement, Clone, Copy, Debug)]
pub enum PlateIconName {
    AlignLeft,
    ArrowDownToLine,
    ArrowUpToLine,
    AudioLines,
    Baseline,
    Bold,
    ChevronDown,
    CodeXml,
    Ellipsis,
    FileUp,
    Film,
    Highlighter,
    Image,
    IndentDecrease,
    IndentIncrease,
    Italic,
    Link,
    Unlink,
    List,
    ListCollapse,
    ListOrdered,
    ListTodo,
    MessageSquareText,
    Minus,
    PaintBucket,
    Pen,
    Plus,
    Redo2,
    Smile,
    Strikethrough,
    Table,
    Underline,
    Undo2,
    WandSparkles,
    WrapText,
}

impl IconNamed for PlateIconName {
    fn path(self) -> SharedString {
        match self {
            Self::AlignLeft => "icons/align-left.svg",
            Self::ArrowDownToLine => "icons/arrow-down-to-line.svg",
            Self::ArrowUpToLine => "icons/arrow-up-to-line.svg",
            Self::AudioLines => "icons/audio-lines.svg",
            Self::Baseline => "icons/baseline.svg",
            Self::Bold => "icons/bold.svg",
            Self::ChevronDown => "icons/chevron-down.svg",
            Self::CodeXml => "icons/code-xml.svg",
            Self::Ellipsis => "icons/ellipsis.svg",
            Self::FileUp => "icons/file-up.svg",
            Self::Film => "icons/film.svg",
            Self::Highlighter => "icons/highlighter.svg",
            Self::Image => "icons/image.svg",
            Self::IndentDecrease => "icons/indent-decrease.svg",
            Self::IndentIncrease => "icons/indent-increase.svg",
            Self::Italic => "icons/italic.svg",
            Self::Link => "icons/link.svg",
            Self::Unlink => "icons/unlink.svg",
            Self::List => "icons/list.svg",
            Self::ListCollapse => "icons/list-collapse.svg",
            Self::ListOrdered => "icons/list-ordered.svg",
            Self::ListTodo => "icons/list-todo.svg",
            Self::MessageSquareText => "icons/message-square-text.svg",
            Self::Minus => "icons/minus.svg",
            Self::PaintBucket => "icons/paint-bucket.svg",
            Self::Pen => "icons/pen.svg",
            Self::Plus => "icons/plus.svg",
            Self::Redo2 => "icons/redo-2.svg",
            Self::Smile => "icons/smile.svg",
            Self::Strikethrough => "icons/strikethrough.svg",
            Self::Table => "icons/table.svg",
            Self::Underline => "icons/underline.svg",
            Self::Undo2 => "icons/undo-2.svg",
            Self::WandSparkles => "icons/wand-sparkles.svg",
            Self::WrapText => "icons/wrap-text.svg",
        }
        .into()
    }
}

impl RenderOnce for PlateIconName {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        Icon::new(self)
    }
}

#[derive(IntoElement)]
pub struct PlateToolbarButton {
    id: ElementId,
    style: StyleRefinement,
    tooltip: Option<SharedString>,
    disabled: bool,
    selected: bool,
    default_text_color: Option<Hsla>,
    children: Vec<gpui::AnyElement>,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    rounding: PlateToolbarRounding,
    justify: PlateToolbarJustify,
    min_width: gpui::Pixels,
    padding_x: gpui::Pixels,
    height: gpui::Pixels,
}

#[derive(Clone, Copy)]
pub enum PlateToolbarJustify {
    Center,
    Between,
}

#[derive(Clone, Copy)]
pub enum PlateToolbarRounding {
    All,
    Left,
    Right,
    None,
}

impl PlateToolbarButton {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            tooltip: None,
            disabled: false,
            selected: false,
            default_text_color: None,
            children: Vec::new(),
            on_click: None,
            rounding: PlateToolbarRounding::All,
            justify: PlateToolbarJustify::Center,
            min_width: px(32.),
            padding_x: px(6.),
            height: px(32.),
        }
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn on_click(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    pub fn justify_between(mut self) -> Self {
        self.justify = PlateToolbarJustify::Between;
        self
    }

    pub fn rounding(mut self, rounding: PlateToolbarRounding) -> Self {
        self.rounding = rounding;
        self
    }

    pub fn min_width(mut self, min_width: gpui::Pixels) -> Self {
        self.min_width = min_width;
        self
    }

    pub fn height(mut self, height: gpui::Pixels) -> Self {
        self.height = height;
        self
    }

    pub fn padding_x(mut self, padding_x: gpui::Pixels) -> Self {
        self.padding_x = padding_x;
        self
    }

    pub fn default_text_color(mut self, color: Hsla) -> Self {
        self.default_text_color = Some(color);
        self
    }
}

impl ParentElement for PlateToolbarButton {
    fn extend(&mut self, elements: impl IntoIterator<Item = gpui::AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for PlateToolbarButton {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Selectable for PlateToolbarButton {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl Disableable for PlateToolbarButton {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for PlateToolbarButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let disabled = self.disabled;
        let clickable = self.on_click.is_some() && !disabled;

        let mut disabled_fg = theme.muted_foreground;
        disabled_fg.a *= 0.6;
        let default_fg = self.default_text_color.unwrap_or(theme.foreground);

        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap(px(8.))
            .h(self.height)
            .min_w(self.min_width)
            .px(self.padding_x)
            .text_size(px(12.))
            .font_weight(gpui::FontWeight::MEDIUM)
            .bg(theme.transparent)
            .text_color(if disabled { disabled_fg } else { default_fg })
            .when(
                matches!(self.justify, PlateToolbarJustify::Center),
                |this| this.justify_center(),
            )
            .when(
                matches!(self.justify, PlateToolbarJustify::Between),
                |this| this.justify_between(),
            )
            .when(matches!(self.rounding, PlateToolbarRounding::All), |this| {
                this.rounded(px(6.))
            })
            .when(
                matches!(self.rounding, PlateToolbarRounding::Left),
                |this| {
                    this.rounded_tl(px(6.))
                        .rounded_bl(px(6.))
                        .rounded_tr(px(0.))
                        .rounded_br(px(0.))
                },
            )
            .when(
                matches!(self.rounding, PlateToolbarRounding::Right),
                |this| {
                    this.rounded_tr(px(6.))
                        .rounded_br(px(6.))
                        .rounded_tl(px(0.))
                        .rounded_bl(px(0.))
                },
            )
            .when(
                matches!(self.rounding, PlateToolbarRounding::None),
                |this| this.rounded(px(0.)),
            )
            .when(!disabled, |this| {
                this.cursor_pointer()
                    .hover(|this| this.bg(theme.muted).text_color(theme.muted_foreground))
                    .active(|this| this.bg(theme.accent).text_color(theme.accent_foreground))
            })
            .when(self.selected, |this| {
                this.bg(theme.accent).text_color(theme.accent_foreground)
            })
            .refine_style(&self.style)
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                if !clickable {
                    cx.stop_propagation();
                    return;
                }

                // Avoid focusing the toolbar button and stealing focus from an editor.
                window.prevent_default();
            })
            .when_some(self.on_click, |this, on_click| {
                this.on_click(move |event, window, cx| {
                    if !clickable {
                        cx.stop_propagation();
                        return;
                    }
                    (on_click)(event, window, cx);
                })
            })
            .children(self.children)
            .when_some(self.tooltip, |this, tooltip| {
                this.tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
            })
    }
}

#[derive(IntoElement)]
pub struct PlateToolbarIconButton {
    base: PlateToolbarButton,
}

impl PlateToolbarIconButton {
    pub fn new(id: impl Into<ElementId>, icon: impl IconNamed) -> Self {
        let icon = Icon::new(icon);
        Self {
            base: PlateToolbarButton::new(id).child(icon),
        }
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.base = self.base.tooltip(tooltip);
        self
    }

    pub fn on_click(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.base = self.base.on_click(on_click);
        self
    }
}

impl Styled for PlateToolbarIconButton {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl Selectable for PlateToolbarIconButton {
    fn selected(mut self, selected: bool) -> Self {
        self.base = self.base.selected(selected);
        self
    }

    fn is_selected(&self) -> bool {
        self.base.is_selected()
    }
}

impl Disableable for PlateToolbarIconButton {
    fn disabled(mut self, disabled: bool) -> Self {
        self.base = self.base.disabled(disabled);
        self
    }
}

impl RenderOnce for PlateToolbarIconButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.base.render(window, cx)
    }
}

#[derive(IntoElement)]
pub struct PlateToolbarDropdownButton {
    id: ElementId,
    style: StyleRefinement,
    tooltip: Option<SharedString>,
    disabled: bool,
    selected: bool,
    left: Vec<gpui::AnyElement>,
    min_width: gpui::Pixels,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

impl PlateToolbarDropdownButton {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            tooltip: None,
            disabled: false,
            selected: false,
            left: Vec::new(),
            min_width: px(32.),
            on_click: None,
        }
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn min_width(mut self, min_width: gpui::Pixels) -> Self {
        self.min_width = min_width;
        self
    }

    pub fn on_click(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }
}

impl ParentElement for PlateToolbarDropdownButton {
    fn extend(&mut self, elements: impl IntoIterator<Item = gpui::AnyElement>) {
        self.left.extend(elements);
    }
}

impl Styled for PlateToolbarDropdownButton {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Selectable for PlateToolbarDropdownButton {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl Disableable for PlateToolbarDropdownButton {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for PlateToolbarDropdownButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        let chevron = Icon::new(PlateIconName::ChevronDown)
            .size_3p5()
            .text_color(theme.muted_foreground);

        let mut button = PlateToolbarButton::new(self.id)
            .justify_between()
            .min_width(self.min_width)
            .padding_x(px(6.))
            .disabled(self.disabled)
            .selected(self.selected)
            .refine_style(&self.style)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .children(self.left),
            )
            .child(chevron);

        if let Some(tooltip) = self.tooltip {
            button = button.tooltip(tooltip);
        }
        if let Some(on_click) = self.on_click {
            button = button.on_click(move |event, window, cx| on_click(event, window, cx));
        }

        button.render(window, cx)
    }
}

#[derive(IntoElement)]
pub struct PlateToolbarSeparator;

impl RenderOnce for PlateToolbarSeparator {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .mx(px(6.))
            .py(px(2.))
            .child(div().h(px(18.)).w(px(1.)).bg(theme.border))
    }
}

#[derive(IntoElement)]
pub struct PlateToolbarSplitButton {
    id: ElementId,
    style: StyleRefinement,
    tooltip: Option<SharedString>,
    disabled: bool,
    selected: bool,
    icon: Icon,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    on_click_caret: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

impl PlateToolbarSplitButton {
    pub fn new(id: impl Into<ElementId>, icon: impl IconNamed) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            tooltip: None,
            disabled: false,
            selected: false,
            icon: Icon::new(icon),
            on_click: None,
            on_click_caret: None,
        }
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn on_click(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    pub fn on_click_caret(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click_caret = Some(Rc::new(on_click));
        self
    }
}

impl Styled for PlateToolbarSplitButton {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Selectable for PlateToolbarSplitButton {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl Disableable for PlateToolbarSplitButton {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for PlateToolbarSplitButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        let main_id = ElementId::NamedChild(Box::new(self.id.clone()), "main".into());
        let caret_id = ElementId::NamedChild(Box::new(self.id.clone()), "caret".into());

        let mut main = PlateToolbarButton::new(main_id)
            .rounding(PlateToolbarRounding::Left)
            .disabled(self.disabled)
            .selected(self.selected)
            .child(self.icon);
        if let Some(tooltip) = self.tooltip {
            main = main.tooltip(tooltip);
        }
        if let Some(on_click) = self.on_click {
            main = main.on_click(move |event, window, cx| on_click(event, window, cx));
        }

        let mut caret = PlateToolbarButton::new(caret_id)
            .rounding(PlateToolbarRounding::Right)
            .min_width(px(16.))
            .padding_x(px(0.))
            .disabled(self.disabled)
            .selected(self.selected)
            .default_text_color(theme.muted_foreground)
            .child(Icon::new(PlateIconName::ChevronDown).size_3p5());
        if let Some(on_click) = self.on_click_caret {
            caret = caret.on_click(move |event, window, cx| on_click(event, window, cx));
        }

        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap(px(0.))
            .refine_style(&self.style)
            .child(main)
            .child(caret)
    }
}

#[derive(IntoElement)]
pub struct PlateToolbarStepper {
    id: ElementId,
    style: StyleRefinement,
    disabled: bool,
    input: gpui::Entity<InputState>,
    on_decrement: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    on_increment: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    value_width: gpui::Pixels,
}

impl PlateToolbarStepper {
    pub fn new(id: impl Into<ElementId>, input: &gpui::Entity<InputState>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            disabled: false,
            input: input.clone(),
            on_decrement: None,
            on_increment: None,
            value_width: px(40.),
        }
    }

    pub fn value_width(mut self, width: gpui::Pixels) -> Self {
        self.value_width = width;
        self
    }

    pub fn on_decrement(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_decrement = Some(Rc::new(on_click));
        self
    }

    pub fn on_increment(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_increment = Some(Rc::new(on_click));
        self
    }
}

impl Styled for PlateToolbarStepper {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Disableable for PlateToolbarStepper {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for PlateToolbarStepper {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        let mut bg = theme.muted;
        bg.a *= 0.6;

        let minus_id = ElementId::NamedChild(Box::new(self.id.clone()), "minus".into());
        let plus_id = ElementId::NamedChild(Box::new(self.id.clone()), "plus".into());

        let mut minus = PlateToolbarButton::new(minus_id)
            .disabled(self.disabled)
            .child(Icon::new(PlateIconName::Minus));
        if let Some(on_click) = self.on_decrement {
            minus = minus.on_click(move |event, window, cx| on_click(event, window, cx));
        }

        let mut plus = PlateToolbarButton::new(plus_id)
            .disabled(self.disabled)
            .child(Icon::new(PlateIconName::Plus));
        if let Some(on_click) = self.on_increment {
            plus = plus.on_click(move |event, window, cx| on_click(event, window, cx));
        }

        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap(px(4.))
            .rounded(px(6.))
            .bg(bg)
            .refine_style(&self.style)
            .child(minus)
            .child(
                div()
                    .h(px(32.))
                    .w(self.value_width)
                    .rounded(px(4.))
                    .hover(|this| this.bg(theme.muted))
                    .child(
                        Input::new(&self.input)
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .w_full()
                            .h(px(32.))
                            .text_center()
                            .text_size(px(12.)),
                    ),
            )
            .child(plus)
    }
}
