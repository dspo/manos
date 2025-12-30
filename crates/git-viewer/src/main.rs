use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::Context as _;
use anyhow::{Result, anyhow};

mod app;
mod git;
mod model;

fn print_usage() {
    println!(
        "git-viewer {}\n\n用法：\n  git-viewer [path]\n  git-viewer --foreground [path]\n\n说明：\n  - path：要打开的目录（默认当前目录）。\n  - 若 path 在 git 仓库内，会自动定位到仓库根目录并加载状态。\n  - 默认会从终端 detach：拉起 GUI 后当前命令立即退出。\n  - 使用 --foreground 可让进程留在前台（便于调试）。\n",
        env!("CARGO_PKG_VERSION")
    );
}

#[derive(Debug, Clone)]
struct CliArgs {
    start_dir: PathBuf,
    foreground: bool,
}

fn resolve_args() -> Result<CliArgs> {
    let mut start_dir: Option<PathBuf> = None;
    let mut foreground = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("git-viewer {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--foreground" => {
                foreground = true;
            }
            _ => {
                if start_dir.is_some() {
                    print_usage();
                    return Err(anyhow!("参数过多：只支持 0 或 1 个 path 参数"));
                }
                start_dir = Some(PathBuf::from(arg));
            }
        }
    }

    let start_dir = match start_dir {
        Some(dir) => normalize_dir(dir)?,
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };

    Ok(CliArgs {
        start_dir,
        foreground,
    })
}

fn normalize_dir(input: PathBuf) -> Result<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut path = input;
    if !path.is_absolute() {
        path = cwd.join(path);
    }

    let path = path.canonicalize().unwrap_or_else(|_| path.clone());

    if path.is_file() {
        return Ok(path.parent().unwrap_or(Path::new(".")).to_path_buf());
    }

    if !path.is_dir() {
        return Err(anyhow!("路径不是目录：{}", path.display()));
    }

    Ok(path)
}

fn should_detach(foreground: bool) -> bool {
    if foreground {
        return false;
    }

    match std::env::var("GIT_VIEWER_FOREGROUND") {
        Ok(value) if !value.trim().is_empty() && value != "0" => return false,
        _ => {}
    }

    true
}

fn spawn_detached_gui(start_dir: &Path) -> Result<()> {
    let exe = std::env::current_exe().context("无法获取当前可执行文件路径")?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--foreground");
    cmd.arg(start_dir);

    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        unsafe {
            cmd.pre_exec(|| {
                unsafe extern "C" {
                    fn setsid() -> i32;
                }

                if setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }

                Ok(())
            });
        }
    }

    cmd.spawn().context("启动 GUI 子进程失败")?;
    Ok(())
}

fn main() {
    let args = match resolve_args() {
        Ok(args) => args,
        Err(err) => {
            eprintln!("{err:#}");
            std::process::exit(2);
        }
    };

    if should_detach(args.foreground) {
        match spawn_detached_gui(&args.start_dir) {
            Ok(()) => return,
            Err(err) => eprintln!("git-viewer: detach failed, falling back to foreground: {err:#}"),
        }
    }

    app::run(args.start_dir);
}
