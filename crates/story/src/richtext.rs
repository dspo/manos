use std::path::PathBuf;

use gpui::InteractiveElement as _;
use gpui::StatefulInteractiveElement as _;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _, TitleBar,
    WindowExt as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
    menu::AppMenuBar,
    notification::Notification,
    popover::Popover,
};
use gpui_manos_components::plate_toolbar::{
    PlateIconName, PlateToolbarButton, PlateToolbarColorPicker, PlateToolbarDropdownButton,
    PlateToolbarIconButton, PlateToolbarRounding, PlateToolbarSeparator,
};
use gpui_manos_plate::{BlockAlign, CommandInfo, PlateValue, PortabilityReport, RichTextState};

use crate::app_menus::{
    About, COMMAND_PALETTE_CONTEXT, CollectAssets, CommandPalette, CommandPaletteSelectNext,
    CommandPaletteSelectPrev, EmbedLocalImages, ExportPlateBundle, ExportPortableJson,
    FIND_CONTEXT, Find, FindNext, FindPrev, InsertImage, Open,
    PortabilityReport as ShowPortabilityReport, Save, SaveAs, SetLink,
};

pub struct RichTextExample {
    app_menu_bar: Entity<AppMenuBar>,
    editor: Entity<RichTextState>,
    file_path: Option<PathBuf>,
    link_input: Entity<InputState>,
    image_input: Entity<InputState>,
    emoji_input: Entity<InputState>,
    mention_input: Entity<InputState>,
    find_input: Entity<InputState>,
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

