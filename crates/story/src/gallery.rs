use gpui::*;
use gpui_component::ActiveTheme as _;
use gpui_component::menu::AppMenuBar;
use gpui_component::sidebar::{Sidebar, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem};
use gpui_component::{Icon, IconName, Selectable as _, v_flex};

use crate::dnd_list::DndListExample;
use crate::dnd_tree::DndTreeExample;
use crate::dnd_vlist::DndVListExample;
use crate::dnd_vtree::DndVTreeExample;
use crate::plate_toolbar_buttons::PlateToolbarButtonsStory;
use crate::richtext::RichTextExample;
use crate::simple_browser::SimpleBrowserStory;
use crate::webview_story::WebViewStory;

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
            StoryId::Introduction => "Introduction",
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

    fn render_introduction(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        v_flex()
            .size_full()
            .p(px(24.))
            .gap_y_3()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .child("Manos Stories"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("A unified Story Gallery: pick a story on the left, preview and interact on the right."),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Welcome Tauri requires web assets: `cd crates/story/examples/webview-app && pnpm install && pnpm build`."),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Rich Text Editor exposes menu actions (Open/Save/...). Select the story first before using the menu."),
            )
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
            SidebarMenu::new().child(item(StoryId::Introduction, "Introduction", IconName::Info));

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

impl Render for StoryGallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content: AnyElement = match self.selected {
            StoryId::Introduction => self.render_introduction(cx),
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
