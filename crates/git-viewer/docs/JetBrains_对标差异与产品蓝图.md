# git-viewer 对标 JetBrains：差异梳理与产品蓝图

本文聚焦：JetBrains IDE（IntelliJ IDEA / GoLand / RustRover 等）的 Git 工具在 **diff 视图、冲突视图** 以及围绕它们的 **编辑/暂存/提交交互**；并给出本项目 `git-viewer` 的产品蓝图与迭代策略（工程与技术路径）。

> 术语说明：本文中的 “JetBrains” 默认指 JetBrains IDE 内置 Git 工具（Local Changes / Commit / Diff Viewer / Merge Tool）。

---

## 1. 现状概览：git-viewer 已具备什么

以当前仓库实现为准（`crates/git-viewer`）：

- **Changes 树 + Commit 面板布局**（JetBrains 类布局）：
  - 左上：Changes 树（分组、折叠、过滤、复选框选择提交文件，含目录三态勾选）。
  - Changes 树支持右键菜单：打开 diff、Stage/Unstage 文件、Rollback/删除未跟踪文件、在文件管理器中显示、复制路径等常用操作。
  - rename/copy 文件：树节点显示 “← 原路径” 提示；文件级 Stage/Unstage/Commit 会同时携带 old/new pathspec，避免只暂存一半路径导致提交不完整。
  - 左下：Commit message 多行输入 + Commit / Commit&Push（并支持左侧面板与分割线拖拽调整）。
- **Diff 视图**：
  - Split / Inline（unified）切换。
  - Split 支持 “对齐/分栏” 两种布局；支持同步滚动、折叠上下文、忽略空白、hunk 导航。
  - 支持行/范围选择（Shift 可跨 hunk）与右键上下文菜单（行/选中/该 hunk：stage/unstage/revert）；hunk header 提供快捷按钮。
  - 工具条 More 菜单提供文件级 Rollback、路径复制、在文件管理器中显示等便捷操作。
  - rename/copy 文件：Diff 读取 old/new 不同路径并正常展示；hunk/行级 stage/unstage/revert 暂对 rename/copy 禁用（需单独的 rename-aware patch 生成与 apply 语义）。
  - Diff 内查找：工具条查找栏（`Cmd/Ctrl+Shift+F`），支持上一处/下一处匹配与行高亮。
  - 可选底部“工作区编辑器”面板（仅当对比目标包含工作区）：按文件类型语法高亮，支持应用预览/从磁盘重载/保存到文件并刷新；`Alt+A` 应用、`Cmd/Ctrl+S` 保存。
  - 支持对比目标：`HEAD↔工作区 / 暂存↔工作区 / HEAD↔暂存`，以及任意 refs（输入 ref 或历史选择 commit）。
- **冲突视图**：
  - 解析 `<<<<<<< / ======= / >>>>>>>`（含 diff3 `|||||||` base）。
  - Ours/Base/Theirs 分栏显示；冲突块采纳（ours/theirs/base/both）与上一/下一冲突导航。
  - 结果编辑器（底部）按文件类型启用语法高亮，可拖拽调整高度；编辑并“应用”刷新冲突检测；冲突清零后可保存到文件或保存并 `git add`。
- **Git 操作（基于系统 git CLI）**：
  - file 级 stage/unstage；hunk 级 stage/unstage/revert（基于 unified patch + `git apply (--cached/-R)`）。
  - commit 支持仅提交勾选文件（`git commit --only -- <paths>`），untracked 自动 `git add`。
- **性能与体验**：
  - IO/diff 计算放到 `background_executor`；打开 diff 时双侧版本读取并行化，降低等待。
  - diff 重算防抖；虚拟列表 item size 缓存。
  - **diff 结果 LRU 缓存（不绑定 Split/Inline）+ 邻近文件预取**，降低切换文件等待。
  - 可选调试：设置 `GIT_VIEWER_TIMING=1`，Diff 底部状态栏显示最近一次加载耗时（io/diff/total）。
- **键盘与命令面板**：
  - `Cmd/Ctrl+K` / `Cmd/Ctrl+Shift+P` Command Palette，覆盖常用操作（导航、切换视图/布局/空白、刷新、历史对比、冲突保存等）。

---

## 2. JetBrains Git 工具的“关键交互模型”（我们要对标的核心体验）

### 2.1 Local Changes（变更列表）核心交互
- **树形 Changes**：文件夹可折叠；文件/文件夹均支持三态勾选（未选/半选/全选）。
- **Staging Area（可选）**：有些 JetBrains 版本/设置提供 “staged/unstaged” 分区（或按变更列表/changelist 管理）。
- **预览即 Diff**：选中一个文件即可在主区预览 diff（无需显式打开）。
- **性能与反馈**：切换文件基本瞬时；大 diff 也可滚动流畅；选择、展开/折叠不卡顿。