        let image_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder(
                "https://example.com/image.png or /path/to/image.png",
                window,
                cx,
            );
            state
        });

        let emoji_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("😀", window, cx);
            state
        });

        let mention_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("@alice", window, cx);
            state
        });

        let find_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("Find…", window, cx);
            state
        });

        cx.observe(&find_input, |this, _, cx| {
            let query = this.find_input.read(cx).value().to_string();
            this.editor.update(cx, |editor, cx| {
                editor.set_find_query(query, cx);
            });
        })
        .detach();

        Self {
            app_menu_bar,
            editor,
            file_path: None,
            link_input,
            image_input,
            emoji_input,
            mention_input,
            find_input,
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

    fn open_image_dialog(&self, initial: String, window: &mut Window, cx: &mut Context<Self>) {
        let image_input = self.image_input.clone();
        let editor = self.editor.clone();

        image_input.update(cx, |state, cx| {
            state.set_value(initial, window, cx);
        });

        window.open_dialog(cx, move |dialog, window, cx| {
            let image_input = image_input.clone();
            let editor = editor.clone();
            let handle = image_input.focus_handle(cx);
            window.focus(&handle);
            let content =
                cx.new(|_cx| InsertImageDialogView::new(editor.clone(), image_input.clone()));
            dialog
                .title("Insert Image")
                .w(px(640.))
                .child(content)
                .on_ok(move |_, window, cx| {
                    let src = image_input.read(cx).value().trim().to_string();
                    if !src.is_empty() {
                        editor.update(cx, |editor, cx| editor.command_insert_image(src, None, cx));
                    }
                    let handle = editor.read(cx).focus_handle();
                    window.focus(&handle);
                    true
                })
        });
    }

    fn insert_image_from_file_picker(&self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Insert Image".into()),
        });
        let editor = self.editor.clone();
        cx.spawn_in(window, async move |_, window| {
            let result = picked.await;
            window
                .update(|window, cx| {
                    let paths = match result {
                        Ok(Ok(Some(paths))) => paths,
                        Ok(Ok(None)) => return,
                        Ok(Err(err)) => {
                            window.push_notification(
                                Notification::new()
                                    .message(format!("Failed to open file picker: {err:?}"))
                                    .autohide(true),
                                cx,
                            );
                            return;
                        }
                        Err(_) => return,
                    };

                    let mut srcs: Vec<String> = Vec::new();
                    for path in paths {
                        let ext = path
                            .extension()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        let is_image = matches!(
                            ext.as_str(),
                            "png"
                                | "jpg"
                                | "jpeg"
                                | "gif"
                                | "webp"
                                | "bmp"
                                | "svg"
                                | "tif"
                                | "tiff"
                        );
                        if !is_image {
                            continue;
                        }
                        srcs.push(path.to_string_lossy().to_string());
                    }

                    if srcs.is_empty() {
                        window.push_notification(
                            Notification::new()
                                .message("No supported image files selected.")
                                .autohide(true),
                            cx,
                        );
                        return;
                    }

                    _ = editor.update(cx, |editor, cx| {
                        if srcs.len() == 1 {
                            editor.command_insert_image(srcs[0].clone(), None, cx);
                        } else {
                            editor.command_insert_images(srcs, cx);
                        }
                    });
                    window.focus(&editor.read(cx).focus_handle());
                })
                .ok();
            Some(())
        })
        .detach();
    }

    fn open_emoji_dialog(&self, initial: String, window: &mut Window, cx: &mut Context<Self>) {
        let emoji_input = self.emoji_input.clone();
        let editor = self.editor.clone();

        emoji_input.update(cx, |state, cx| {
            state.set_value(initial, window, cx);
        });

        window.open_dialog(cx, move |dialog, window, cx| {
            let emoji_input = emoji_input.clone();
            let editor = editor.clone();
            let handle = emoji_input.focus_handle(cx);
            window.focus(&handle);
            dialog
                .title("Insert Emoji")
                .w(px(520.))
                .child(div().p(px(12.)).child(Input::new(&emoji_input).w_full()))
                .on_ok(move |_, window, cx| {
                    let emoji = emoji_input.read(cx).value().trim().to_string();
                    if !emoji.is_empty() {
                        editor.update(cx, |editor, cx| {
                            editor.command_insert_emoji(emoji, cx);
                        });
                    }
                    let handle = editor.read(cx).focus_handle();
                    window.focus(&handle);
                    true
                })
        });
    }

    fn open_mention_dialog(&self, initial: String, window: &mut Window, cx: &mut Context<Self>) {
        let mention_input = self.mention_input.clone();
        let editor = self.editor.clone();

        mention_input.update(cx, |state, cx| {
            state.set_value(initial, window, cx);
        });

        window.open_dialog(cx, move |dialog, window, cx| {
            let mention_input = mention_input.clone();
            let editor = editor.clone();
            let handle = mention_input.focus_handle(cx);
            window.focus(&handle);
            dialog
                .title("Insert Mention")
                .w(px(520.))
                .child(div().p(px(12.)).child(Input::new(&mention_input).w_full()))
                .on_ok(move |_, window, cx| {
                    let label = mention_input.read(cx).value().trim().to_string();
                    if !label.is_empty() {
                        editor.update(cx, |editor, cx| {
                            editor.command_insert_mention(label, cx);
                        });
                    }
                    let handle = editor.read(cx).focus_handle();
                    window.focus(&handle);
                    true
                })
        });
    }

    fn open_find_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        let find_input = self.find_input.clone();
        let editor = self.editor.clone();

        window.open_dialog(cx, move |dialog, window, cx| {
            let find_input = find_input.clone();
            let editor = editor.clone();
            let handle = find_input.focus_handle(cx);
            window.focus(&handle);
            let content = cx.new(|_cx| FindDialogView::new(editor.clone(), find_input.clone()));
            dialog
                .title("Find")
                .w(px(520.))
                .child(content)
                .on_ok(move |_, window, cx| {
                    let handle = editor.read(cx).focus_handle();
                    window.focus(&handle);
                    true
                })
        });
    }

    fn open_portability_report_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        let editor = self.editor.clone();

        window.open_dialog(cx, move |dialog, _window, cx| {
            let editor = editor.clone();
            let content = cx.new(|_cx| PortabilityReportDialogView::new(editor.clone()));
            dialog
                .title("Portability Report")
                .w(px(840.))
                .child(content)
                .on_ok(move |_, window, cx| {
                    let handle = editor.read(cx).focus_handle();
                    window.focus(&handle);
                    true
                })
        });
    }

    fn open_command_palette_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        let editor = self.editor.clone();

        window.open_dialog(cx, move |dialog, window, cx| {
            let palette = cx.new(|cx| CommandPaletteDialog::new(editor.clone(), window, cx));
            let handle = palette.read(cx).search_input.focus_handle(cx);
            window.focus(&handle);
            let editor_for_ok = editor.clone();
            let palette_for_ok = palette.clone();

            dialog
                .title("Command Palette")
                .w(px(760.))
                .child(palette)
                .on_ok(move |_, window, cx| {
                    match palette_for_ok
                        .update(cx, |palette, cx| palette.run_selected_command(window, cx))
                    {
                        Ok(()) => {
                            let handle = editor_for_ok.read(cx).focus_handle();
                            window.focus(&handle);
                            true
                        }
                        Err(err) => {
                            window.push_notification(
                                Notification::new().message(err).autohide(true),
                                cx,
                            );
                            false
                        }
                    }
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
                        let base_dir = path.parent().map(|p| p.to_path_buf());
                        _ = editor.update(cx, |editor, cx| {
                            editor.load_plate_value(value, base_dir, cx)
                        });
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
                let report = self.editor.read(cx).portability_report();
                let mut message = format!("Saved to {}", path.display());
                let errors = report.error_count();
                let warnings = report.warning_count();
                if errors > 0 || warnings > 0 {
                    message.push_str(&format!(
                        " (portability: {errors} error(s), {warnings} warning(s) — File → Portability Report...)"
                    ));
                }
                window.push_notification(Notification::new().message(message), cx);
            }
            return;
        }

        self.save_as(window, cx);
    }

    fn save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.editor.read(cx).plate_value();
        let json = value.to_json_pretty().unwrap_or_else(|_| "{}".to_string());
        let old_base_dir = self
            .file_path
            .as_ref()
            .and_then(|path| path.parent().map(|p| p.to_path_buf()));
        let relative_assets = self.editor.read(cx).referenced_relative_image_paths();

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
        let editor = self.editor.clone();

        cx.spawn_in(window, async move |_, window| {
            let path: PathBuf = picked.await.ok()?.ok()??;

            let base_dir = path.parent().map(|p| p.to_path_buf());
            let mut copied = 0usize;
            let mut skipped = 0usize;
            let mut failed = 0usize;

            if let (Some(old_base_dir), Some(new_base_dir)) =
                (old_base_dir.as_ref(), base_dir.as_ref())
            {
                if !relative_assets.is_empty() {
                    let old_base_dir_canon = std::fs::canonicalize(old_base_dir)
                        .unwrap_or_else(|_| old_base_dir.clone());
                    let new_base_dir_canon = std::fs::canonicalize(new_base_dir)
                        .unwrap_or_else(|_| new_base_dir.clone());
                    if old_base_dir_canon != new_base_dir_canon {
                        for rel in &relative_assets {
                            let rel_path = PathBuf::from(rel);
                            let src_path = old_base_dir.join(&rel_path);
                            let src_canon = match std::fs::canonicalize(&src_path) {
                                Ok(path) => path,
                                Err(_) => {
                                    skipped += 1;
                                    continue;
                                }
                            };
                            if !src_canon.starts_with(&old_base_dir_canon) || !src_canon.is_file() {
                                skipped += 1;
                                continue;
                            }

                            let dest_path = new_base_dir.join(&rel_path);
                            if dest_path.exists() {
                                skipped += 1;
                                continue;
                            }
                            if let Some(parent) = dest_path.parent() {
                                if let Err(_) = std::fs::create_dir_all(parent) {
                                    failed += 1;
                                    continue;
                                }
                            }
                            if std::fs::copy(&src_canon, &dest_path).is_ok() {
                                copied += 1;
                            } else {
                                failed += 1;
                            }
                        }
                    }
                }
            }

            match std::fs::write(&path, json) {
                Ok(()) => {
                    window
                        .update(|window, cx| {
                            _ = this.update(cx, |this, _| this.file_path = Some(path.clone()));
                            let portability: PortabilityReport = editor.update(cx, |editor, cx| {
                                editor.set_document_base_dir(base_dir.clone(), cx);
                                editor.portability_report()
                            });

                            let mut message = format!("Saved to {}", path.display());
                            if copied > 0 || failed > 0 || skipped > 0 {
                                message.push_str(&format!(
                                    " (copied {copied} assets, failed {failed}, skipped {skipped})"
                                ));
                            }
                            let errors = portability.error_count();
                            let warnings = portability.warning_count();
                            if errors > 0 || warnings > 0 {
                                message.push_str(&format!(
                                    " (portability: {errors} error(s), {warnings} warning(s) — File → Portability Report...)"
                                ));
                            }
                            window.push_notification(Notification::new().message(message), cx);
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

    fn export_portable_json(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let portability = self.editor.read(cx).portability_report();
        if portability.images_absolute_missing > 0 || portability.images_relative_missing > 0 {
            window.push_notification(
                Notification::new()
                    .message("Warning: document contains missing images. Check File → Portability Report…")
                    .autohide(true),
                cx,
            );
        }

        let document_base_dir = self
            .file_path
            .as_ref()
            .and_then(|path| path.parent().map(|p| p.to_path_buf()))
            .or_else(|| std::env::current_dir().ok());

        let directory = document_base_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));

        let suggested_name = self
            .file_path
            .as_ref()
            .and_then(|p| p.file_name().map(|name| name.to_string_lossy().to_string()))
            .map(|name| {
                if let Some(stem) = name.strip_suffix(".json") {
                    format!("{stem}.portable.json")
                } else {
                    format!("{name}-portable.json")
                }
            })
            .unwrap_or_else(|| "plate-portable.json".to_string());

        let picked = cx.prompt_for_new_path(&directory, Some(&suggested_name));
        let value = self.editor.read(cx).plate_value();

        cx.spawn_in(window, async move |_, window| {
            let path: PathBuf = picked.await.ok()?.ok()??;

            let (portable, report) =
                RichTextState::make_plate_value_portable(value, document_base_dir);
            let json = portable
                .to_json_pretty()
                .unwrap_or_else(|_| "{}".to_string());

            let result = std::fs::write(&path, json);
            window
                .update(|window, cx| match result {
                    Ok(()) => {
                        let mut message = format!("Exported portable JSON to {}", path.display());
                        if report.embedded > 0 {
                            message.push_str(&format!(" (embedded {})", report.embedded));
                        }
                        if report.failed > 0 {
                            message.push_str(&format!(", failed {}", report.failed));
                        }
                        if report.skipped > 0 {
                            message.push_str(&format!(", skipped {}", report.skipped));
                        }
                        window.push_notification(
                            Notification::new().message(message).autohide(true),
                            cx,
                        );
                    }
                    Err(err) => {
                        window.push_notification(
                            Notification::new()
                                .message(format!("Failed to export: {err}"))
                                .autohide(true),
                            cx,
                        );
                    }
                })
                .ok();

            Some(())
        })
        .detach();
    }

    fn export_plate_bundle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let portability = self.editor.read(cx).portability_report();
        if portability.images_absolute_missing > 0 || portability.images_relative_missing > 0 {
            window.push_notification(
                Notification::new()
                    .message("Warning: document contains missing images. Check File → Portability Report…")
                    .autohide(true),
                cx,
            );
        }

        let document_base_dir = self
            .file_path
            .as_ref()
            .and_then(|path| path.parent().map(|p| p.to_path_buf()))
            .or_else(|| std::env::current_dir().ok());

        let directory = document_base_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));

        let suggested_name = self
            .file_path
            .as_ref()
            .and_then(|p| p.file_name().map(|name| name.to_string_lossy().to_string()))
            .map(|name| {
                if let Some(stem) = name.strip_suffix(".json") {
                    format!("{stem}.bundle.json")
                } else {
                    format!("{name}.bundle.json")
                }
            })
            .unwrap_or_else(|| "plate.bundle.json".to_string());

        let picked = cx.prompt_for_new_path(&directory, Some(&suggested_name));
        let value = self.editor.read(cx).plate_value();

        cx.spawn_in(window, async move |_, window| {
            let path: PathBuf = picked.await.ok()?.ok()??;
            let bundle_dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();

            let (bundled, assets, report) =
                RichTextState::make_plate_value_bundle(value, document_base_dir);

            let assets_dir = bundle_dir.join("assets");
            if let Err(err) = std::fs::create_dir_all(&assets_dir) {
                window
                    .update(|window, cx| {
                        window.push_notification(
                            Notification::new()
                                .message(format!("Failed to create bundle assets dir: {err}"))
                                .autohide(true),
                            cx,
                        );
                    })
                    .ok();
                return Some(());
            }

            for asset in &assets {
                let dest_path = bundle_dir.join(&asset.relative_path);
                if let Some(parent) = dest_path.parent() {
                    if let Err(err) = std::fs::create_dir_all(parent) {
                        window
                            .update(|window, cx| {
                                window.push_notification(
                                    Notification::new()
                                        .message(format!(
                                            "Failed to create assets folder {}: {err}",
                                            parent.display()
                                        ))
                                        .autohide(true),
                                    cx,
                                );
                            })
                            .ok();
                        return Some(());
                    }
                }
                if let Err(err) = std::fs::write(&dest_path, &asset.bytes) {
                    window
                        .update(|window, cx| {
                            window.push_notification(
                                Notification::new()
                                    .message(format!(
                                        "Failed to write asset {}: {err}",
                                        dest_path.display()
                                    ))
                                    .autohide(true),
                                cx,
                            );
                        })
                        .ok();
                    return Some(());
                }
            }

            let json = bundled
                .to_json_pretty()
                .unwrap_or_else(|_| "{}".to_string());
            let result = std::fs::write(&path, json);

            window
                .update(|window, cx| match result {
                    Ok(()) => {
                        let mut message = format!(
                            "Exported Plate bundle to {} (assets {})",
                            path.display(),
                            assets.len()
                        );
                        if report.bundled > 0 {
                            message.push_str(&format!(", bundled {}", report.bundled));
                        }
                        if report.bundled_data_url > 0 {
                            message.push_str(&format!(" (data {})", report.bundled_data_url));
                        }
                        if report.failed > 0 {
                            message.push_str(&format!(", failed {}", report.failed));
                        }
                        if report.skipped > 0 {
                            message.push_str(&format!(", skipped {}", report.skipped));
                        }

                        window.push_notification(
                            Notification::new().message(message).autohide(true),
                            cx,
                        );
                    }
                    Err(err) => {
                        window.push_notification(
                            Notification::new()
                                .message(format!("Failed to export bundle: {err}"))
                                .autohide(true),
                            cx,
                        );
                    }
                })
                .ok();

            Some(())
        })
        .detach();
    }
}

impl Render for RichTextExample {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (
            can_undo,
            can_redo,
            has_selected_block,
            can_move_selected_block_up,
            can_move_selected_block_down,
            marks,
            bulleted,
            ordered,
            table_active,
            columns_active,
            heading_level,
            code_block_active,
            font_size,
            quote,
            toggle,
            toggle_collapsed,
            todo,
            indent_level,
            align,
        ) = {
            let editor = self.editor.read(cx);
            (
                editor.can_undo(),
                editor.can_redo(),
                editor.has_selected_block(),
                editor.can_move_selected_block_up(),
                editor.can_move_selected_block_down(),
                editor.active_marks(),
                editor.is_bulleted_list_active(),
                editor.is_ordered_list_active(),
                editor.is_table_active(),
                editor.is_columns_active(),
                editor.heading_level(),
                editor.is_code_block_active(),
                editor.block_font_size(),
                editor.is_blockquote_active(),
                editor.is_toggle_active(),
                editor.is_toggle_collapsed(),
                editor.is_todo_active(),
                editor.indent_level(),
                editor.block_align(),
            )
        };
        let bold = marks.bold;
        let italic = marks.italic;
        let underline = marks.underline;
        let strikethrough = marks.strikethrough;
        let code = marks.code;
        let link = marks.link.is_some();
        let text_color = marks.text_color.as_deref().and_then(parse_hex_color);
        let highlight_color = marks.highlight_color.as_deref().and_then(parse_hex_color);
        let align_icon = match align {
            BlockAlign::Left => PlateIconName::AlignLeft,
            BlockAlign::Center => PlateIconName::AlignCenter,
            BlockAlign::Right => PlateIconName::AlignRight,
        };
        let font_size_effective =
            effective_font_size(font_size, heading_level, code_block_active).clamp(8, 72);
        let font_size_label: SharedString = match font_size {
            Some(size) => size.to_string().into(),
            None => "A".into(),
        };
        let font_size_reset_tooltip: SharedString = match font_size {
            Some(size) => format!("Font size: {size} (click to reset)").into(),
            None => format!("Font size: {font_size_effective} (default, click to reset)").into(),
        };
        let heading_label = if code_block_active {
            "Code Block".to_string()
        } else {
            match heading_level.unwrap_or(0).clamp(0, 6) {
                1 => "Heading 1".to_string(),
                2 => "Heading 2".to_string(),
                3 => "Heading 3".to_string(),
                4 => "Heading 4".to_string(),
                5 => "Heading 5".to_string(),
                6 => "Heading 6".to_string(),
                _ => "Paragraph".to_string(),
            }
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
            .on_action(cx.listener(|this, _: &EmbedLocalImages, window, cx| {
                let report = this
                    .editor
                    .update(cx, |editor, cx| editor.command_embed_local_images(cx));
                let message = if report.embedded > 0 {
                    let mut msg = format!("Embedded {} local image(s).", report.embedded);
                    if report.failed > 0 {
                        msg.push_str(&format!(" Failed: {}.", report.failed));
                    }
                    if report.skipped > 0 {
                        msg.push_str(&format!(" Skipped: {}.", report.skipped));
                    }
                    msg
                } else if report.failed > 0 {
                    format!("No images embedded. Failed: {}.", report.failed)
                } else {
                    "No local images found to embed.".to_string()
                };
                window.push_notification(Notification::new().message(message).autohide(true), cx);

                let handle = this.editor.read(cx).focus_handle();
                window.focus(&handle);
            }))
            .on_action(cx.listener(|this, _: &ExportPortableJson, window, cx| {
                this.export_portable_json(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ExportPlateBundle, window, cx| {
                this.export_plate_bundle(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CollectAssets, window, cx| {
                if this.file_path.is_none() {
                    window.push_notification(
                        Notification::new()
                            .message(
                                "Save the document first (Save As...) to set a base directory.",
                            )
                            .autohide(true),
                        cx,
                    );
                    return;
                }

                let report = this.editor.update(cx, |editor, cx| {
                    editor.command_collect_assets_into_assets_dir(cx)
                });
                let message = if report.rewritten > 0 || report.assets_written > 0 {
                    let mut msg = format!(
                        "Collected assets into ./assets (rewritten {}, wrote {}).",
                        report.rewritten, report.assets_written
                    );
                    if report.rewritten_data_url > 0 {
                        msg.push_str(&format!(" Data: {}.", report.rewritten_data_url));
                    }
                    if report.failed > 0 {
                        msg.push_str(&format!(" Failed: {}.", report.failed));
                    }
                    if report.skipped > 0 {
                        msg.push_str(&format!(" Skipped: {}.", report.skipped));
                    }
                    msg
                } else if report.failed > 0 {
                    format!("No assets collected. Failed: {}.", report.failed)
                } else {
                    "No images found to collect.".to_string()
                };
                window.push_notification(Notification::new().message(message).autohide(true), cx);

                let handle = this.editor.read(cx).focus_handle();
                window.focus(&handle);
            }))
            .on_action(cx.listener(|this, _: &ShowPortabilityReport, window, cx| {
                this.open_portability_report_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CommandPalette, window, cx| {
                this.open_command_palette_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &InsertImage, window, cx| {
                this.open_image_dialog(String::new(), window, cx);
            }))
            .on_action(cx.listener(|this, _: &SetLink, window, cx| {
                if this.editor.read(cx).has_selected_block() {
                    window.push_notification(
                        Notification::new()
                            .message("Set Link is unavailable when a block is selected.")
                            .autohide(true),
                        cx,
                    );
                    return;
                }
                let initial = this.editor.read(cx).active_link_url().unwrap_or_default();
                this.open_link_dialog(initial, window, cx);
            }))
            .on_action(cx.listener(|this, _: &Find, window, cx| {
                this.open_find_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &FindNext, window, cx| {
                this.editor.update(cx, |editor, cx| {
                    editor.find_next(cx);
                });
                let handle = this.editor.read(cx).focus_handle();
                window.focus(&handle);
            }))
            .on_action(cx.listener(|this, _: &FindPrev, window, cx| {
                this.editor.update(cx, |editor, cx| {
                    editor.find_prev(cx);
                });
                let handle = this.editor.read(cx).focus_handle();
                window.focus(&handle);
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
                    .id("richtext-toolbar")
                    .flex()
                    .flex_row()
                    .w_full()
                    .overflow_x_scroll()
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
                        PlateToolbarIconButton::new("block-duplicate", PlateIconName::Plus)
                            .disabled(!has_selected_block)
                            .tooltip("Duplicate selected block (Alt+Click block first)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_duplicate_selected_block(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("block-move-up", PlateIconName::ArrowUpToLine)
                            .disabled(!can_move_selected_block_up)
                            .tooltip("Move selected block up (Alt+Click block first)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_move_selected_block_up(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new(
                            "block-move-down",
                            PlateIconName::ArrowDownToLine,
                        )
                        .disabled(!can_move_selected_block_down)
                        .tooltip("Move selected block down (Alt+Click block first)")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.editor
                                .update(cx, |ed, cx| ed.command_move_selected_block_down(cx));
                            let handle = this.editor.read(cx).focus_handle();
                            window.focus(&handle);
                        })),
                    )
                    .child(
                        PlateToolbarIconButton::new("block-delete", PlateIconName::ListCollapse)
                            .disabled(!has_selected_block)
                            .tooltip("Delete selected block (Alt+Click block first)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_delete_selected_block(cx));
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
                        PlateToolbarIconButton::new("image", PlateIconName::Image)
                            .tooltip(
                                "Insert image (Click: URL/path dialog, Shift+Click: file picker)",
                            )
                            .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                                if event.modifiers().shift {
                                    this.insert_image_from_file_picker(window, cx);
                                } else {
                                    this.open_image_dialog(String::new(), window, cx);
                                }
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("mention", PlateIconName::MessageSquareText)
                            .tooltip("Insert mention (Cmd/Ctrl+Shift+M inserts default)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_mention_dialog(String::new(), window, cx);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("emoji", PlateIconName::Smile)
                            .tooltip("Insert emoji")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_emoji_dialog(String::new(), window, cx);
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
                    .child({
                        let editor = self.editor.clone();

                        fn row_above(ed: &mut RichTextState, cx: &mut Context<RichTextState>) {
                            ed.command_insert_table_row_above(cx);
                        }
                        fn row_below(ed: &mut RichTextState, cx: &mut Context<RichTextState>) {
                            ed.command_insert_table_row_below(cx);
                        }
                        fn delete_row(ed: &mut RichTextState, cx: &mut Context<RichTextState>) {
                            ed.command_delete_table_row(cx);
                        }
                        fn delete_table(ed: &mut RichTextState, cx: &mut Context<RichTextState>) {
                            ed.command_delete_table(cx);
                        }

                        Popover::new("plate-toolbar-table-row-menu")
                            .appearance(false)
                            .trigger(
                                PlateToolbarDropdownButton::new("plate-toolbar-table-row-trigger")
                                    .disabled(!table_active)
                                    .tooltip("Table row")
                                    .child(PlateIconName::ArrowDownToLine)
                                    .on_click(|_, _, _| {}),
                            )
                            .content(move |_, _window, cx| {
                                let theme = cx.theme();
                                let popover = cx.entity();

                                let editor_for_items = editor.clone();
                                let make_item = move |id: &'static str,
                                                      label: &'static str,
                                                      run: fn(
                                    &mut RichTextState,
                                    &mut Context<RichTextState>,
                                )| {
                                    let popover = popover.clone();
                                    let editor = editor_for_items.clone();
                                    div()
                                        .id(id)
                                        .flex()
                                        .items_center()
                                        .justify_start()
                                        .h(px(28.))
                                        .px(px(10.))
                                        .rounded(px(6.))
                                        .bg(theme.transparent)
                                        .text_color(theme.popover_foreground)
                                        .cursor_pointer()
                                        .hover(|this| {
                                            this.bg(theme.accent)
                                                .text_color(theme.accent_foreground)
                                        })
                                        .active(|this| {
                                            this.bg(theme.accent)
                                                .text_color(theme.accent_foreground)
                                        })
                                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                            window.prevent_default();
                                            editor.update(cx, |editor, cx| run(editor, cx));
                                            let handle = editor.read(cx).focus_handle();
                                            window.focus(&handle);
                                            popover.update(cx, |state, cx| {
                                                state.dismiss(window, cx);
                                            });
                                        })
                                        .child(label)
                                };

                                div()
                                    .p(px(6.))
                                    .bg(theme.popover)
                                    .border_1()
                                    .border_color(theme.border)
                                    .rounded(theme.radius)
                                    .shadow_md()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.))
                                    .child(make_item(
                                        "plate-toolbar-table-row-above",
                                        "Insert row above",
                                        row_above,
                                    ))
                                    .child(make_item(
                                        "plate-toolbar-table-row-below",
                                        "Insert row below",
                                        row_below,
                                    ))
                                    .child(make_item(
                                        "plate-toolbar-table-row-delete",
                                        "Delete row",
                                        delete_row,
                                    ))
                                    .child(div().h(px(1.)).bg(theme.border).mx(px(6.)).my(px(2.)))
                                    .child(make_item(
                                        "plate-toolbar-table-delete",
                                        "Delete table",
                                        delete_table,
                                    ))
                            })
                    })
                    .child({
                        let editor = self.editor.clone();

                        fn col_left(ed: &mut RichTextState, cx: &mut Context<RichTextState>) {
                            ed.command_insert_table_col_left(cx);
                        }
                        fn col_right(ed: &mut RichTextState, cx: &mut Context<RichTextState>) {
                            ed.command_insert_table_col_right(cx);
                        }
                        fn delete_col(ed: &mut RichTextState, cx: &mut Context<RichTextState>) {
                            ed.command_delete_table_col(cx);
                        }

                        Popover::new("plate-toolbar-table-col-menu")
                            .appearance(false)
                            .trigger(
                                PlateToolbarDropdownButton::new("plate-toolbar-table-col-trigger")
                                    .disabled(!table_active)
                                    .tooltip("Table column")
                                    .child(PlateIconName::IndentIncrease)
                                    .on_click(|_, _, _| {}),
                            )
                            .content(move |_, _window, cx| {
                                let theme = cx.theme();
                                let popover = cx.entity();

                                let editor_for_items = editor.clone();
                                let make_item = move |id: &'static str,
                                                      label: &'static str,
                                                      run: fn(
                                    &mut RichTextState,
                                    &mut Context<RichTextState>,
                                )| {
                                    let popover = popover.clone();
                                    let editor = editor_for_items.clone();
                                    div()
                                        .id(id)
                                        .flex()
                                        .items_center()
                                        .justify_start()
                                        .h(px(28.))
                                        .px(px(10.))
                                        .rounded(px(6.))
                                        .bg(theme.transparent)
                                        .text_color(theme.popover_foreground)
                                        .cursor_pointer()
                                        .hover(|this| {
                                            this.bg(theme.accent)
                                                .text_color(theme.accent_foreground)
                                        })
                                        .active(|this| {
                                            this.bg(theme.accent)
                                                .text_color(theme.accent_foreground)
                                        })
                                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                            window.prevent_default();
                                            editor.update(cx, |editor, cx| run(editor, cx));
                                            let handle = editor.read(cx).focus_handle();
                                            window.focus(&handle);
                                            popover.update(cx, |state, cx| {
                                                state.dismiss(window, cx);
                                            });
                                        })
                                        .child(label)
                                };

                                div()
                                    .p(px(6.))
                                    .bg(theme.popover)
                                    .border_1()
                                    .border_color(theme.border)
                                    .rounded(theme.radius)
                                    .shadow_md()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.))
                                    .child(make_item(
                                        "plate-toolbar-table-col-left",
                                        "Insert column left",
                                        col_left,
                                    ))
                                    .child(make_item(
                                        "plate-toolbar-table-col-right",
                                        "Insert column right",
                                        col_right,
                                    ))
                                    .child(make_item(
                                        "plate-toolbar-table-col-delete",
                                        "Delete column",
                                        delete_col,
                                    ))
                            })
                    })
                    .child(
                        PlateToolbarIconButton::new("columns", PlateIconName::AlignJustify)
                            .selected(columns_active)
                            .tooltip(if columns_active {
                                "Unwrap columns"
                            } else {
                                "Insert columns (2)"
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor.update(cx, |ed, cx| {
                                    if ed.is_columns_active() {
                                        ed.command_unwrap_columns(cx);
                                    } else {
                                        ed.command_insert_columns(2, cx);
                                    }
                                });
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(PlateToolbarSeparator)
                    .child({
                        let editor = self.editor.clone();
                        let heading_label = heading_label.clone();
                        let heading_level = heading_level;

                        Popover::new("plate-toolbar-heading-menu")
                            .appearance(false)
                            .trigger(
                                PlateToolbarDropdownButton::new("plate-toolbar-heading-trigger")
                                    .min_width(px(120.))
                                    .tooltip("Block type")
                                    .child(div().child(heading_label))
                                    .on_click(|_, _, _| {}),
                            )
                            .content(move |_, _window, cx| {
                                let theme = cx.theme();
                                let popover = cx.entity();

                                let editor_for_items = editor.clone();
                                let make_heading_item =
                                    |id: &'static str,
                                     label: &'static str,
                                     target: Option<u64>,
                                     selected: bool| {
                                        let popover = popover.clone();
                                        let editor = editor_for_items.clone();
                                        div()
                                            .id(id)
                                            .flex()
                                            .items_center()
                                            .justify_start()
                                            .h(px(28.))
                                            .px(px(10.))
                                            .rounded(px(6.))
                                            .bg(theme.transparent)
                                            .text_color(theme.popover_foreground)
                                            .cursor_pointer()
                                            .hover(|this| {
                                                this.bg(theme.accent)
                                                    .text_color(theme.accent_foreground)
                                            })
                                            .when(selected, |this| {
                                                this.bg(theme.accent)
                                                    .text_color(theme.accent_foreground)
                                            })
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                move |_, window, cx| {
                                                    window.prevent_default();
                                                    editor.update(cx, |editor, cx| {
                                                        if editor.is_code_block_active() {
                                                            editor.command_toggle_code_block(cx);
                                                        }
                                                        if let Some(level) = target {
                                                            editor.command_set_heading(level, cx);
                                                        } else {
                                                            editor.command_unset_heading(cx);
                                                        }
                                                    });
                                                    let handle = editor.read(cx).focus_handle();
                                                    window.focus(&handle);
                                                    popover.update(cx, |state, cx| {
                                                        state.dismiss(window, cx);
                                                    });
                                                },
                                            )
                                            .child(label)
                                    };

                                let editor_for_items = editor.clone();
                                let make_code_block_item =
                                    |id: &'static str, label: &'static str, selected: bool| {
                                        let popover = popover.clone();
                                        let editor = editor_for_items.clone();
                                        div()
                                            .id(id)
                                            .flex()
                                            .items_center()
                                            .justify_start()
                                            .h(px(28.))
                                            .px(px(10.))
                                            .rounded(px(6.))
                                            .bg(theme.transparent)
                                            .text_color(theme.popover_foreground)
                                            .cursor_pointer()
                                            .hover(|this| {
                                                this.bg(theme.accent)
                                                    .text_color(theme.accent_foreground)
                                            })
                                            .when(selected, |this| {
                                                this.bg(theme.accent)
                                                    .text_color(theme.accent_foreground)
                                            })
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                move |_, window, cx| {
                                                    window.prevent_default();
                                                    editor.update(cx, |editor, cx| {
                                                        if !editor.is_code_block_active() {
                                                            editor.command_toggle_code_block(cx);
                                                        }
                                                    });
                                                    let handle = editor.read(cx).focus_handle();
                                                    window.focus(&handle);
                                                    popover.update(cx, |state, cx| {
                                                        state.dismiss(window, cx);
                                                    });
                                                },
                                            )
                                            .child(label)
                                    };

                                let selected_level = heading_level;
                                let code_block_selected = code_block_active;
                                div()
                                    .p(px(6.))
                                    .bg(theme.popover)
                                    .border_1()
                                    .border_color(theme.border)
                                    .rounded(theme.radius)
                                    .shadow_md()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(make_heading_item(
                                        "plate-toolbar-heading-paragraph",
                                        "Paragraph",
                                        None,
                                        !code_block_selected && selected_level.is_none(),
                                    ))
                                    .child(make_heading_item(
                                        "plate-toolbar-heading-1",
                                        "Heading 1",
                                        Some(1),
                                        !code_block_selected && selected_level == Some(1),
                                    ))
                                    .child(make_heading_item(
                                        "plate-toolbar-heading-2",
                                        "Heading 2",
                                        Some(2),
                                        !code_block_selected && selected_level == Some(2),
                                    ))
                                    .child(make_heading_item(
                                        "plate-toolbar-heading-3",
                                        "Heading 3",
                                        Some(3),
                                        !code_block_selected && selected_level == Some(3),
                                    ))
                                    .child(make_heading_item(
                                        "plate-toolbar-heading-4",
                                        "Heading 4",
                                        Some(4),
                                        !code_block_selected && selected_level == Some(4),
                                    ))
                                    .child(make_heading_item(
                                        "plate-toolbar-heading-5",
                                        "Heading 5",
                                        Some(5),
                                        !code_block_selected && selected_level == Some(5),
                                    ))
                                    .child(make_heading_item(
                                        "plate-toolbar-heading-6",
                                        "Heading 6",
                                        Some(6),
                                        !code_block_selected && selected_level == Some(6),
                                    ))
                                    .child(make_code_block_item(
                                        "plate-toolbar-heading-code-block",
                                        "Code Block",
                                        code_block_selected,
                                    ))
                            })
                    })
                    .child({
                        div()
                            .flex()
                            .items_center()
                            .gap(px(0.))
                            .child(
                                PlateToolbarButton::new("font-size-decrease")
                                    .rounding(PlateToolbarRounding::Left)
                                    .disabled(has_selected_block)
                                    .tooltip("Decrease font size")
                                    .child("A-")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let (font_size, heading_level, code_block_active) = {
                                            let editor = this.editor.read(cx);
                                            (
                                                editor.block_font_size(),
                                                editor.heading_level(),
                                                editor.is_code_block_active(),
                                            )
                                        };
                                        let base = effective_font_size(
                                            font_size,
                                            heading_level,
                                            code_block_active,
                                        );
                                        let next = base.saturating_sub(1).clamp(8, 72);
                                        this.editor.update(cx, |ed, cx| {
                                            ed.command_set_font_size(next, cx)
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(
                                PlateToolbarButton::new("font-size-reset")
                                    .rounding(PlateToolbarRounding::None)
                                    .disabled(has_selected_block)
                                    .tooltip(font_size_reset_tooltip.clone())
                                    .child(font_size_label.clone())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor
                                            .update(cx, |ed, cx| ed.command_unset_font_size(cx));
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                            .child(
                                PlateToolbarButton::new("font-size-increase")
                                    .rounding(PlateToolbarRounding::Right)
                                    .disabled(has_selected_block)
                                    .tooltip("Increase font size")
                                    .child("A+")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let (font_size, heading_level, code_block_active) = {
                                            let editor = this.editor.read(cx);
                                            (
                                                editor.block_font_size(),
                                                editor.heading_level(),
                                                editor.is_code_block_active(),
                                            )
                                        };
                                        let base = effective_font_size(
                                            font_size,
                                            heading_level,
                                            code_block_active,
                                        );
                                        let next = base.saturating_add(1).clamp(8, 72);
                                        this.editor.update(cx, |ed, cx| {
                                            ed.command_set_font_size(next, cx)
                                        });
                                        let handle = this.editor.read(cx).focus_handle();
                                        window.focus(&handle);
                                    })),
                            )
                    })
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
                        PlateToolbarIconButton::new("italic", PlateIconName::Italic)
                            .selected(italic)
                            .tooltip("Italic (Cmd/Ctrl+I)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_toggle_italic(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("underline", PlateIconName::Underline)
                            .selected(underline)
                            .tooltip("Underline (Cmd/Ctrl+U)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_toggle_underline(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("strikethrough", PlateIconName::Strikethrough)
                            .selected(strikethrough)
                            .tooltip("Strikethrough (Cmd/Ctrl+Shift+X)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_toggle_strikethrough(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("code", PlateIconName::CodeXml)
                            .selected(code)
                            .tooltip("Code (Cmd/Ctrl+E)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor.update(cx, |ed, cx| ed.command_toggle_code(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarColorPicker::new("text-color", PlateIconName::Baseline)
                            .tooltip("Text color")
                            .value(text_color)
                            .on_change({
                                let editor = self.editor.clone();
                                move |color, window, cx| {
                                    editor.update(cx, |editor, cx| {
                                        if let Some(color) = color {
                                            editor.command_set_text_color(hsla_to_hex(color), cx);
                                        } else {
                                            editor.command_unset_text_color(cx);
                                        }
                                    });
                                    let handle = editor.read(cx).focus_handle();
                                    window.focus(&handle);
                                }
                            }),
                    )
                    .child(
                        PlateToolbarColorPicker::new("highlight-color", PlateIconName::PaintBucket)
                            .tooltip("Highlight color")
                            .value(highlight_color)
                            .on_change({
                                let editor = self.editor.clone();
                                move |color, window, cx| {
                                    editor.update(cx, |editor, cx| {
                                        if let Some(color) = color {
                                            editor.command_set_highlight_color(
                                                hsla_to_hex(color),
                                                cx,
                                            );
                                        } else {
                                            editor.command_unset_highlight_color(cx);
                                        }
                                    });
                                    let handle = editor.read(cx).focus_handle();
                                    window.focus(&handle);
                                }
                            }),
                    )
                    .child({
                        let active_align = align;
                        let trigger_icon = align_icon;
                        let editor = self.editor.clone();

                        Popover::new("plate-toolbar-align-menu")
                            .appearance(false)
                            .trigger(
                                PlateToolbarDropdownButton::new("plate-toolbar-align-trigger")
                                    .tooltip("Align")
                                    .child(trigger_icon)
                                    .on_click(|_, _, _| {}),
                            )
                            .content(move |_, _window, cx| {
                                let theme = cx.theme();
                                let popover = cx.entity();

                                let editor_for_items = editor.clone();
                                let make_item =
                                    move |id: &'static str,
                                          icon: PlateIconName,
                                          align: BlockAlign,
                                          selected: bool| {
                                        let popover = popover.clone();
                                        let editor = editor_for_items.clone();

                                        div()
                                            .id(id)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .h(px(32.))
                                            .w(px(32.))
                                            .rounded(px(4.))
                                            .bg(theme.transparent)
                                            .text_color(theme.popover_foreground)
                                            .cursor_pointer()
                                            .hover(|this| {
                                                this.bg(theme.accent)
                                                    .text_color(theme.accent_foreground)
                                            })
                                            .active(|this| {
                                                this.bg(theme.accent)
                                                    .text_color(theme.accent_foreground)
                                            })
                                            .when(selected, |this| {
                                                this.bg(theme.accent)
                                                    .text_color(theme.accent_foreground)
                                            })
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                move |_, window, cx| {
                                                    window.prevent_default();
                                                    editor.update(cx, |editor, cx| {
                                                        editor.command_set_align(align, cx);
                                                    });
                                                    let handle = editor.read(cx).focus_handle();
                                                    window.focus(&handle);
                                                    popover.update(cx, |state, cx| {
                                                        state.dismiss(window, cx);
                                                    });
                                                },
                                            )
                                            .child(gpui_component::Icon::new(icon))
                                    };

                                div()
                                    .p(px(4.))
                                    .bg(theme.popover)
                                    .border_1()
                                    .border_color(theme.border)
                                    .rounded(theme.radius)
                                    .shadow_md()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.))
                                    .child(make_item(
                                        "plate-toolbar-align-left",
                                        PlateIconName::AlignLeft,
                                        BlockAlign::Left,
                                        active_align == BlockAlign::Left,
                                    ))
                                    .child(make_item(
                                        "plate-toolbar-align-center",
                                        PlateIconName::AlignCenter,
                                        BlockAlign::Center,
                                        active_align == BlockAlign::Center,
                                    ))
                                    .child(make_item(
                                        "plate-toolbar-align-right",
                                        PlateIconName::AlignRight,
                                        BlockAlign::Right,
                                        active_align == BlockAlign::Right,
                                    ))
                            })
                    })
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
                        PlateToolbarIconButton::new(
                            "indent-decrease",
                            PlateIconName::IndentDecrease,
                        )
                        .disabled(indent_level == 0)
                        .tooltip("Outdent")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.editor
                                .update(cx, |ed, cx| ed.command_indent_decrease(cx));
                            let handle = this.editor.read(cx).focus_handle();
                            window.focus(&handle);
                        })),
                    )
                    .child(
                        PlateToolbarIconButton::new(
                            "indent-increase",
                            PlateIconName::IndentIncrease,
                        )
                        .disabled(indent_level >= 8)
                        .tooltip("Indent")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.editor
                                .update(cx, |ed, cx| ed.command_indent_increase(cx));
                            let handle = this.editor.read(cx).focus_handle();
                            window.focus(&handle);
                        })),
                    )
                    .child(
                        PlateToolbarIconButton::new("todo", PlateIconName::ListTodo)
                            .selected(todo)
                            .tooltip("Todo")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor.update(cx, |ed, cx| ed.command_toggle_todo(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("blockquote", PlateIconName::WrapText)
                            .selected(quote)
                            .tooltip("Blockquote")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_toggle_blockquote(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("toggle", PlateIconName::ListCollapse)
                            .selected(toggle)
                            .tooltip("Toggle (wrap/unwrap)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_toggle_toggle(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("toggle-collapse", PlateIconName::ChevronDown)
                            .disabled(!toggle)
                            .selected(toggle_collapsed)
                            .tooltip(if toggle_collapsed {
                                "Expand"
                            } else {
                                "Collapse"
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor
                                    .update(cx, |ed, cx| ed.command_toggle_collapsed(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(
                        PlateToolbarIconButton::new("link", PlateIconName::Link)
                            .disabled(has_selected_block)
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
                    .child(
                        PlateToolbarIconButton::new("unlink", PlateIconName::Unlink)
                            .disabled(!link || has_selected_block)
                            .tooltip("Unlink")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.editor.update(cx, |ed, cx| ed.command_unset_link(cx));
                                let handle = this.editor.read(cx).focus_handle();
                                window.focus(&handle);
                            })),
                    )
                    .child(PlateToolbarSeparator)
                    .child(
                        PlateToolbarIconButton::new("command-palette", PlateIconName::Ellipsis)
                            .tooltip("Command Palette")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_command_palette_dialog(window, cx);
                            })),
                    )
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

struct InsertImageDialogView {
    editor: Entity<RichTextState>,
    image_input: Entity<InputState>,
}

impl InsertImageDialogView {
    fn new(editor: Entity<RichTextState>, image_input: Entity<InputState>) -> Self {
        Self {
            editor,
            image_input,
        }
    }

    fn pick_image_file(&self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Select Image".into()),
        });
        let image_input = self.image_input.clone();

        cx.spawn_in(window, async move |_, window| {
            let result = picked.await;
            window
                .update(|window, cx| {
                    let mut paths = match result {
                        Ok(Ok(Some(paths))) => paths,
                        Ok(Ok(None)) => return,
                        Ok(Err(err)) => {
                            window.push_notification(
                                Notification::new()
                                    .message(format!("Failed to open file picker: {err:?}"))
                                    .autohide(true),
                                cx,
                            );
                            return;
                        }
                        Err(_) => return,
                    };

                    let Some(path) = paths.pop() else {
                        return;
                    };

                    let ext = path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let is_image = matches!(
                        ext.as_str(),
                        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "tif" | "tiff"
                    );
                    if !is_image {
                        window.push_notification(
                            Notification::new()
                                .message("Selected file is not a supported image type.")
                                .autohide(true),
                            cx,
                        );
                        return;
                    }

                    image_input.update(cx, |state, cx| {
                        state.set_value(path.to_string_lossy().to_string(), window, cx);
                    });
                    window.focus(&image_input.focus_handle(cx));
                })
                .ok();
            Some(())
        })
        .detach();
    }
}

impl Render for InsertImageDialogView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let base_dir_label = self
            .editor
            .read(cx)
            .document_base_dir()
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "— (not set)".to_string());

        div()
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(Input::new(&self.image_input).w_full())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(Button::new("Browse…").small().ghost().on_click(cx.listener(
                        |this, _, window, cx| {
                            this.pick_image_file(window, cx);
                        },
                    )))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child(format!("Base dir: {base_dir_label}")),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(theme.muted_foreground)
                    .child(
                        "Tip: paste a URL, an absolute path, or a path relative to the base dir.",
                    ),
            )
    }
}

