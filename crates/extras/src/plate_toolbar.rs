use std::rc::Rc;

use gpui::InteractiveElement as _;
use gpui::StatefulInteractiveElement as _;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, ClickEvent, ElementId, Hsla, IntoElement, MouseButton, ParentElement,
    RenderOnce, SharedString, StyleRefinement, Styled, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use gpui_component::Colorize as _;
use gpui_component::Disableable;
use gpui_component::Selectable;
use gpui_component::Sizable as _;
use gpui_component::StyledExt as _;
use gpui_component::input::{Input, InputState};
use gpui_component::popover::Popover;
use gpui_component::tooltip::Tooltip;
use gpui_component::{Icon, IconNamed};

#[derive(IntoElement, Clone, Copy, Debug)]
pub enum PlateIconName {
    AlignCenter,
    AlignJustify,
    AlignLeft,
    AlignRight,
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
            Self::AlignCenter => "icons/align-center.svg",
            Self::AlignJustify => "icons/align-justify.svg",
            Self::AlignLeft => "icons/align-left.svg",
            Self::AlignRight => "icons/align-right.svg",
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

fn default_toolbar_color_swatches() -> Vec<Hsla> {
    let grayscale = [0.0, 0.13, 0.26, 0.4, 0.55, 0.67, 0.78, 0.87, 0.93, 1.0];
    let hues = [
        0.0,   // red
        30.0,  // orange
        55.0,  // yellow
        120.0, // green
        180.0, // cyan
        210.0, // blue
        270.0, // purple
        320.0, // pink
    ];
    let lightness = [0.35, 0.45, 0.55, 0.65, 0.75];

    let mut colors = Vec::new();
    for l in grayscale {
        colors.push(gpui::hsla(0.0, 0.0, l, 1.0));
    }
    for hue in hues {
        for l in lightness {
            colors.push(gpui::hsla(hue / 360.0, 0.85, l, 1.0));
        }
    }
    colors
}

#[derive(Default)]
struct PlateToolbarColorPickerState {
    input: Option<gpui::Entity<InputState>>,
}

#[derive(IntoElement)]
pub struct PlateToolbarColorPicker {
    id: ElementId,
    style: StyleRefinement,
    tooltip: Option<SharedString>,
    disabled: bool,
    value: Option<Hsla>,
    icon: Icon,
    swatches: Option<Rc<Vec<Hsla>>>,
    on_change: Option<Rc<dyn Fn(Option<Hsla>, &mut Window, &mut App)>>,
}

impl PlateToolbarColorPicker {
    pub fn new(id: impl Into<ElementId>, icon: impl IconNamed) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            tooltip: None,
            disabled: false,
            value: None,
            icon: Icon::new(icon),
            swatches: None,
            on_change: None,
        }
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn value(mut self, value: Option<Hsla>) -> Self {
        self.value = value;
        self
    }

    pub fn swatches(mut self, swatches: Vec<Hsla>) -> Self {
        self.swatches = Some(Rc::new(swatches));
        self
    }

    pub fn on_change(
        mut self,
        on_change: impl Fn(Option<Hsla>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(on_change));
        self
    }
}

impl Styled for PlateToolbarColorPicker {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Disableable for PlateToolbarColorPicker {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for PlateToolbarColorPicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let value = self.value;
        let selected_hex = value.map(|color| color.to_hex());

        let input_state_id = ElementId::NamedChild(Box::new(self.id.clone()), "input-state".into());
        let input_state = window.use_keyed_state(input_state_id, cx, |window, cx| {
            PlateToolbarColorPickerState {
                input: Some(cx.new(|cx| {
                    let mut state = InputState::new(window, cx);
                    state.set_placeholder("#RRGGBB", window, cx);
                    state
                })),
            }
        });
        let input = input_state
            .read(cx)
            .input
            .as_ref()
            .expect("input state should exist")
            .clone();

        let trigger_id = ElementId::NamedChild(Box::new(self.id.clone()), "trigger".into());
        let popover_id = ElementId::NamedChild(Box::new(self.id.clone()), "popover".into());

        let mut indicator = theme.border;
        indicator.a *= 0.8;
        let indicator = value.unwrap_or(indicator);

        let mut trigger = PlateToolbarButton::new(trigger_id)
            .disabled(self.disabled)
            .relative()
            .child(self.icon)
            .child(
                div()
                    .absolute()
                    .left(px(6.))
                    .right(px(6.))
                    .bottom(px(6.))
                    .h(px(2.))
                    .rounded(px(999.))
                    .bg(indicator),
            )
            // The popover toggles on the wrapper; use a no-op click to keep toolbar behavior.
            .on_click(|_, _, _| {});

        if let Some(tooltip) = self.tooltip {
            trigger = trigger.tooltip(tooltip);
        }
        trigger = trigger.refine_style(&self.style);

