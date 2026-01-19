use gpui::*;
use gpui_component::ActiveTheme as _;
use gpui_component::menu::AppMenuBar;
use gpui_component::sidebar::{Sidebar, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem};
use gpui_component::text::TextView;
use gpui_component::{Icon, IconName, Selectable as _, v_flex};

use crate::dnd_list::DndListExample;
use crate::dnd_tree::DndTreeExample;
use crate::dnd_vlist::DndVListExample;
use crate::dnd_vtree::DndVTreeExample;
use crate::plate_toolbar_buttons::PlateToolbarButtonsStory;
use crate::richtext::RichTextExample;
use crate::simple_browser::SimpleBrowserStory;
use crate::webview_story::WebViewStory;

const README_MD: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../README.md"));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StoryId {
    Introduction,
    DndList,
    DndTree,
    DndVList,
    DndVTree,
    RichText,
    PlateToolbarButtons,
    WelcomeTauri,
    SimpleBrowser,
}

impl StoryId {
    fn title(self) -> &'static str {
        match self {
            StoryId::Introduction => "README.md",
            StoryId::DndList => "DnD List",
            StoryId::DndTree => "DnD Tree",
            StoryId::DndVList => "DnD VList",
            StoryId::DndVTree => "DnD VTree",
            StoryId::RichText => "Rich Text Editor",
            StoryId::PlateToolbarButtons => "Plate Toolbar Buttons",
            StoryId::WelcomeTauri => "Welcome Tauri",
            StoryId::SimpleBrowser => "Simple Browser",
        }
    }
}

pub struct StoryGallery {
    app_menu_bar: Entity<AppMenuBar>,
    selected: StoryId,
    dnd_list: Option<Entity<DndListExample>>,
    dnd_tree: Option<Entity<DndTreeExample>>,
    dnd_vlist: Option<Entity<DndVListExample>>,
    dnd_vtree: Option<Entity<DndVTreeExample>>,
    richtext: Option<Entity<RichTextExample>>,
    plate_toolbar_buttons: Option<Entity<PlateToolbarButtonsStory>>,
    welcome_tauri: Option<Entity<WebViewStory>>,
    simple_browser: Option<Entity<SimpleBrowserStory>>,
}