struct FindDialogView {
    editor: Entity<RichTextState>,
    find_input: Entity<InputState>,
}

impl FindDialogView {
    fn new(editor: Entity<RichTextState>, find_input: Entity<InputState>) -> Self {
        Self { editor, find_input }
    }
}

impl Render for FindDialogView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (current, total) = self.editor.read(cx).find_stats();
        let has_matches = total > 0;
        let stats = if has_matches {
            format!("{current} / {total} matches")
        } else {
            "No matches".to_string()
        };

        div()
            .key_context(FIND_CONTEXT)
            .on_action(cx.listener(|this, _: &FindNext, window, cx| {
                this.editor.update(cx, |editor, cx| editor.find_next(cx));
                let handle = this.find_input.focus_handle(cx);
                window.focus(&handle);
            }))
            .on_action(cx.listener(|this, _: &FindPrev, window, cx| {
                this.editor.update(cx, |editor, cx| editor.find_prev(cx));
                let handle = this.find_input.focus_handle(cx);
                window.focus(&handle);
            }))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(Input::new(&self.find_input).w_full())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child(stats),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                Button::new("Prev")
                                    .small()
                                    .ghost()
                                    .disabled(!has_matches)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| editor.find_prev(cx));
                                        let handle = this.find_input.focus_handle(cx);
                                        window.focus(&handle);
                                    })),
                            )
                            .child(
                                Button::new("Next")
                                    .small()
                                    .ghost()
                                    .disabled(!has_matches)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.editor.update(cx, |editor, cx| editor.find_next(cx));
                                        let handle = this.find_input.focus_handle(cx);
                                        window.focus(&handle);
                                    })),
                            ),
                    ),
            )
    }
}

