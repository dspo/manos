use gpui::*;
use gpui_component::Root;

use gpui_component_extras_story::richtext::RichTextExample;

fn main() {
    let app = Application::new();

    app.run(move |cx| {
        gpui_component::init(cx);
        gpui_rich_text::init(cx);
        cx.activate(true);

        cx.spawn(async move |cx| {
            cx.open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("GPUI Component Extras".into()),
                        appears_transparent: false,
                        traffic_light_position: None,
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    let view = RichTextExample::view(window, cx);
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
