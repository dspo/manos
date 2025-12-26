use std::path::PathBuf;

use gpui::*;
use gpui_component::{
    ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _, TitleBar,
    WindowExt as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
    menu::AppMenuBar,
    notification::Notification,
};
use gpui_manos_components::plate_toolbar::{
    PlateIconName, PlateToolbarIconButton, PlateToolbarSeparator,
};
use gpui_manos_plate::{PlateValue, RichTextState};

use crate::app_menus::{About, Open, Save, SaveAs};

pub struct RichTextExample {
    app_menu_bar: Entity<AppMenuBar>,
    editor: Entity<RichTextState>,
    file_path: Option<PathBuf>,
    link_input: Entity<InputState>,
}

impl RichTextExample {
    pub fn new(
        app_menu_bar: Entity<AppMenuBar>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor = cx.new(|cx| RichTextState::new(window, cx));
        let focus_handle = editor.read(cx).focus_handle();
        window.focus(&focus_handle);

        let link_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("https://example.com", window, cx);
            state
        });

        Self {
            app_menu_bar,
            editor,
            file_path: None,
            link_input,
        }
    }

    pub fn view(
        app_menu_bar: Entity<AppMenuBar>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| Self::new(app_menu_bar, window, cx))
    }

    fn open_link_dialog(&self, initial: String, window: &mut Window, cx: &mut Context<Self>) {
        let link_input = self.link_input.clone();
        let editor = self.editor.clone();

        link_input.update(cx, |state, cx| {
            state.set_value(initial, window, cx);
        });

        window.open_dialog(cx, move |dialog, window, cx| {
            let link_input = link_input.clone();
            let editor = editor.clone();
            let handle = link_input.focus_handle(cx);
            window.focus(&handle);
            dialog
                .title("Set Link")
                .w(px(520.))
                .child(div().p(px(12.)).child(Input::new(&link_input).w_full()))
                .on_ok(move |_, window, cx| {
                    let url = link_input.read(cx).value().trim().to_string();
                    editor.update(cx, |editor, cx| {
                        if url.is_empty() {
                            editor.command_unset_link(cx);
                        } else {
                            editor.command_set_link(url, cx);
                        }
                    });
                    let handle = editor.read(cx).focus_handle();
                    window.focus(&handle);
                    true
                })
        });
    }

    fn open_from_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open Plate JSON".into()),
        });

        let editor = self.editor.clone();
        let this = cx.entity();
        cx.spawn_in(window, async move |_, window| {
            let path: PathBuf = picked.await.ok()?.ok()??.into_iter().next()?;
            let content = std::fs::read_to_string(&path).ok()?;
            let parsed = PlateValue::from_json_str(&content).ok();
            window
                .update(|window, cx| match parsed {
                    Some(value) => {
                        _ = this.update(cx, |this, _| this.file_path = Some(path.clone()));
                        _ = editor.update(cx, |editor, cx| editor.load_plate_value(value, cx));
                    }
                    None => {
                        window.push_notification(
                            Notification::new().message("Failed to parse JSON"),
                            cx,
                        );
                    }
                })
                .ok();
            Some(())
        })
        .detach();
    }

    fn save_to_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.editor.read(cx).plate_value();
        let json = value.to_json_pretty().unwrap_or_else(|_| "{}".to_string());

        if let Some(path) = self.file_path.clone() {
            if std::fs::write(&path, json).is_ok() {
                window.push_notification(
                    Notification::new().message(format!("Saved to {}", path.display())),
                    cx,
                );
            }
            return;
        }

        self.save_as(window, cx);
    }

    fn save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.editor.read(cx).plate_value();
        let json = value.to_json_pretty().unwrap_or_else(|_| "{}".to_string());

        let directory = self
            .file_path
            .as_ref()
            .and_then(|path| path.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let suggested_name = self
            .file_path
            .as_ref()
            .and_then(|p| p.file_name().map(|name| name.to_string_lossy().to_string()))
            .unwrap_or_else(|| "plate.json".to_string());

        let picked = cx.prompt_for_new_path(&directory, Some(&suggested_name));
        let this = cx.entity();

        cx.spawn_in(window, async move |_, window| {
            let path: PathBuf = picked.await.ok()?.ok()??;
            match std::fs::write(&path, json) {
                Ok(()) => {
                    window
                        .update(|window, cx| {
                            _ = this.update(cx, |this, _| this.file_path = Some(path.clone()));
                            window.push_notification(
                                Notification::new().message(format!("Saved to {}", path.display())),
                                cx,
                            );
                        })
                        .ok();
                }
                Err(err) => {
                    window
                        .update(|window, cx| {
                            window.push_notification(
                                Notification::new().message(format!("Failed to save: {err}")),
                                cx,
                            );
                        })
                        .ok();
                }
            }
            Some(())
        })
        .detach();
    }
}