struct PortabilityReportDialogView {
    editor: Entity<RichTextState>,
}

impl PortabilityReportDialogView {
    fn new(editor: Entity<RichTextState>) -> Self {
        Self { editor }
    }

    fn export_portable_json(&self, window: &mut Window, cx: &mut Context<Self>) {
        let document_base_dir = self
            .editor
            .read(cx)
            .document_base_dir()
            .or_else(|| std::env::current_dir().ok());
        let directory = document_base_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));
        let picked = cx.prompt_for_new_path(&directory, Some("plate-portable.json"));
        let value = self.editor.read(cx).plate_value();

        cx.spawn_in(window, async move |_, window| {
            let path: PathBuf = picked.await.ok()?.ok()??;
            let (portable, report) =
                RichTextState::make_plate_value_portable(value, document_base_dir);
            let json = portable
                .to_json_pretty()
                .unwrap_or_else(|_| "{}".to_string());
            let result = std::fs::write(&path, json);

            window
                .update(|window, cx| match result {
                    Ok(()) => {
                        let mut message = format!("Exported portable JSON to {}", path.display());
                        if report.embedded > 0 {
                            message.push_str(&format!(" (embedded {})", report.embedded));
                        }
                        if report.failed > 0 {
                            message.push_str(&format!(", failed {}", report.failed));
                        }
                        if report.skipped > 0 {
                            message.push_str(&format!(", skipped {}", report.skipped));
                        }
                        window.push_notification(
                            Notification::new().message(message).autohide(true),
                            cx,
                        );
                    }
                    Err(err) => {
                        window.push_notification(
                            Notification::new()
                                .message(format!("Failed to export: {err}"))
                                .autohide(true),
                            cx,
                        );
                    }
                })
                .ok();
            Some(())
        })
        .detach();
    }

    fn export_plate_bundle(&self, window: &mut Window, cx: &mut Context<Self>) {
        let document_base_dir = self
            .editor
            .read(cx)
            .document_base_dir()
            .or_else(|| std::env::current_dir().ok());
        let directory = document_base_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));
        let picked = cx.prompt_for_new_path(&directory, Some("plate.bundle.json"));
        let value = self.editor.read(cx).plate_value();

        cx.spawn_in(window, async move |_, window| {
            let path: PathBuf = picked.await.ok()?.ok()??;
            let bundle_dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();

            let (bundled, assets, report) =
                RichTextState::make_plate_value_bundle(value, document_base_dir);

            let assets_dir = bundle_dir.join("assets");
            if let Err(err) = std::fs::create_dir_all(&assets_dir) {
                window
                    .update(|window, cx| {
                        window.push_notification(
                            Notification::new()
                                .message(format!("Failed to create bundle assets dir: {err}"))
                                .autohide(true),
                            cx,
                        );
                    })
                    .ok();
                return Some(());
            }

            for asset in &assets {
                let dest_path = bundle_dir.join(&asset.relative_path);
                if let Some(parent) = dest_path.parent() {
                    if let Err(err) = std::fs::create_dir_all(parent) {
                        window
                            .update(|window, cx| {
                                window.push_notification(
                                    Notification::new()
                                        .message(format!(
                                            "Failed to create assets folder {}: {err}",
                                            parent.display()
                                        ))
                                        .autohide(true),
                                    cx,
                                );
                            })
                            .ok();
                        return Some(());
                    }
                }
                if let Err(err) = std::fs::write(&dest_path, &asset.bytes) {
                    window
                        .update(|window, cx| {
                            window.push_notification(
                                Notification::new()
                                    .message(format!(
                                        "Failed to write asset {}: {err}",
                                        dest_path.display()
                                    ))
                                    .autohide(true),
                                cx,
                            );
                        })
                        .ok();
                    return Some(());
                }
            }

            let json = bundled
                .to_json_pretty()
                .unwrap_or_else(|_| "{}".to_string());
            let result = std::fs::write(&path, json);

            window
                .update(|window, cx| match result {
                    Ok(()) => {
                        let mut message = format!(
                            "Exported Plate bundle to {} (assets {})",
                            path.display(),
                            assets.len()
                        );
                        if report.bundled > 0 {
                            message.push_str(&format!(", bundled {}", report.bundled));
                        }
                        if report.bundled_data_url > 0 {
                            message.push_str(&format!(" (data {})", report.bundled_data_url));
                        }
                        if report.failed > 0 {
                            message.push_str(&format!(", failed {}", report.failed));
                        }
                        if report.skipped > 0 {
                            message.push_str(&format!(", skipped {}", report.skipped));
                        }
                        window.push_notification(
                            Notification::new().message(message).autohide(true),
                            cx,
                        );
                    }
                    Err(err) => {
                        window.push_notification(
                            Notification::new()
                                .message(format!("Failed to export bundle: {err}"))
                                .autohide(true),
                            cx,
                        );
                    }
                })
                .ok();

            Some(())
        })
        .detach();
    }
}