### 2.2 Diff Viewer 核心交互
- **Split/Unified** 任意切换且不丢上下文（滚动位置、选中 hunk）。
- **导航**：Prev/Next change、Prev/Next file，且“变更”包括 hunk、冲突、差异块。
- **精细高亮**：行级 + 词级（intra-line）差异；语法高亮；忽略空白策略完善。
- **编辑与应用**：
  - 常见场景是直接编辑右侧（工作区）并即时看到 diff 更新（或至少能快速刷新）。
  - 冲突解决器中编辑 result + 一键应用。
- **部分暂存（精细度）**：
  - JetBrains 允许按 **hunk/行** 级别 stage/unstage（甚至在 UI 上“点选行”）。

### 2.3 Merge/Conflict Tool 核心交互
- **三方合并**：左（ours）、右（theirs）、中（result），可选 base。
- **冲突导航与列表**：冲突块列表、计数、跳转；每块提供 accept left/right/both/resolve。
- **结果编辑体验**：结果编辑器是“主角”，支持语法高亮、缩进、撤销、搜索等编辑能力。
- **保存闭环**：Apply → Mark resolved → 保存/关闭，且与 Git 状态联动。

---

## 3. 与 JetBrains 的差异清单（能力 + 交互）

下面按“用户感知优先级”列差异，并标注建议优先级（P0/P1/P2）。

### 3.1 Changes/Commit 区差异

**P0：切换文件的整体体验**
- JetBrains：选中文件几乎瞬时展示 diff；大仓库也尽量不阻塞 UI。
- git-viewer：已做缓存与预取，但仍可能受 `git show` 进程开销、文本/行构建等影响；缺少更细粒度的渐进渲染与占位策略（例如先展示上次缓存、再后台刷新）。

**P1：Staging Area 的交互一致性**
- JetBrains：常见模式是 staged/unstaged 清晰分区（或 changelist 体系），并有“从 diff 内直接 stage/unstage 选中部分”的闭环。
- git-viewer：已支持对比目标与 stage/unstage，但 staged/unstaged 的信息结构与 JetBrains 仍不一致（例如 staged/unstaged 并列视图、按区域操作的可发现性）。

**P2：changelist / shelving / patch**
- JetBrains：支持 changelist、shelf、生成 patch 等工程化流。
- git-viewer：当前不覆盖（可作为后续扩展，不必进入核心对标范围）。

### 3.2 Diff Viewer 差异

**P0：语法高亮 + 编辑**
- JetBrains：diff 中显示的代码基本等同编辑器体验（主题、语法、选择、搜索）；可直接编辑结果（至少在冲突解决器里）。
- git-viewer：当前 diff 行是自绘文本段（diff spans），**尚未接入语法高亮管线**；普通 diff 视图基本只读，缺少“在 diff 中编辑工作区版本并即时更新”的体验。

**P0：行/词级差异质量**
- JetBrains：intra-line diff 在复杂修改中更稳定、更贴近代码语义（配合语法高亮与空白策略）。
- git-viewer：已有 segment 高亮，但算法/样式可能更朴素；空白策略的颗粒度也较少（例如仅 ignore_whitespace bool）。

**P1：部分暂存精细度（到行级）**
- JetBrains：可 stage/unstage hunk/行，且交互非常直接（按钮、快捷键、上下文菜单）。
- git-viewer：已支持 **hunk 级 + 行/范围级（按行）** 的 stage/unstage/revert（基于“选中 → 生成 patch → `git apply (--cached/-R)`”）；Shift 范围选择已支持跨 hunk；并补齐了 diff 行/标题右键菜单与 hunk header 快捷按钮；仍缺更完整的快捷键体系与更小的 patch/context 优化（尤其是极小 selection 的鲁棒性与自动扩 context）。

**P1：Diff 内搜索/跳转**
- JetBrains：在 diff 中搜索、跳转到行号、跳转到定义/文件等非常自然。
- git-viewer：已支持 diff 内 find（含高亮与上一/下一处）；仍缺行号跳转、符号级导航、打开到外部编辑器等。

### 3.3 冲突解决器差异

**P0：三方合并“结果编辑器”的编辑能力**
- JetBrains：结果编辑器具备编辑器级能力（撤销树、搜索、语法、缩进、代码格式化、选择/多光标等）。
- git-viewer：结果编辑器已能编辑，但功能仍偏轻量（目前依赖 `InputState::code_editor` 的能力上限）；缺少更丰富的编辑/导航/搜索动作与快捷键。

**P1：冲突块的“差异可视化”与“可操作性”**
- JetBrains：冲突块内不仅能 accept，还能更细致地按差异块操作，并清晰标注每侧变化。
- git-viewer：已有 ours/theirs/base 展示与采纳按钮，但缺少更细粒度的块级交互（如逐差异块 accept/撤销、冲突块列表）。

---

## 4. git-viewer 产品蓝图（对标 JetBrains 的“最小闭环”）