        let swatches = self
            .swatches
            .unwrap_or_else(|| Rc::new(default_toolbar_color_swatches()));
        let on_change = self.on_change;
        let disabled = self.disabled;

        Popover::new(popover_id)
            .appearance(false)
            .trigger(trigger)
            .content(move |_, _window, cx| {
                let theme = cx.theme();
                let popover = cx.entity();
                let popover_entity_id = popover.entity_id();

                let popover_for_swatches = popover.clone();
                let input_for_swatches = input.clone();
                let on_change_for_swatches = on_change.clone();
                let selected_hex_for_swatches = selected_hex.clone();

                let make_swatch =
                    move |id: SharedString, color: Option<Hsla>| -> gpui::AnyElement {
                        let popover = popover_for_swatches.clone();
                        let input = input_for_swatches.clone();
                        let on_change = on_change_for_swatches.clone();
                        let selected_hex = selected_hex_for_swatches.clone();

                        let (bg, label) = match color {
                            Some(color) => (color, None),
                            None => (theme.background, Some("×")),
                        };

                        let mut swatch = div()
                            .id(id)
                            .flex()
                            .items_center()
                            .justify_center()
                            .h(px(24.))
                            .w(px(24.))
                            .rounded(px(999.))
                            .bg(bg)
                            .border_1()
                            .border_color(theme.border)
                            .when_some(label, |this, label| {
                                this.text_size(px(12.))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(theme.muted_foreground)
                                    .child(label)
                            });

                        if let (Some(selected_hex), Some(color)) = (&selected_hex, color) {
                            if *selected_hex == color.to_hex() {
                                swatch = swatch.border_2().border_color(theme.ring);
                            }
                        }

                        if disabled {
                            return swatch.opacity(0.5).cursor_not_allowed().into_any_element();
                        }

                        swatch
                            .cursor_pointer()
                            .hover(|this| this.shadow_md())
                            .active(|this| this.shadow_md())
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                window.prevent_default();

                                if let Some(on_change) = on_change.as_ref() {
                                    (on_change)(color, window, cx);
                                }

                                input.update(cx, |state, cx| {
                                    let next = color.map(|c| c.to_hex()).unwrap_or_default();
                                    state.set_value(next, window, cx);
                                });

                                popover.update(cx, |state, cx| state.dismiss(window, cx));
                            })
                            .into_any_element()
                    };

                let apply_id = SharedString::from(format!("{:?}:apply", popover_entity_id));
                let popover_for_apply = popover.clone();
                let input_for_apply = input.clone();
                let on_change_for_apply = on_change.clone();
                let apply_button = div()
                    .id(apply_id)
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(32.))
                    .w(px(32.))
                    .rounded(px(6.))
                    .bg(theme.transparent)
                    .text_color(theme.popover_foreground)
                    .cursor_pointer()
                    .hover(|this| this.bg(theme.accent).text_color(theme.accent_foreground))
                    .active(|this| this.bg(theme.accent).text_color(theme.accent_foreground))
                    .child(Icon::new(PlateIconName::Plus))
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        window.prevent_default();
                        let val = input_for_apply.read(cx).value().to_string();
                        let Ok(color) = Hsla::parse_hex(&val) else {
                            return;
                        };

                        if let Some(on_change) = on_change_for_apply.as_ref() {
                            (on_change)(Some(color), window, cx);
                        }

                        input_for_apply.update(cx, |state, cx| {
                            state.set_value(color.to_hex(), window, cx);
                        });

                        popover_for_apply.update(cx, |state, cx| state.dismiss(window, cx));
                    });

                let swatch_size_px: f32 = 24.;
                let swatch_gap_px: f32 = 4.;
                let total_swatches = swatches.len().saturating_add(1).max(1);
                let columns = (total_swatches as f32).sqrt().ceil() as usize;
                let columns = columns.clamp(1, 10);
                let swatch_grid_width = px(
                    columns as f32 * swatch_size_px
                        + (columns.saturating_sub(1) as f32) * swatch_gap_px,
                );

                div()
                    .p(px(4.))
                    .bg(theme.popover)
                    .border_1()
                    .border_color(theme.border)
                    .rounded(theme.radius)
                    .shadow_md()
                    .text_color(theme.popover_foreground)
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .w(swatch_grid_width)
                            .gap(px(swatch_gap_px))
                            .child(make_swatch(
                                SharedString::from(format!("{:?}:clear", popover_entity_id)),
                                None,
                            ))
                            .children(swatches.iter().copied().enumerate().map(|(ix, color)| {
                                make_swatch(
                                    SharedString::from(format!(
                                        "{:?}:swatch:{}",
                                        popover_entity_id, ix
                                    )),
                                    Some(color),
                                )
                            })),
                    )
                    .child(div().h(px(1.)).bg(theme.border).my(px(4.)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .px(px(4.))
                            .child(Input::new(&input).small().w(px(120.)))
                            .child(apply_button),
                    )
            })
    }
}
