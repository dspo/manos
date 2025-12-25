use gpui::*;
use gpui_component::{Root, TitleBar};
use gpui_manos_components::assets::ExtrasAssetSource;

use anyhow::Error;
use gpui_manos_components_story::{app_menus, richtext_next, themes};

fn main() {
    let app = Application::new().with_assets(ExtrasAssetSource::new());

    app.run(move |cx| {
        gpui_component::init(cx);
        themes::init(cx);
        richtext_next::init(cx);
        cx.activate(true);

        cx.spawn(async move |cx| {
            cx.open_window(
                WindowOptions {
                    titlebar: Some(TitleBar::title_bar_options()),
                    ..Default::default()
                },
                |window, cx| {
                    window.set_window_title("Rich Text (Next Core)");
                    let app_menu_bar = app_menus::init("Mano", window, cx);
                    let view = richtext_next::RichTextNextExample::view(app_menu_bar, window, cx);
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )?;

            Ok::<_, Error>(())
        })
        .detach();
    });
}