### 4.1 产品定位
- 目标：以 gpui/gpui-component 为基础，打造 JetBrains 风格的 **diff + 冲突解决工具**，重点在交互、性能、可编辑性与 Git 语义一致性。
- 非目标（短期）：完整替代 JetBrains 的 Git Log/Branch/Rebase/Cherry-pick 等全套能力（可作为远期扩展）。

### 4.2 核心用户流程（必须做到“顺滑”）
1. 打开仓库 → 看到 changes 树（可筛选/展开/勾选）  
2. 选中文件 → 主区立即展示 diff（Split/Unified 切换不丢上下文）  
3. 在 diff 中进行 stage/unstage/revert（至少 hunk 级，逐步到行级）  
4. 遇到冲突 → 冲突视图解决 → 保存并标记 resolved（`git add`）  
5. 输入 commit message → commit（仅提交勾选文件）→ 状态刷新一致

### 4.3 能力分层（架构与模块）
- **GitBackend（CLI 优先）**
  - status / show（HEAD/index/ref/worktree）/ apply patch / add / reset / commit / push
  - 目标：语义对齐 Git CLI，减少边界差异。
- **DiffEngine（本地算法层）**
  - 二方 diff：hunks + line mapping + intra-line segments
  - 三方：marker 解析 → 冲突块模型 → result 合并策略
- **Diff/Conflict ViewModel（可缓存）**
  - 负责把文本/模型变成“可渲染 rows”，并支持快速重建（例如 Split/Inline 切换只重建 rows）。
- **UI Components（gpui-component 风格）**
  - ChangesTree（含三态 checkbox、虚拟列表、快速过滤）
  - DiffToolbar / ConflictToolbar（icon-only + tooltip）
  - DiffVirtualListRow（支持 split/inline 两套 row renderer）
  - ConflictBlockRow（ours/base/theirs/result）
  - CommitPanel（multi-line + info tooltip + actions）

---

## 5. 迭代技术路线（工程策略）

### 5.1 原则
- **先语义闭环，再体验打磨**：stage/unstage/revert/commit 语义必须正确；UI 再逐步逼近 JetBrains。
- **缓存优先、异步优先**：任何可能卡 UI 的工作（git/io/diff/layout）都放后台；主线程只做渲染与状态切换。
- **可回退的优化**：每次性能优化都要可开关、可观测（统计耗时/命中率），避免“优化把逻辑搞复杂”。

### 5.2 迭代策略（从 P0 到 P2）

**阶段 A（P0）：可编辑性与高亮基础**
- 目标：
  - 冲突结果编辑器做到“编辑器感”（至少：搜索、撤销、常用快捷键、较好的字体/行高/缩进体验）。
  - diff 视图接入语法高亮（可先只对右侧工作区 / result editor 做）。
- 技术建议：
  - 调研/复用 Zed 的高亮与 buffer 相关抽象（若可抄，优先抄）。
  - 将 diff 的“文本渲染”与“高亮 spans”解耦：diff spans 叠加在语法高亮之上。

**阶段 B（P0→P1）：行级/范围级操作（部分暂存/撤销）**
- 目标：
  - 在 hunk 内支持选中行/块 → stage/unstage/revert（生成最小 patch）。
  - 在冲突块内支持更细粒度 accept（按差异块、甚至按行）。
- 技术建议：
  - 先实现“行级 patch 生成器”（把 row selection 映射回 unified patch 的上下文）。
  - 继续沿用 `git apply (--cached/-R)`，保证与 git 行为对齐。

**阶段 C（P1）：Diff 内搜索/导航/定位**
- 目标：
  - diff 内 find（类似 JetBrains 的 diff search），支持 next/prev match。
  - 行号跳转、快速打开到外部编辑器/路径复制。
- 技术建议：
  - 基于 rows 构建“可搜索文本视图索引”（只索引可见/展开部分也可）。

**阶段 D（P2）：更完整的 Git ToolWindow（可选）**
- staged/unstaged 分区、changelist、shelf、patch、log 等（按需扩展）。

### 5.3 验收与基线（建议建立“对标仓库”）
- 构造一个专门的 demo repo，包含：
  - 大文件（1–5 万行）、多 hunks、复杂词级修改
  - 多种冲突形态（diff3/非 diff3，插入/删除交错，双侧同一行改动）
  - rename/copy/delete（即使暂不支持，也要表现稳定）
- 每个阶段都用该 repo 做“体验回归”：
  - 文件切换耗时、滚动 FPS、操作正确性（stage/unstage/revert/commit）与错误提示一致性。

---

## 6. 建议的近期优先级（下一步该做什么）

结合对标差异，建议近期优先做：
1. **冲突结果编辑器增强（P0）**：搜索/撤销/快捷键/更像编辑器的体验（这决定“冲突解决工具”的核心质感）。
2. **diff 视图语法高亮（P0）**：至少右侧工作区/结果编辑器先高亮；然后把 diff spans 叠加到高亮上。
3. **行级 stage/unstage（P1）**：从 JetBrains 的“精细操作”开始补齐最具差异化的能力。