impl Render for PortabilityReportDialogView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let report = self.editor.read(cx).portability_report();

        let base_dir_label = report
            .base_dir
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "— (not set)".to_string());

        let errors = report.error_count();
        let warnings = report.warning_count();

        let mut issue_items: Vec<AnyElement> = Vec::new();
        for (ix, issue) in report.issues.iter().take(200).enumerate() {
            let level = match issue.level {
                gpui_manos_plate::PortabilityIssueLevel::Error => "ERROR",
                gpui_manos_plate::PortabilityIssueLevel::Warning => "WARN",
            };
            let level_color = match issue.level {
                gpui_manos_plate::PortabilityIssueLevel::Error => theme.accent,
                gpui_manos_plate::PortabilityIssueLevel::Warning => theme.muted_foreground,
            };

            issue_items.push(
                div()
                    .id(("portability-issue", ix))
                    .p(px(8.))
                    .border_1()
                    .border_color(theme.border)
                    .rounded(theme.radius / 2.)
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(level_color)
                                    .child(level),
                            )
                            .child(div().text_size(px(13.)).child(issue.message.clone())),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child(issue.src.clone()),
                    )
                    .into_any_element(),
            );
        }

        div()
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme.muted_foreground)
                                    .child("Base directory"),
                            )
                            .child(div().text_size(px(13.)).child(base_dir_label))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme.muted_foreground)
                                    .child(format!(
                                        "Images: {} (data {}, http {}, abs ok {}, abs missing {}, rel ok {}, rel missing {})",
                                        report.images_total,
                                        report.images_data_url,
                                        report.images_http,
                                        report.images_absolute_ok,
                                        report.images_absolute_missing,
                                        report.images_relative_ok,
                                        report.images_relative_missing
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme.muted_foreground)
                                    .child(format!("{errors} error(s), {warnings} warning(s)")),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                Button::new("Collect Assets")
                                    .small()
                                    .ghost()
                                    .disabled(report.base_dir.is_none())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let report =
                                            this.editor.update(cx, |editor, cx| {
                                                editor.command_collect_assets_into_assets_dir(cx)
                                            });
                                        let message = if report.rewritten > 0
                                            || report.assets_written > 0
                                        {
                                            let mut msg = format!(
                                                "Collected assets into ./assets (rewritten {}, wrote {}).",
                                                report.rewritten, report.assets_written
                                            );
                                            if report.rewritten_data_url > 0 {
                                                msg.push_str(&format!(
                                                    " Data: {}.",
                                                    report.rewritten_data_url
                                                ));
                                            }
                                            if report.failed > 0 {
                                                msg.push_str(&format!(" Failed: {}.", report.failed));
                                            }
                                            if report.skipped > 0 {
                                                msg.push_str(&format!(" Skipped: {}.", report.skipped));
                                            }
                                            msg
                                        } else if report.failed > 0 {
                                            format!(
                                                "No assets collected. Failed: {}.",
                                                report.failed
                                            )
                                        } else {
                                            "No images found to collect.".to_string()
                                        };
                                        window.push_notification(
                                            Notification::new()
                                                .message(message)
                                                .autohide(true),
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("Export Bundle…")
                                    .small()
                                    .ghost()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.export_plate_bundle(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("Export Portable JSON…")
                                    .small()
                                    .ghost()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.export_portable_json(window, cx);
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child("Issues (first 200)"),
                    )
	                    .child(
	                        div()
	                            .id("portability-report-issues")
	                            .max_h(px(420.))
	                            .overflow_y_scroll()
	                            .flex()
	                            .flex_col()
	                            .gap(px(8.))
                            .children(issue_items),
                    ),
            )
    }
}

