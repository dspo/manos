use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Context as _;
use anyhow::{Result, anyhow};
use gpui::*;
use gpui_component::{Root, TitleBar, WindowExt as _, button::Button, notification::Notification};

#[derive(Clone, Debug)]
struct FileEntry {
    path: String,
    status: String,
}

struct GitViewerApp {
    repo_root: PathBuf,
    files: Vec<FileEntry>,
    loading: bool,
}

impl GitViewerApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let this = cx.entity();
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let repo_root = detect_repo_root(&cwd);
        let repo_root_for_task = repo_root.clone();

        cx.spawn_in(window, async move |_, window| {
            let entries = fetch_git_status(&repo_root_for_task)
                .map_err(|err| {
                    eprintln!("git status failed: {err:?}");
                    err
                })
                .unwrap_or_default();

            let _ = window.update(|_window, cx| {
                this.update(cx, |this, _cx| {
                    this.loading = false;
                    this.files = entries;
                })
            });

            Some(())
        })
        .detach();

        Self {
            repo_root,
            files: Vec::new(),
            loading: true,
        }
    }
}

impl Render for GitViewerApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header: SharedString = if self.loading {
            "正在加载 git 状态…".into()
        } else {
            format!("发现 {} 个变更文件", self.files.len()).into()
        };

        let list: Vec<AnyElement> = if self.loading {
            vec![div().child("加载中…").into_any_element()]
        } else if self.files.is_empty() {
            vec![div().child("没有检测到变更文件").into_any_element()]
        } else {
            self.files
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    let path = entry.path.clone();
                    let status = entry.status.clone();
                    Button::new(("file", index))
                        .label(format!("{status} {path}"))
                        .w_full()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            println!("[git-viewer] 点击文件: {status} {path}");

                            let repo_root = this.repo_root.clone();
                            let path_for_task = path.clone();
                            cx.spawn_in(window, async move |_, window| {
                                let full_path = repo_root.join(&path_for_task);
                                let result = std::fs::read(&full_path);
                                let message = match result {
                                    Ok(bytes) => format!(
                                        "读取文件成功：{}（{} bytes）",
                                        path_for_task,
                                        bytes.len()
                                    ),
                                    Err(err) => {
                                        format!("读取文件失败：{}（{err}）", path_for_task)
                                    }
                                };

                                window
                                    .update(|window, cx| {
                                        window.push_notification(
                                            Notification::new().message(message),
                                            cx,
                                        );
                                    })
                                    .ok();

                                Some(())
                            })
                            .detach();
                        }))
                        .into_any_element()
                })
                .collect()
        };

        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(12.))
            .child(div().child(header))
            .child(div().flex_col().gap(px(6.)).children(list))
    }
}

fn fetch_git_status(repo_root: &Path) -> Result<Vec<FileEntry>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["status", "--porcelain=v2", "-z"])
        .output()
        .context("执行 git status 失败")?;

    if !output.status.success() {
        return Err(anyhow!(
            "git status 返回非零: {}",
            output.status.code().unwrap_or(-1)
        ));
    }

    let mut entries = Vec::new();
    let mut segments = output.stdout.split(|b| *b == b'\0').peekable();
    while let Some(record) = segments.next() {
        if record.is_empty() {
            continue;
        }

        let record = String::from_utf8_lossy(record);
        if record.starts_with("1 ") {
            if let Some(entry) = parse_type_1_record(&record) {
                entries.push(entry);
            }
            continue;
        }

        if record.starts_with("2 ") {
            if let Some(entry) = parse_type_2_record(&record) {
                entries.push(entry);
            }
            _ = segments.next(); // orig_path
            continue;
        }

        if record.starts_with("u ") {
            if let Some(entry) = parse_unmerged_record(&record) {
                entries.push(entry);
            }
            continue;
        }

        if record.starts_with("? ") {
            if let Some(path) = record.strip_prefix("? ") {
                entries.push(FileEntry {
                    path: path.to_string(),
                    status: "??".to_string(),
                });
            }
            continue;
        }

        if record.starts_with("! ") {
            if let Some(path) = record.strip_prefix("! ") {
                entries.push(FileEntry {
                    path: path.to_string(),
                    status: "!!".to_string(),
                });
            }
        }
    }

    Ok(entries)
}

fn detect_repo_root(start_dir: &Path) -> PathBuf {
    let output = Command::new("git")
        .arg("-C")
        .arg(start_dir)
        .args(["rev-parse", "--show-toplevel"])
        .output();

    let Ok(output) = output else {
        return start_dir.to_path_buf();
    };
    if !output.status.success() {
        return start_dir.to_path_buf();
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return start_dir.to_path_buf();
    }

    PathBuf::from(path)
}

fn parse_type_1_record(record: &str) -> Option<FileEntry> {
    // `1 <xy> <sub> <mH> <mI> <mW> <hH> <hI> <path>`
    let mut parts = record.splitn(9, ' ');
    let _type_ = parts.next()?;
    let status = parts.next()?.to_string();
    let path = parts.nth(6)?.to_string();
    Some(FileEntry { path, status })
}

fn parse_type_2_record(record: &str) -> Option<FileEntry> {
    // `2 <xy> <sub> <mH> <mI> <mW> <hH> <hI> <X> <score> <path> \0 <orig_path>`
    let mut parts = record.splitn(11, ' ');
    let _type_ = parts.next()?;
    let status = parts.next()?.to_string();
    let path = parts.nth(8)?.to_string();
    Some(FileEntry { path, status })
}

fn parse_unmerged_record(record: &str) -> Option<FileEntry> {
    // `u <xy> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>`
    let mut parts = record.splitn(11, ' ');
    let _type_ = parts.next()?;
    let status = parts.next()?.to_string();
    let path = parts.nth(8)?.to_string();
    Some(FileEntry { path, status })
}

fn main() {
    let app = Application::new();

    app.run(move |cx| {
        gpui_component::init(cx);
        cx.activate(true);

        cx.spawn(async move |cx| {
            cx.open_window(
                WindowOptions {
                    titlebar: Some(TitleBar::title_bar_options()),
                    ..Default::default()
                },
                |window, cx| {
                    window.set_window_title("git-viewer");
                    let view = cx.new(|cx| GitViewerApp::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
