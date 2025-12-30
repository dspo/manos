## git-viewer 自定义字体

`git-viewer` 启动时会尝试从以下位置加载字体文件（`.ttf` / `.otf` / `.ttc`）：

1. 环境变量 `GIT_VIEWER_FONTS_DIR` 指向的目录（最高优先级）
2. 本目录（便于开发阶段 `cargo run`，仅 debug 构建启用）
3. 可执行文件同级目录下的 `fonts/`（便于二进制分发：把字体文件与 `git-viewer` 放在一起）

另外：编译时会把本目录下的字体**嵌入到二进制**（见 `crates/git-viewer/build.rs`），这样你可以把字体文件放在这里并重新构建，然后分发一个单独的 `git-viewer` 二进制文件（注意字体许可）。

加载字体后，可以通过环境变量指定要使用的字体族：

- `GIT_VIEWER_FONT_FAMILY`：UI 字体
- `GIT_VIEWER_MONO_FONT_FAMILY`：等宽字体（diff/代码区域）

提示：字体族名称需要与字体文件内部声明的 family 一致（不一定等于文件名）。