struct CommandPaletteDialog {
    editor: Entity<RichTextState>,
    commands: Vec<CommandInfo>,
    search_input: Entity<InputState>,
    args_input: Entity<InputState>,
    selected_index: usize,
    last_search: String,
}

impl CommandPaletteDialog {
    fn new(editor: Entity<RichTextState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let commands = editor.read(cx).command_list();

        let search_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("Search by id / label / keyword…", window, cx);
            state
        });

        let args_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("Args JSON (optional)…", window, cx);
            state
        });

        Self {
            editor,
            commands,
            search_input,
            args_input,
            selected_index: 0,
            last_search: String::new(),
        }
    }

    fn filtered_indices(&self, search: &str) -> Vec<usize> {
        self.commands
            .iter()
            .enumerate()
            .filter_map(|(ix, cmd)| {
                if search.is_empty() {
                    return Some(ix);
                }

                let search = search.trim();
                if search.is_empty() {
                    return Some(ix);
                }

                let id = cmd.id.to_lowercase();
                let label = cmd.label.to_lowercase();
                if id.contains(search) || label.contains(search) {
                    return Some(ix);
                }

                if cmd
                    .keywords
                    .iter()
                    .any(|kw| kw.to_lowercase().contains(search))
                {
                    return Some(ix);
                }

                None
            })
            .collect()
    }

    fn select_prev(&mut self, cx: &mut Context<Self>) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            cx.notify();
        }
    }

    fn select_next(&mut self, cx: &mut Context<Self>) {
        let search_raw = self.search_input.read(cx).value();
        let search = search_raw.trim().to_lowercase();
        let filtered_len = self.filtered_indices(&search).len();
        if filtered_len == 0 {
            self.selected_index = 0;
            return;
        }
        let max_index = filtered_len - 1;
        if self.selected_index < max_index {
            self.selected_index += 1;
            cx.notify();
        }
    }

    fn run_selected_command(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let search_raw = self.search_input.read(cx).value();
        let search = search_raw.trim().to_lowercase();
        let filtered = self.filtered_indices(&search);

        let Some(&command_ix) =
            filtered.get(self.selected_index.min(filtered.len().saturating_sub(1)))
        else {
            return Err("No command selected".to_string());
        };

        let command_id = self.commands[command_ix].id.clone();

        let args_raw = self.args_input.read(cx).value();
        let args_trimmed = args_raw.trim();
        let args_json: Option<serde_json::Value> = if args_trimmed.is_empty() {
            None
        } else {
            Some(
                serde_json::from_str(args_trimmed)
                    .map_err(|err| format!("Invalid args JSON: {err}"))?,
            )
        };

        self.editor.update(cx, |editor, cx| {
            editor.command_run(&command_id, args_json, cx)
        })
    }
}

