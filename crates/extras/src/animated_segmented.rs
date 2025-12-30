use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Element, ElementId, GlobalElementId, InspectorElementId,
    InteractiveElement as _, IntoElement, LayoutId, MouseButton, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div, px,
    relative,
};
use gpui_component::tooltip::Tooltip;
use gpui_component::{ActiveTheme as _, Sizable, Size, StyledExt as _};

#[derive(Default)]
pub struct AnimatedSegmentedItem {
    icon: Option<AnyElement>,
    label: Option<SharedString>,
    tooltip: Option<SharedString>,
    disabled: bool,
}

impl AnimatedSegmentedItem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn icon(mut self, icon: impl IntoElement) -> Self {
        self.icon = Some(icon.into_any_element());
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(IntoElement)]
pub struct AnimatedSegmented {
    id: ElementId,
    style: StyleRefinement,
    items: Vec<AnimatedSegmentedItem>,
    selected_index: usize,
    size: Size,
    animation_duration: Duration,
    on_click: Option<Rc<dyn Fn(&usize, &mut Window, &mut App)>>,
}

impl AnimatedSegmented {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            items: Vec::new(),
            selected_index: 0,
            size: Size::Small,
            animation_duration: Duration::from_millis(160),
            on_click: None,
        }
    }

    pub fn items(mut self, items: impl Into<Vec<AnimatedSegmentedItem>>) -> Self {
        self.items = items.into();
        self
    }

    pub fn selected_index(mut self, index: usize) -> Self {
        self.selected_index = index;
        self
    }

    pub fn animation_duration(mut self, duration: Duration) -> Self {
        self.animation_duration = duration;
        self
    }

    pub fn on_click(mut self, on_click: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }
}

impl Styled for AnimatedSegmented {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Sizable for AnimatedSegmented {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for AnimatedSegmented {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let count = self.items.len();
        if count == 0 {
            return div().into_any_element();
        }

        let base_id = self.id.clone();
        let selected = self.selected_index.min(count.saturating_sub(1));

        let height = match self.size {
            Size::XSmall => px(20.),
            Size::Small => px(24.),
            Size::Large => px(36.),
            _ => px(32.),
        };

        let inner_height = match self.size {
            Size::XSmall => px(16.),
            Size::Small => px(20.),
            Size::Large => px(28.),
            _ => px(24.),
        };

        let indicator_id: ElementId = (base_id.clone(), "indicator").into();
        let indicator = AnimatedSegmentedIndicator {
            id: indicator_id,
            selected_index: selected,
            count,
            duration: self.animation_duration,
        };

        let on_click = self.on_click.clone();
        let items = self.items;

        div()
            .id(base_id.clone())
            .h(height)
            .bg(theme.tab_bar_segmented)
            .rounded(theme.radius)
            .p(px(2.))
            .refine_style(&self.style)
            .child(
                div()
                    .relative()
                    .h(inner_height)
                    .w_full()
                    .child(indicator)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .h_full()
                            .w_full()
                            .children(items.into_iter().enumerate().map(|(ix, item)| {
                                let selected = ix == selected;
                                let disabled = item.disabled;
                                let icon = item.icon;
                                let label = item.label;
                                let tooltip = item.tooltip;
                                let segment_id: ElementId =
                                    (base_id.clone(), format!("segment-{}", ix)).into();
                                let tooltip_text = tooltip.clone().or_else(|| label.clone());

                                let content = div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(6.))
                                    .min_w(px(0.))
                                    .when_some(icon, |this, icon| this.child(icon))
                                    .when_some(label, |this, label| {
                                        this.child(div().truncate().child(label))
                                    });

                                let mut segment = div()
                                    .id(segment_id)
                                    .flex()
                                    .flex_1()
                                    .h_full()
                                    .items_center()
                                    .justify_center()
                                    .rounded(theme.radius)
                                    .text_sm()
                                    .child(content);

                                if disabled {
                                    segment =
                                        segment.text_color(theme.muted_foreground).cursor_default();
                                } else if selected {
                                    segment = segment
                                        .text_color(theme.tab_active_foreground)
                                        .cursor_default();
                                } else {
                                    segment = segment
                                        .text_color(theme.tab_foreground)
                                        .cursor_pointer()
                                        .hover(|this| this.bg(theme.secondary_hover))
                                        .active(|this| this.bg(theme.secondary))
                                        .when_some(on_click.clone(), move |this, on_click| {
                                            this.on_mouse_down(
                                                MouseButton::Left,
                                                move |_, window, cx| {
                                                    window.prevent_default();
                                                    on_click(&ix, window, cx);
                                                },
                                            )
                                        });
                                }

                                if let Some(tooltip_text) = tooltip_text {
                                    segment = segment.tooltip(move |window, cx| {
                                        Tooltip::new(tooltip_text.clone()).build(window, cx)
                                    });
                                }

                                segment
                            })),
                    ),
            )
            .into_any_element()
    }
}

struct IndicatorState {
    start: Instant,
    from: f32,
    to: f32,
    count: usize,
}

struct AnimatedSegmentedIndicator {
    id: ElementId,
    selected_index: usize,
    count: usize,
    duration: Duration,
}

impl IntoElement for AnimatedSegmentedIndicator {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl AnimatedSegmentedIndicator {
    fn value(&self, state: &IndicatorState) -> (f32, bool) {
        if state.from == state.to || self.duration.as_secs_f32() <= 0.0 {
            return (state.to, true);
        }

        let mut t = state.start.elapsed().as_secs_f32() / self.duration.as_secs_f32();
        if t >= 1.0 {
            return (state.to, true);
        }

        t = t.clamp(0.0, 1.0);
        let eased = t * t * (3.0 - 2.0 * t);
        let value = state.from + (state.to - state.from) * eased;
        (value, false)
    }

    fn current_value(&self, state: &IndicatorState) -> f32 {
        self.value(state).0
    }
}

impl Element for AnimatedSegmentedIndicator {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let Some(global_id) = global_id else {
            let mut el = div().into_any_element();
            return (el.request_layout(window, cx), el);
        };

        let selected = self.selected_index.min(self.count.saturating_sub(1));
        let count = self.count.max(1);

        window.with_element_state(global_id, |state, window| {
            let mut state = state.unwrap_or_else(|| IndicatorState {
                start: Instant::now(),
                from: selected as f32,
                to: selected as f32,
                count,
            });

            if state.count != count {
                state = IndicatorState {
                    start: Instant::now(),
                    from: selected as f32,
                    to: selected as f32,
                    count,
                };
            } else if (state.to - selected as f32).abs() > f32::EPSILON {
                let current = self.current_value(&state);
                state.from = current;
                state.to = selected as f32;
                state.start = Instant::now();
            }

            let (value, done) = self.value(&state);
            if !done {
                window.request_animation_frame();
            }

            let left_fraction = (value / count as f32).clamp(0.0, 1.0);
            let width_fraction = (1.0 / count as f32).clamp(0.0, 1.0);

            let theme = cx.theme();
            let indicator = div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(left_fraction))
                .w(relative(width_fraction))
                .bg(theme.background)
                .rounded(theme.radius)
                .when(theme.shadow, |this| this.shadow_md());

            let mut indicator = indicator.into_any_element();
            ((indicator.request_layout(window, cx), indicator), state)
        })
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: gpui::Bounds<gpui::Pixels>,
        element: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        element.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: gpui::Bounds<gpui::Pixels>,
        element: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        element.paint(window, cx);
    }
}