impl Render for RichTextExample {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (can_undo, can_redo, bold, link, bulleted, ordered, table_active) = {
            let editor = self.editor.read(cx);
            (
                editor.can_undo(),
                editor.can_redo(),
                editor.is_bold_active(),
                editor.has_link_active(),
                editor.is_bulleted_list_active(),
                editor.is_ordered_list_active(),
                editor.is_table_active(),
            )
        };

        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(theme.muted)
            .on_action(cx.listener(|this, _: &Open, window, cx| {
                this.open_from_file(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Save, window, cx| {
                this.save_to_file(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveAs, window, cx| {
                this.save_as(window, cx);
            }))
            .on_action(cx.listener(|_, _: &About, window, cx| {
                window.push_notification(
                    Notification::new()
                        .message("Manos Rich Text (Plate Core)")
                        .autohide(true),
                    cx,
                );
            }))
            .child(
                TitleBar::new().child(div().flex().items_center().child(self.app_menu_bar.clone())),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .p(px(8.))
                    .bg(theme.background)
                    .border_b_1()
                    .border_color(theme.border)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        PlateToolbarIconButton::new("undo", PlateIconName::Undo2)
                            .disabled(!can_undo)
                            .tooltip("Undo")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor.update(cx, |ed, cx| ed.command_undo(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("redo", PlateIconName::Redo2)
                            .disabled(!can_redo)
                            .tooltip("Redo")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor.update(cx, |ed, cx| ed.command_redo(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(PlateToolbarSeparator)
                    .child(
                        PlateToolbarIconButton::new("divider", PlateIconName::Minus)
                            .tooltip("Insert divider (Cmd/Ctrl+Shift+D)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_insert_divider(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("mention", PlateIconName::MessageSquareText)
                            .tooltip("Insert mention (Cmd/Ctrl+Shift+M)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor.update(cx, |ed, cx| {
                                    ed.command_insert_mention("Alice".into(), cx)
                                });
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("table", PlateIconName::Table)
                            .tooltip("Insert table (2×2)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_insert_table(2, 2, cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("table-row", PlateIconName::ArrowDownToLine)
                            .disabled(!table_active)
                            .tooltip("Insert row below")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_insert_table_row_below(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("table-col", PlateIconName::IndentIncrease)
                            .disabled(!table_active)
                            .tooltip("Insert column right")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_insert_table_col_right(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new(
                            "table-delete-row",
                            PlateIconName::ArrowUpToLine,
                        )
                        .disabled(!table_active)
                        .tooltip("Delete row")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.editor
                                .update(cx, |ed, cx| ed.command_delete_table_row(cx));
                            let handle = this.editor.read(cx).focus_handle();
                            window.focus(&handle);
                        })),
                    )
                    .child(
                        PlateToolbarIconButton::new(
                            "table-delete-col",
                            PlateIconName::IndentDecrease,
                        )
                        .disabled(!table_active)
                        .tooltip("Delete column")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.editor
                                .update(cx, |ed, cx| ed.command_delete_table_col(cx));
                            let handle = this.editor.read(cx).focus_handle();
                            window.focus(&handle);
                        })),
                    )
                    .child(PlateToolbarSeparator)
                    .child(
                        PlateToolbarIconButton::new("bold", PlateIconName::Bold)
                            .selected(bold)
                            .tooltip("Bold (Cmd/Ctrl+B)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor.update(cx, |ed, cx| ed.command_toggle_bold(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("list", PlateIconName::List)
                            .selected(bulleted)
                            .tooltip("Bulleted list (Cmd/Ctrl+Shift+8)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_toggle_bulleted_list(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("list-ordered", PlateIconName::ListOrdered)
                            .selected(ordered)
                            .tooltip("Ordered list (Cmd/Ctrl+Shift+7)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_toggle_ordered_list(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("link", PlateIconName::Link)
                            .selected(link)
                            .tooltip(if link {
                                "Edit link (clear URL to remove)"
                            } else {
                                "Set link"
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                let initial =
                                    this.editor.read(cx).active_link_url().unwrap_or_default();
                                this.open_link_dialog(initial, window, cx);
                            })),
                    )
                    .child(PlateToolbarSeparator)
                    .child(
                        Button::new("open")
                            .icon(IconName::FolderOpen)
                            .small()
                            .ghost()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_from_file(window, cx)),
                            ),
                    )
                    .child(Button::new("save").small().ghost().on_click(
                        cx.listener(|this, _, window, cx| this.save_to_file(window, cx)),
                    )),
            )
            .child(div().flex_1().min_h(px(0.)).child(self.editor.clone()))
    }
}
