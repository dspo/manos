use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_manos_webview::webview::WebView;
use gpui_manos_webview::wry::WebViewId;

const DEFAULT_URL: &str = "https://www.gpui.rs/";

pub struct SimpleBrowserStory {
    url_input: Entity<InputState>,
    webview: Entity<WebView>,
    visible: bool,
}

impl SimpleBrowserStory {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let url_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder(
                "Enter an URL in GPUI component and render in webview",
                window,
                cx,
            );
            state.set_value(DEFAULT_URL.to_string(), window, cx);
            state
        });

        let webview = build_webview(window, cx, DEFAULT_URL);

        cx.subscribe(&url_input, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.navigate_from_input(cx);
            }
        })
        .detach();

        window.focus(&url_input.focus_handle(cx));

        Self {
            url_input,
            webview,
            visible: true,
        }
    }

    pub fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;

        if visible {
            self.webview.update(cx, |webview, _| webview.show());
        } else {
            self.webview.update(cx, |webview, _| webview.hide());
        }

        cx.notify();
    }

    fn navigate_from_input(&mut self, cx: &mut Context<Self>) {
        let raw = self.url_input.read(cx).value().trim().to_string();
        if raw.is_empty() {
            return;
        }

        let url = normalize_url(&raw);
        self.webview.update(cx, |webview, _| webview.load_url(&url));
    }
}

impl Render for SimpleBrowserStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .size_full()
            .bg(theme.background)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_x_2()
                    .px(px(12.))
                    .py(px(10.))
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.background)
                    .child(
                        Icon::new(IconName::Globe)
                            .size_4()
                            .text_color(theme.muted_foreground),
                    )
                    .child(Input::new(&self.url_input).flex_1().min_w(px(0.)))
                    .child(
                        Button::new("navigate")
                            .small()
                            .primary()
                            .label("Go")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.navigate_from_input(cx);
                            })),
                    ),
            )
            .child(div().flex_1().min_h(px(0.)).child(self.webview.clone()))
    }
}

fn build_webview(window: &mut Window, cx: &mut App, url: &str) -> Entity<WebView> {
    let url = url.to_string();
    cx.new(|cx: &mut Context<WebView>| {
        let builder = gpui_manos_webview::Builder::new()
            .with_webview_id(WebViewId::from("simple_browser"))
            .apply(move |b| b.with_url(url.clone()));

        let webview = builder.build_as_child(window).unwrap();
        WebView::new(webview, window, cx)
    })
}

fn normalize_url(raw: &str) -> String {
    if raw.contains("://") {
        raw.to_string()
    } else {
        format!("https://{raw}")
    }
}
