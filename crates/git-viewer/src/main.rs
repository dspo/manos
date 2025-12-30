use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

mod app;
mod git;
mod model;

fn print_usage() {
    println!(
        "git-viewer {}\n\n用法：\n  git-viewer [path]\n\n说明：\n  - path：要打开的目录（默认当前目录）。\n  - 若 path 在 git 仓库内，会自动定位到仓库根目录并加载状态。\n",
        env!("CARGO_PKG_VERSION")
    );
}

fn resolve_start_dir_from_args() -> Result<PathBuf> {
    let mut args = std::env::args().skip(1);
    let Some(arg) = args.next() else {
        return Ok(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    };

    if args.next().is_some() {
        print_usage();
        return Err(anyhow!("参数过多：只支持 0 或 1 个 path 参数"));
    }

    if arg == "-h" || arg == "--help" {
        print_usage();
        std::process::exit(0);
    }
    if arg == "-V" || arg == "--version" {
        println!("git-viewer {}", env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut path = PathBuf::from(arg);
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

fn main() {
    let start_dir = match resolve_start_dir_from_args() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("{err:#}");
            std::process::exit(2);
        }
    };

    app::run(start_dir);
}
