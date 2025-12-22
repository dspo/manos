use gpui::*;
use gpui_component::{Root, TitleBar};
use gpui_manos_components::assets::ExtrasAssetSource;

use gpui_manos_components_story::app_menus;
use gpui_manos_components_story::richtext::RichTextExample;
use gpui_manos_components_story::themes;
use gpui_component_extras_story::dnd_tree::DndTreeExample;

fn main() {
    let app = Application::new().with_assets(ExtrasAssetSource::new());

    app.run(move |cx| {
        gpui_component::init(cx);
        gpui_manos_plate::init(cx);
        themes::init(cx);
        cx.activate(true);

        cx.spawn(async move |cx| {
            cx.open_window(
                WindowOptions {
                    titlebar: Some(TitleBar::title_bar_options()),
                    ..Default::default()
                },
                |window, cx| {
                    window.set_window_title("Manos Components");
                    let app_menu_bar = app_menus::init("Manos Components", window, cx);
                    let view = RichTextExample::view(app_menu_bar, window, cx);
                    let view = DndTreeExample::view(window, cx);
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