impl StoryGallery {
    pub fn view(
        app_menu_bar: Entity<AppMenuBar>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|_| Self {
            app_menu_bar,
            selected: StoryId::Introduction,
            dnd_list: None,
            dnd_tree: None,
            dnd_vlist: None,
            dnd_vtree: None,
            richtext: None,
            plate_toolbar_buttons: None,
            welcome_tauri: None,
            simple_browser: None,
        })
    }

    fn select_story(&mut self, next: StoryId, cx: &mut Context<Self>) {
        if self.selected == next {
            return;
        }

        match self.selected {
            StoryId::WelcomeTauri => {
                if let Some(story) = &self.welcome_tauri {
                    story.update(cx, |story, cx| story.set_visible(false, cx));
                }
            }
            StoryId::SimpleBrowser => {
                if let Some(story) = &self.simple_browser {
                    story.update(cx, |story, cx| story.set_visible(false, cx));
                }
            }
            _ => {}
        };

        self.selected = next;

        match self.selected {
            StoryId::WelcomeTauri => {
                if let Some(story) = &self.welcome_tauri {
                    story.update(cx, |story, cx| story.set_visible(true, cx));
                }
            }
            StoryId::SimpleBrowser => {
                if let Some(story) = &self.simple_browser {
                    story.update(cx, |story, cx| story.set_visible(true, cx));
                }
            }
            _ => {}
        };

        cx.notify();
    }

    fn ensure_dnd_list(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<DndListExample> {
        if let Some(view) = &self.dnd_list {
            return view.clone();
        }
        let view = DndListExample::view(window, cx);
        self.dnd_list = Some(view.clone());
        view
    }

    fn ensure_dnd_tree(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<DndTreeExample> {
        if let Some(view) = &self.dnd_tree {
            return view.clone();
        }
        let view = DndTreeExample::view(window, cx);
        self.dnd_tree = Some(view.clone());
        view
    }

    fn ensure_dnd_vlist(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<DndVListExample> {
        if let Some(view) = &self.dnd_vlist {
            return view.clone();
        }
        let view = DndVListExample::view(window, cx);
        self.dnd_vlist = Some(view.clone());
        view
    }

    fn ensure_dnd_vtree(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<DndVTreeExample> {
        if let Some(view) = &self.dnd_vtree {
            return view.clone();
        }
        let view = DndVTreeExample::view(window, cx);
        self.dnd_vtree = Some(view.clone());
        view
    }

    fn ensure_richtext(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<RichTextExample> {
        if let Some(view) = &self.richtext {
            return view.clone();
        }
        let view = RichTextExample::view(self.app_menu_bar.clone(), window, cx);
        self.richtext = Some(view.clone());
        view
    }

    fn ensure_plate_toolbar_buttons(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<PlateToolbarButtonsStory> {
        if let Some(view) = &self.plate_toolbar_buttons {
            return view.clone();
        }
        let view = PlateToolbarButtonsStory::view(window, cx);
        self.plate_toolbar_buttons = Some(view.clone());
        view
    }

    fn ensure_welcome_tauri(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<WebViewStory> {
        if let Some(view) = &self.welcome_tauri {
            return view.clone();
        }
        let view = WebViewStory::view(window, cx);
        self.welcome_tauri = Some(view.clone());
        view
    }

    fn ensure_simple_browser(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<SimpleBrowserStory> {
        if let Some(view) = &self.simple_browser {
            return view.clone();
        }
        let view = SimpleBrowserStory::view(window, cx);
        self.simple_browser = Some(view.clone());
        view
    }

    fn render_readme(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let readme = prepare_readme_markdown(README_MD);

        TextView::markdown("readme-md", readme, window, cx)
            .size_full()
            .scrollable(true)
            .selectable(true)
            .p(px(24.))
            .into_any_element()
    }

    fn sidebar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let gallery = cx.entity();
        let selected = self.selected;

        let item = |id: StoryId, label: &'static str, icon: IconName| {
            SidebarMenuItem::new(label)
                .icon(Icon::new(icon).size_4())
                .active(selected == id)
                .on_click({
                    let gallery = gallery.clone();
                    move |_, _window, cx| {
                        gallery.update(cx, |this, cx| this.select_story(id, cx));
                    }
                })
        };

        let dnd_menu = SidebarMenuItem::new("DnD")
            .icon(Icon::new(IconName::LayoutDashboard).size_4())
            .default_open(true)
            .children([
                item(StoryId::DndList, "List", IconName::Menu),
                item(StoryId::DndTree, "Tree", IconName::FolderOpen),
                item(StoryId::DndVList, "VList", IconName::ChevronsUpDown),
                item(StoryId::DndVTree, "VTree", IconName::Folder),
            ]);

        let stories_menu = SidebarMenu::new().children([
            dnd_menu,
            item(StoryId::RichText, "Rich Text", IconName::ALargeSmall),
            item(
                StoryId::PlateToolbarButtons,
                "Plate Toolbar",
                IconName::Palette,
            ),
            item(StoryId::WelcomeTauri, "Welcome Tauri", IconName::Frame),
            item(StoryId::SimpleBrowser, "Simple Browser", IconName::Globe),
        ]);

        let getting_started_menu =
            SidebarMenu::new().child(item(StoryId::Introduction, "README.md", IconName::Info));

        let header = SidebarHeader::new()
            .child(Icon::new(IconName::GalleryVerticalEnd).size_4())
            .child(div().font_weight(FontWeight::MEDIUM).child("Gallery"))
            .selected(selected == StoryId::Introduction);

        Sidebar::left()
            .header(header)
            .children([
                SidebarGroup::new("Getting Started").child(getting_started_menu),
                SidebarGroup::new("Stories").child(stories_menu),
            ])
            .render(window, cx)
    }
}

fn prepare_readme_markdown(markdown: &str) -> String {
    // Markdown images store their URL as a `SharedUri` and are loaded through gpui's HttpClient.
    // `http::Uri` requires paths to start with `/`, so rewrite repo-relative image URLs like
    // `docs/foo.png` into `/docs/foo.png` for in-app rendering.
    let mut out = String::with_capacity(markdown.len());

    let mut in_fenced_code_block = false;
    let mut fence: Option<&str> = None;

    for line in markdown.lines() {
        let trimmed = line.trim_start();

        if !in_fenced_code_block {
            if trimmed.starts_with("```") {
                in_fenced_code_block = true;
                fence = Some("```");
            } else if trimmed.starts_with("~~~") {
                in_fenced_code_block = true;
                fence = Some("~~~");
            }
        } else if fence.is_some_and(|f| trimmed.starts_with(f)) {
            in_fenced_code_block = false;
            fence = None;
        }

        if in_fenced_code_block {
            out.push_str(line);
        } else {
            out.push_str(&rewrite_repo_relative_image_urls(line));
        }
        out.push('\n');
    }

    if !markdown.ends_with('\n') {
        out.pop();
    }

    out
}

fn rewrite_repo_relative_image_urls(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut i = 0;

    while let Some(rel) = line[i..].find("![") {
        let start = i + rel;
        out.push_str(&line[i..start]);

        // Find `](` after the `![` to locate an inline image destination.
        let Some(close_bracket_rel) = line[start + 2..].find("](") else {
            out.push_str(&line[start..]);
            return out;
        };
        let close_bracket = start + 2 + close_bracket_rel;
        let open_paren = close_bracket + 1; // points to '('

        // Find the matching `)` for this `(` (naive; good enough for README images).
        let Some(close_paren_rel) = line[open_paren + 1..].find(')') else {
            out.push_str(&line[start..]);
            return out;
        };
        let close_paren = open_paren + 1 + close_paren_rel;

        let before = &line[start..open_paren + 1];
        let inside = &line[open_paren + 1..close_paren];
        let after = &line[close_paren..close_paren + 1];

        out.push_str(before);
        out.push_str(&rewrite_image_destination(inside));
        out.push_str(after);

        i = close_paren + 1;
    }

    out.push_str(&line[i..]);
    out
}

fn rewrite_image_destination(inner: &str) -> String {
    let bytes = inner.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }

    if idx >= bytes.len() {
        return inner.to_string();
    }

    let (url_start, url_end) = if bytes[idx] == b'<' {
        let url_start = idx + 1;
        let Some(end_rel) = inner[url_start..].find('>') else {
            return inner.to_string();
        };
        let url_end = url_start + end_rel;
        (url_start, url_end)
    } else {
        let url_start = idx;
        let mut url_end = url_start;
        while url_end < bytes.len() && !bytes[url_end].is_ascii_whitespace() {
            url_end += 1;
        }
        (url_start, url_end)
    };

    let url = &inner[url_start..url_end];
    let Some(rewritten) = rewrite_repo_relative_url(url) else {
        return inner.to_string();
    };

    let mut out = String::with_capacity(inner.len() + 1);
    out.push_str(&inner[..url_start]);
    out.push_str(&rewritten);
    out.push_str(&inner[url_end..]);
    out
}

fn rewrite_repo_relative_url(url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }

    let lower = url.to_ascii_lowercase();
    if url.starts_with('/') || url.starts_with('#') {
        return None;
    }
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("file://")
        || lower.starts_with("data:")
        || lower.starts_with("mailto:")
        || lower.starts_with("tel:")
        || lower.starts_with("javascript:")
    {
        return None;
    }

    let trimmed = url.trim_start_matches("./");
    Some(format!("/{trimmed}"))
}

impl Render for StoryGallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content: AnyElement = match self.selected {
            StoryId::Introduction => self.render_readme(window, cx),
            StoryId::DndList => self.ensure_dnd_list(window, cx).into_any_element(),
            StoryId::DndTree => self.ensure_dnd_tree(window, cx).into_any_element(),
            StoryId::DndVList => self.ensure_dnd_vlist(window, cx).into_any_element(),
            StoryId::DndVTree => self.ensure_dnd_vtree(window, cx).into_any_element(),
            StoryId::RichText => self.ensure_richtext(window, cx).into_any_element(),
            StoryId::PlateToolbarButtons => self
                .ensure_plate_toolbar_buttons(window, cx)
                .into_any_element(),
            StoryId::WelcomeTauri => {
                let view = self.ensure_welcome_tauri(window, cx);
                view.update(cx, |story, cx| story.set_visible(true, cx));
                view.into_any_element()
            }
            StoryId::SimpleBrowser => {
                let view = self.ensure_simple_browser(window, cx);
                view.update(cx, |story, cx| story.set_visible(true, cx));
                view.into_any_element()
            }
        };

        // Hide webviews immediately when they are not selected, otherwise they may remain visible
        // at the last bounds (they are native views).
        if self.selected != StoryId::WelcomeTauri {
            if let Some(story) = &self.welcome_tauri {
                story.update(cx, |story, cx| story.set_visible(false, cx));
            }
        }
        if self.selected != StoryId::SimpleBrowser {
            if let Some(story) = &self.simple_browser {
                story.update(cx, |story, cx| story.set_visible(false, cx));
            }
        }

        v_flex().size_full().child(
            gpui_component::h_flex()
                .size_full()
                .items_start()
                .child(self.sidebar(window, cx))
                .child(
                    v_flex()
                        .flex_1()
                        .h_full()
                        .min_w(px(0.))
                        .min_h(px(0.))
                        .bg(cx.theme().background)
                        .child(
                            v_flex()
                                .size_full()
                                .child(
                                    div()
                                        .w_full()
                                        .border_b_1()
                                        .border_color(cx.theme().border)
                                        .bg(cx.theme().background)
                                        .px(px(16.))
                                        .py(px(12.))
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(self.selected.title()),
                                        ),
                                )
                                .child(div().flex_1().min_h(px(0.)).child(content)),
                        ),
                ),
        )
    }
}