impl Render for CommandPaletteDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let search_raw = self.search_input.read(cx).value();
        let search = search_raw.trim().to_lowercase();

        if search != self.last_search {
            self.last_search = search.clone();
            self.selected_index = 0;
        }

        let filtered = self.filtered_indices(&search);
        if filtered.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= filtered.len() {
            self.selected_index = filtered.len() - 1;
        }

        let selected = filtered
            .get(self.selected_index)
            .and_then(|ix| self.commands.get(*ix));

        let selected_description = selected
            .and_then(|cmd| cmd.description.as_ref())
            .cloned()
            .unwrap_or_else(|| "No description".to_string());

        let selected_keywords = selected
            .map(|cmd| {
                if cmd.keywords.is_empty() {
                    "—".to_string()
                } else {
                    cmd.keywords.join(", ")
                }
            })
            .unwrap_or_else(|| "—".to_string());

        let selected_args_example = selected
            .and_then(|cmd| cmd.args_example.as_ref())
            .and_then(|v| serde_json::to_string_pretty(v).ok());

        let filtered_commands: Vec<(String, String)> = filtered
            .iter()
            .filter_map(|&ix| {
                self.commands
                    .get(ix)
                    .map(|cmd| (cmd.id.clone(), cmd.label.clone()))
            })
            .collect();

        let args_input = self.args_input.clone();
        let editor = self.editor.clone();
        let selected_index = self.selected_index;
        let items =
            filtered_commands
                .into_iter()
                .enumerate()
                .map(move |(row_ix, (command_id, label))| {
                    let args_input = args_input.clone();
                    let editor = editor.clone();
                    let command_id_for_run = command_id.clone();
                    let is_selected = row_ix == selected_index;

                    let mut row = div()
                        .id(SharedString::from(command_id.clone()))
                        .flex()
                        .items_center()
                        .justify_between()
                        .h(px(32.))
                        .px(px(10.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .hover(|this| this.bg(theme.accent).text_color(theme.accent_foreground))
                        .active(|this| this.bg(theme.accent).text_color(theme.accent_foreground))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            window.prevent_default();

                            let args_raw = args_input.read(cx).value();
                            let args_trimmed = args_raw.trim();
                            let args_json: Option<serde_json::Value> = if args_trimmed.is_empty() {
                                None
                            } else {
                                match serde_json::from_str(args_trimmed) {
                                    Ok(value) => Some(value),
                                    Err(err) => {
                                        window.push_notification(
                                            Notification::new()
                                                .message(format!("Invalid args JSON: {err}"))
                                                .autohide(true),
                                            cx,
                                        );
                                        return;
                                    }
                                }
                            };

                            match editor.update(cx, |editor, cx| {
                                editor.command_run(&command_id_for_run, args_json, cx)
                            }) {
                                Ok(()) => {
                                    window.close_dialog(cx);
                                    let handle = editor.read(cx).focus_handle();
                                    window.focus(&handle);
                                }
                                Err(err) => {
                                    window.push_notification(
                                        Notification::new().message(err).autohide(true),
                                        cx,
                                    );
                                }
                            }
                        });

                    row = if is_selected {
                        row.bg(theme.accent).text_color(theme.accent_foreground)
                    } else {
                        row.bg(theme.transparent)
                            .text_color(theme.popover_foreground)
                    };

                    row.child(div().flex_1().min_w(px(0.)).child(label)).child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.muted_foreground)
                            .child(command_id),
                    )
                });

        let args_example_text = selected_args_example
            .clone()
            .unwrap_or_else(|| "—".to_string());
        let args_example_for_fill = selected_args_example.clone();

        div()
            .key_context(COMMAND_PALETTE_CONTEXT)
            .on_action(cx.listener(|this, _: &CommandPaletteSelectPrev, _, cx| {
                this.select_prev(cx);
            }))
            .on_action(cx.listener(|this, _: &CommandPaletteSelectNext, _, cx| {
                this.select_next(cx);
            }))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(Input::new(&self.search_input).w_full())
                    .child(Input::new(&self.args_input).w_full()),
            )
            .child(
                div()
                    .flex()
                    .gap(px(10.))
                    .items_start()
                    .child(
                        div()
                            .id("command-palette-list")
                            .flex_1()
                            .max_h(px(420.))
                            .overflow_y_scroll()
                            .border_1()
                            .border_color(theme.border)
                            .rounded(theme.radius)
                            .bg(theme.popover)
                            .p(px(6.))
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .children(items),
                    )
                    .child(
                        div()
                            .w(px(280.))
                            .border_1()
                            .border_color(theme.border)
                            .rounded(theme.radius)
                            .bg(theme.popover)
                            .p(px(10.))
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme.muted_foreground)
                                    .child("Description"),
                            )
                            .child(div().text_size(px(13.)).child(selected_description))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme.muted_foreground)
                                    .child("Keywords"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme.muted_foreground)
                                    .child(selected_keywords),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme.muted_foreground)
                                    .child("Args example"),
                            )
                            .child(
                                div()
                                    .border_1()
                                    .border_color(theme.border)
                                    .rounded(px(6.))
                                    .bg(theme.background)
                                    .p(px(8.))
                                    .text_size(px(11.))
                                    .text_color(theme.muted_foreground)
                                    .child(args_example_text),
                            )
                            .child(
                                Button::new("Use args example")
                                    .small()
                                    .ghost()
                                    .disabled(args_example_for_fill.is_none())
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let Some(example) = args_example_for_fill.clone() else {
                                            return;
                                        };
                                        this.args_input.update(cx, |state, cx| {
                                            state.set_value(example, window, cx);
                                        });
                                        let handle = this.args_input.read(cx).focus_handle(cx);
                                        window.focus(&handle);
                                    })),
                            ),
                    ),
            )
    }
}

fn parse_hex_color(value: &str) -> Option<Hsla> {
    Rgba::try_from(value).ok().map(Hsla::from)
}

fn hsla_to_hex(color: Hsla) -> String {
    let rgba: Rgba = color.into();
    let r = (rgba.r * 255.0).round() as u8;
    let g = (rgba.g * 255.0).round() as u8;
    let b = (rgba.b * 255.0).round() as u8;
    let a = (rgba.a * 255.0).round() as u8;
    format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
}

fn default_heading_font_size(level: u64) -> u64 {
    match level.clamp(1, 6) {
        1 => 28,
        2 => 22,
        3 => 18,
        4 => 16,
        5 => 14,
        _ => 13,
    }
}

fn effective_font_size(
    explicit: Option<u64>,
    heading_level: Option<u64>,
    code_block_active: bool,
) -> u64 {
    const DEFAULT_TEXT_BLOCK_FONT_SIZE: u64 = 16;
    explicit.unwrap_or_else(|| {
        if code_block_active {
            return DEFAULT_TEXT_BLOCK_FONT_SIZE;
        }
        heading_level
            .map(default_heading_font_size)
            .unwrap_or(DEFAULT_TEXT_BLOCK_FONT_SIZE)
    })
}
