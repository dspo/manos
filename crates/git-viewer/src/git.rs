use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context as _;
use anyhow::{Result, anyhow};

use crate::model::FileEntry;

pub(crate) fn status_xy(status: &str) -> Option<(char, char)> {
    let mut chars = status.chars();
    Some((chars.next()?, chars.next()?))
}

pub(crate) fn is_untracked_status(status: &str) -> bool {
    status == "??"
}

pub(crate) fn is_ignored_status(status: &str) -> bool {
    status == "!!"
}

pub(crate) fn is_conflict_status(status: &str) -> bool {
    status.contains('U') || status == "AA" || status == "DD"
}

pub(crate) fn is_clean_status_code(code: char) -> bool {
    code == '.' || code == ' '
}

pub(crate) fn is_modified_status_code(code: char) -> bool {
    !is_clean_status_code(code) && code != '?' && code != '!'
}

pub(crate) fn run_git<I, S>(repo_root: &Path, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .context("执行 git 命令失败")?;

    if output.status.success() {
        return Ok(());
    }

    Err(anyhow!(
        "git 命令返回非零（{}）：{}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

pub(crate) fn run_git_with_stdin<I, S>(repo_root: &Path, args: I, stdin: &str) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    use std::io::Write as _;
    use std::process::Stdio;

    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("执行 git 命令失败")?;

    if let Some(mut input) = child.stdin.take() {
        input
            .write_all(stdin.as_bytes())
            .context("写入 git stdin 失败")?;
    }

    let output = child.wait_with_output().context("等待 git 进程失败")?;
    if output.status.success() {
        return Ok(());
    }

    Err(anyhow!(
        "git 命令返回非零（{}）：{}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn delete_worktree_path(repo_root: &Path, path: &str) -> Result<()> {
    let full_path = repo_root.join(path);
    let metadata = match std::fs::symlink_metadata(&full_path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("读取文件元数据失败：{}", full_path.display()));
        }
    };

    if metadata.file_type().is_dir() {
        std::fs::remove_dir_all(&full_path)
            .with_context(|| format!("删除目录失败：{}", full_path.display()))?;
    } else {
        std::fs::remove_file(&full_path)
            .with_context(|| format!("删除文件失败：{}", full_path.display()))?;
    }

    Ok(())
}

pub(crate) fn rollback_path_on_disk(repo_root: &Path, path: &str, status: &str) -> Result<String> {
    if is_untracked_status(status) {
        delete_worktree_path(repo_root, path)?;
        return Ok(format!("已删除未跟踪文件：{path}"));
    }

    let staged_added = status_xy(status).is_some_and(|(x, _)| x == 'A');
    let looks_like_new_file = staged_added || status.contains('A');
    if looks_like_new_file {
        let _ = run_git(repo_root, ["rm", "--cached", "--", path]);
        let _ = delete_worktree_path(repo_root, path);
        return Ok(format!("已回滚新文件：{path}"));
    }

    run_git(repo_root, ["checkout", "HEAD", "--", path])
        .with_context(|| format!("执行 git checkout HEAD -- 失败：{path}"))?;
    Ok(format!("已回滚到 HEAD：{path}"))
}

pub(crate) fn reveal_path_in_file_manager(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("open");
        if path.exists() && !path.is_dir() {
            cmd.arg("-R").arg(path);
        } else if path.is_dir() {
            cmd.arg(path);
        } else {
            cmd.arg(path.parent().unwrap_or(path));
        }

        let output = cmd.output().context("执行 open 失败")?;
        if output.status.success() {
            return Ok(());
        }
        return Err(anyhow!(
            "open 返回非零（{}）：{}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    #[cfg(target_os = "windows")]
    {
        let target = if path.exists() && !path.is_dir() {
            format!("/select,{}", path.display())
        } else {
            path.parent().unwrap_or(path).display().to_string()
        };
        let output = Command::new("explorer")
            .arg(target)
            .output()
            .context("执行 explorer 失败")?;
        if output.status.success() {
            return Ok(());
        }
        return Err(anyhow!(
            "explorer 返回非零（{}）：{}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let target = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent().unwrap_or(path).to_path_buf()
        };
        let output = Command::new("xdg-open")
            .arg(target)
            .output()
            .context("执行 xdg-open 失败")?;
        if output.status.success() {
            return Ok(());
        }
        return Err(anyhow!(
            "xdg-open 返回非零（{}）：{}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
}

pub(crate) fn fetch_git_status(repo_root: &Path) -> Result<Vec<FileEntry>> {
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
            let orig_path = segments.next().and_then(|orig| {
                (!orig.is_empty()).then(|| String::from_utf8_lossy(orig).into_owned())
            });
            if let Some(entry) = parse_type_2_record(&record, orig_path) {
                entries.push(entry);
            }
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
                    orig_path: None,
                });
            }
            continue;
        }

        if record.starts_with("! ") {
            if let Some(path) = record.strip_prefix("! ") {
                entries.push(FileEntry {
                    path: path.to_string(),
                    status: "!!".to_string(),
                    orig_path: None,
                });
            }
        }
    }

    Ok(entries)
}

pub(crate) fn detect_repo_root(start_dir: &Path) -> PathBuf {
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

pub(crate) fn workspace_name_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string())
        .unwrap_or_else(|| path.display().to_string())
}

pub(crate) fn fetch_git_branch(repo_root: &Path) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .context("执行 git rev-parse 失败")?;

    if !output.status.success() {
        return Ok("No Repo".to_string());
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch.is_empty() {
        return Ok("No Repo".to_string());
    }

    if branch == "HEAD" {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .args(["rev-parse", "--short", "HEAD"])
            .output()
            .context("执行 git rev-parse --short HEAD 失败")?;

        if output.status.success() {
            let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !hash.is_empty() {
                return Ok(format!("detached@{hash}"));
            }
        }

        return Ok("DETACHED".to_string());
    }

    Ok(branch)
}

pub(crate) fn fetch_last_commit_message(repo_root: &Path) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["log", "-1", "--pretty=%B"])
        .output()
        .context("执行 git log 失败")?;

    if !output.status.success() {
        return Ok(String::new());
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_type_1_record(record: &str) -> Option<FileEntry> {
    // `1 <xy> <sub> <mH> <mI> <mW> <hH> <hI> <path>`
    let mut parts = record.splitn(9, ' ');
    let _type_ = parts.next()?;
    let status = parts.next()?.to_string();
    let path = parts.nth(6)?.to_string();
    Some(FileEntry {
        path,
        status,
        orig_path: None,
    })
}

fn parse_type_2_record(record: &str, orig_path: Option<String>) -> Option<FileEntry> {
    // `2 <xy> <sub> <mH> <mI> <mW> <hH> <hI> <X> <score> <path> \0 <orig_path>`
    let mut parts = record.splitn(11, ' ');
    let _type_ = parts.next()?;
    let status = parts.next()?.to_string();
    let path = parts.nth(8)?.to_string();
    Some(FileEntry {
        path,
        status,
        orig_path,
    })
}

fn parse_unmerged_record(record: &str) -> Option<FileEntry> {
    // `u <xy> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>`
    let mut parts = record.splitn(11, ' ');
    let _type_ = parts.next()?;
    let status = parts.next()?.to_string();
    let path = parts.nth(8)?.to_string();
    Some(FileEntry {
        path,
        status,
        orig_path: None,
    })
}
