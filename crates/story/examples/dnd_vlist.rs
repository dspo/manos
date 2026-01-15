use gpui::*;
use gpui_component::{Root, TitleBar};
use gpui_manos_components::assets::ExtrasAssetSource;
use gpui_manos_components_story::dnd_vlist::DndVListExample;
use gpui_manos_components_story::themes;

fn main() {
    let app = Application::new().with_assets(ExtrasAssetSource::new());

    app.run(move |cx| {
        gpui_component::init(cx);
        themes::init(cx);
        cx.activate(true);

        cx.spawn(async move |cx| {
            cx.open_window(
                WindowOptions {
                    titlebar: Some(TitleBar::title_bar_options()),
                    ..Default::default()
                },
                |window, cx| {
                    window.set_window_title("DnD VList");
                    let view = DndVListExample::view(window, cx);
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
