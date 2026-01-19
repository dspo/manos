use gpui::*;
use gpui_component::{Root, TitleBar};
use gpui_manos_components::assets::ExtrasAssetSource;
use gpui_manos_components_story::app_menus;
use gpui_manos_components_story::gallery::StoryGallery;
use gpui_manos_components_story::themes;
use std::future::Future;
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

fn main() {
    let repo_root = repo_root();
    let app = Application::new()
        .with_assets(ExtrasAssetSource::new())
        .with_http_client(Arc::new(RepoFileHttpClient::new(repo_root)));

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
                    window.set_window_title("Manos Stories");
                    let app_menu_bar = app_menus::init("Manos Stories", window, cx);
                    let view = StoryGallery::view(app_menu_bar, window, cx);
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}

fn repo_root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    root.canonicalize().unwrap_or(root)
}

struct RepoFileHttpClient {
    root: PathBuf,
}

impl RepoFileHttpClient {
    fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl http_client::HttpClient for RepoFileHttpClient {
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn user_agent(&self) -> Option<&http_client::http::HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&http_client::Url> {
        None
    }

    fn send(
        &self,
        req: http_client::Request<http_client::AsyncBody>,
    ) -> Pin<
        Box<
            dyn Future<Output = anyhow::Result<http_client::Response<http_client::AsyncBody>>>
                + Send
                + 'static,
        >,
    > {
        let root = self.root.clone();
        Box::pin(async move {
            if req.method() != http_client::Method::GET {
                return Ok(http_client::Response::builder()
                    .status(http_client::StatusCode::METHOD_NOT_ALLOWED)
                    .body(http_client::AsyncBody::empty())
                    .unwrap());
            }

            let uri = req.uri();
            if uri.scheme_str().is_some() {
                return Ok(http_client::Response::builder()
                    .status(http_client::StatusCode::NOT_FOUND)
                    .body(http_client::AsyncBody::empty())
                    .unwrap());
            }

            let path = uri.path().trim_start_matches('/');
            if path.is_empty() {
                return Ok(http_client::Response::builder()
                    .status(http_client::StatusCode::NOT_FOUND)
                    .body(http_client::AsyncBody::empty())
                    .unwrap());
            }

            let rel = Path::new(path);
            if rel.components().any(|c| {
                matches!(
                    c,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            }) {
                return Ok(http_client::Response::builder()
                    .status(http_client::StatusCode::FORBIDDEN)
                    .body(http_client::AsyncBody::empty())
                    .unwrap());
            }

            let file_path = root.join(rel);
            let bytes = match std::fs::read(&file_path) {
                Ok(bytes) => bytes,
                Err(_) => {
                    return Ok(http_client::Response::builder()
                        .status(http_client::StatusCode::NOT_FOUND)
                        .body(http_client::AsyncBody::empty())
                        .unwrap());
                }
            };

            Ok(http_client::Response::builder()
                .status(http_client::StatusCode::OK)
                .body(bytes.into())
                .unwrap())
        })
    }
}
