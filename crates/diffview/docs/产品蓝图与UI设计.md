# 产品蓝图与 UI 设计（Git 冲突解决 / Diff 工具）

## 目标与范围
- 目标：提供 JetBrains 级的文件变更和冲突解决体验，覆盖二方 diff、三方冲突解决、部分暂存、性能与导航能力。
- MVP：文件列表（含冲突过滤）；二方 diff（unified/split）；三方冲突视图；冲突块采纳 ours/theirs/base；上一/下一冲突导航；折叠上下文；忽略空白；hunk 级 stage/unstage/revert；同步滚动与行高；性能友好的虚拟化。
- 进阶：历史/任意 commit 对比；评论；键盘驱动命令；外部修改检测与重载；多仓库；未解决计数；辅助标尺与摘要；AI 建议（可选）。

## 信息架构与流程
- 根布局：左侧文件面板 + 顶部工具条 + 主视图 + 右侧标尺/辅助（可折叠） + 底部状态栏。
- 文件面板：按仓库显示 Modified/Conflicts/Staged/Untracked；支持过滤、分组、批量操作；点击打开主视图。
- 主视图模式：split 双栏、inline 单栏、三栏冲突（ours/base/theirs + 结果）。模式切换保持上下文（光标/滚动）。
- Git 操作流：状态 -> 选择文件 -> 选择视图 -> 逐块/逐行编辑或采纳 -> stage/unstage/revert -> 保存结果。

## UI 设计（详细）
- 顶部工具条
  - 视图模式切换：Split / Inline / Conflict（当文件有冲突时可用）。
  - Diff 选项：忽略空白、折叠上下文大小（e.g. 3/5/10 行）、显示内联差异开关。
  - 导航：上一/下一 hunk、上一/下一 冲突、跳到首/尾；快速搜索行号。
  - Git 操作：Stage/Unstage 当前文件或选定块；Revert 选中块；“标记为已解决”按钮在冲突视图激活。
- 文件列表面板
  - 过滤 tabs：All / Conflicts / Staged / Unstaged / Untracked。
  - 列：状态图标（M/A/D/U/Conflicted）、文件路径，右侧小计（例如暂存/未暂存）。
  - 快捷操作：右键或行内按钮 stage/unstage/revert 打开，双击打开主视图。
- Split 双栏 diff
  - 左/右滚动容器共享 ScrollModel，行高缓存同步。
  - 双 gutter：行号、变更类型标记（add/del/modify）、可点击跳转；冲突时显示来源徽标。
  - 行渲染：语法高亮 + diff span 着色；空白差异可隐藏；上下文折叠区域显示“展开 N 行”行。
  - 内联工具条：悬停 hunk 时显示采纳/还原/复制原/复制新。
- Inline 单栏 diff
  - 统一流渲染变更；gutter 显示旧/新行号；折叠未改动上下文；同样的 hunk 工具条。
- 三栏冲突视图
  - ours / base / theirs + 结果栏，至少显示 ours/base/theirs，结果栏可与 ours/theirs 并排或置底。
  - 冲突块头部：来源标签（Ours/Theirs/Base）、状态（未解决/已解决）、操作按钮（采纳 ours/theirs/base、保留两侧、清空为编辑）。
  - 冲突导航：右侧标尺显示冲突位置与类型；顶部显示计数。
  - 行高与滚动：三栏共享同步，折叠非冲突区域减少噪音。
- 右侧标尺/导航
  - 显示变更/冲突分布，支持点击跳转；不同颜色代表 add/del/conflict。
- 底部状态栏
  - 展示当前仓库、文件状态、未解决冲突数、忽略空白状态、光标位置。
- 键盘支持（可选）
  - 快捷键：跳 hunk/冲突、采纳 ours/theirs、切换模式、折叠/展开上下文、stage/unstage 选中。

## 组件拆解（含难度评估）
- RepoFileList（难度：中）
  - 作用：展示仓库状态、过滤、快速打开文件。
  - 设计/原因：UI 简单但需结合 git 异步状态更新、过滤、批量操作，状态一致性是主要复杂点。
- TopToolbar（难度：易）
  - 作用：模式切换、diff 选项、导航、Git 操作入口。
  - 设计/原因：主要是按钮与开关布局，逻辑简单；只需处理禁用态与事件分发。
- DiffViewport (Split/Inline)（难度：较难）
  - 作用：承载滚动区域与虚拟化。
  - 设计/原因：需共享 ScrollModel、行高缓存、窗口化渲染，确保双/三栏对齐和性能，是渲染层核心难点。
- Gutter（难度：中）
  - 作用：行号、变更/冲突标记、点击跳转。
  - 设计/原因：绘制与交互简单，但要支持双行号与类型映射、同步滚动，对齐精度要求较高。
- DiffLine（难度：中）
  - 作用：单行渲染（文本 + diff span + 背景）。
  - 设计/原因：需要融合语法高亮与 diff spans，处理忽略空白、选中态，逻辑适中。
- HunkControls（难度：易）
  - 作用：悬浮/贴边工具条，提供采纳、还原、折叠等操作。
  - 设计/原因：按钮与事件转发为主，状态依赖 hunk 元数据，交互清晰。
- InlineContextFold（难度：易）
  - 作用：折叠未变更区域，显示“展开 N 行”。
  - 设计/原因：渲染占位 + 状态存储，逻辑简单。
- ConflictBlock（难度：难）
  - 作用：三方冲突块的容器。
  - 设计/原因：需对齐 ours/base/theirs/merged，支持多种采纳策略、编辑态与已解决标记，涉及行高同步和复杂状态流转。
- ScrollRuler（难度：中）
  - 作用：右侧标尺，展示变更/冲突分布并跳转。
  - 设计/原因：绘制小块与点击跳转简单，但要与主视图同步滚动并映射比例，需要精度与性能权衡。
- StatusBar（难度：易）
  - 作用：显示仓库/文件状态、未解决计数、光标位置、忽略空白。
  - 设计/原因：信息展示为主，逻辑简单。
- CommandPalette（可选，难度：中）
  - 作用：键盘友好地执行跳转/采纳/切换模式。
  - 设计/原因：列表过滤 + 快捷键提示简单，但命令注册/上下文生效需要全局状态绑定。

## 实现步骤与 TODO 映射（闭环迭代）
### 1) 基础骨架与状态拉取
- 实现目标：跑起一个可交互的最小产品壳（窗口 + 文件变更列表），并把“状态拉取”闭环打通。
- 如何实现：
  - 在 `crates/git-viewer` 实现 gpui 二进制入口，打开窗口并挂载 `Root`。
  - 实现最小 Git 状态服务：调用 `git status --porcelain=v2 -z`，解析得到 `(status, path)` 列表。
  - 渲染只读文件列表：每行是可点击项；点击后打印日志，并尝试读取工作区文件内容（验证路径解析与 IO 流程）。
- 验收（建议用最终产品二进制验收）：
  - 运行 `cargo run -p git-viewer`，窗口能显示“发现 N 个变更文件”与文件列表。
  - 点击任一文件项：stdout 有 `[git-viewer] 点击文件: ...`，且弹出 notification（成功时显示字节数，失败时显示错误）。
- 本步 TODO：
  - [x] 应用骨架：gpui 入口、窗口、`Root` 挂载
  - [x] 仓库与状态服务（MVP）：`git status --porcelain=v2 -z` 拉取与解析
  - [x] RepoFileList（只读态）：列表渲染 + 点击日志
  - [x] 文件内容读取（MVP）：点击项触发读取并反馈（notification）

### 2) 文本模型与二方 diff 数据层
- 实现目标：把“对比文本”变成稳定的数据结构（hunks/行映射/行内 spans），不依赖 UI。
- 如何实现：
  - 将 `crates/diffview` 升级为真正的 library crate（后续 diff 组件与算法都放这里）。
  - 文本模型：用 rope（优先 `ropey`）封装 `Document`（按行访问、UTF-16/字节偏移转换按需）。
  - 二方 diff：基于 `similar`（或后续替换为 Zed diff）生成 hunks；提供“忽略空白”配置。
  - 行内差异：在每个替换/修改块内再做一次 token/字符级 diff，产出 spans（added/removed/unchanged）。
- 验收（example/测试）：
  - `crates/diffview` 增加 `examples/diff_model.rs`（或单元测试）：
    - 输入两段字符串，输出 hunk 数量、每个 hunk 的范围、以及某个修改行的 spans。
    - 打开/关闭忽略空白时，hunk 或 spans 有可观察差异。
- 本步 TODO：
  - [ ] `crates/diffview`：创建/完善为 library crate（可被 `git-viewer` 依赖）
  - [ ] 文本模型与数据层：rope 文档加载、行访问抽象
  - [ ] Diff 计算管线：二方 diff -> hunks + 行内 spans；支持忽略空白
  - [ ] 基础示例/测试：验证数据层输出稳定可复现

### 3) Split 视图 MVP（只读）
- 实现目标：实现 JetBrains 风格的 split diff “能用版”（滚动同步、gutter、hunk 展示、折叠上下文）。
- 如何实现：
  - 视图输入：接入步骤 2 的 diff 模型（hunks + 行内 spans）。
  - DiffViewport：实现窗口化渲染（按可视行范围生成元素）；维护行高缓存。
  - Split 同步滚动：左右列共享 ScrollModel 或共享 offset；确保跳转定位一致。
  - Gutter：左右行号 + 变更标记；点击 gutter/hunk header 跳转到对应位置。
  - 折叠上下文：hunk 外未变更区域折叠成占位行，可展开 N 行。
  - 忽略空白：切换后触发重新计算 diff 模型并刷新视图。
- 验收（建议用 story 或独立 demo）：
  - 在 `crates/git-viewer` 增加一个 demo 模式（内置两段示例文本）：
    - 能看到左右两列、行号、变更背景色。
    - 滚动同步；点击 gutter 跳转；折叠/展开上下文工作；切换忽略空白可刷新。
- 本步 TODO：
  - [ ] DiffViewport + 虚拟化：窗口化渲染、行高缓存
  - [ ] Split 视图渲染：左右列绑定同步滚动
  - [ ] Gutter：双行号、变更标记、点击跳转
  - [ ] HunkControls（基础）：折叠/展开上下文、跳转
  - [ ] TopToolbar（部分）：忽略空白/折叠大小开关
  - [ ] StatusBar（基础）：显示当前模式/统计

### 4) Inline 视图
- 实现目标：提供 unified/inline diff（单列流式），与 split 共享同一套数据模型与折叠逻辑。
- 如何实现：
  - 复用 DiffViewport：同样按可视窗口渲染，但行模型变成“单列合并行”。
  - Gutter：显示旧/新行号；删除行显示旧行号、新行号为空；新增行反之。
  - 与 split 共享：忽略空白、上下文折叠、hunk 导航。
- 验收：
  - 在 demo 中添加模式切换（Split/Inline），切换后布局变化正确且滚动/跳转仍可用。
- 本步 TODO：
  - [ ] Inline 视图渲染：单列流 + 双行号 gutter
  - [ ] TopToolbar：模式切换入口（Split/Inline）

### 5) 文件列表联动与基本导航
- 实现目标：从“文件列表”进入“文件 diff”，并具备 hunk 级导航与基本工具条。
- 如何实现：
  - RepoFileList：点击项加载工作区版本 vs index/HEAD（先选一个对比目标），驱动 DiffState。
  - TopToolbar：模式切换、忽略空白、折叠大小、上一/下一 hunk。
  - 状态同步：切换文件、切换模式、切换选项都能稳定刷新，不丢失必要的滚动/定位状态。
- 验收：
  - 在真实仓库运行 `git-viewer`：
    - 点击任一变更文件能看到 diff 视图。
    - 上一/下一 hunk 能跳转；忽略空白/折叠大小生效。
- 本步 TODO：
  - [ ] RepoFileList（增强）：打开文件进入 diff 视图（而非仅点击日志）
  - [ ] TopToolbar（基础）：模式切换、忽略空白、折叠大小、hunk 导航
  - [ ] HunkControls（导航）：上一/下一 hunk、跳转定位
  - [ ] StatusBar：显示当前文件/统计信息

### 6) 三方 merge 数据与 ConflictBlock（读写）
- 实现目标：冲突文件进入三方视图并完成“逐块采纳/编辑”闭环。
- 如何实现：
  - 三方 merge：基于 base/ours/theirs 生成冲突块模型（块范围、来源、默认合并结果）。
  - ConflictBlock：渲染 ours/base/theirs（+ 结果栏），保持行对齐与同步滚动。
  - 交互：采纳 ours/theirs/base、保留两侧、进入编辑；维护块的 resolved 状态。
- 验收：
  - 提供冲突样例（内置字符串或测试仓库）：
    - 冲突块可见且可导航；采纳按钮能更新结果栏；resolved 状态可见并可切换。
- 本步 TODO：
  - [ ] 三方 merge 计算：生成冲突块（含忽略空白策略预留）
  - [ ] ConflictBlock 渲染：三栏对齐 + 结果栏可编辑
  - [ ] 冲突块交互：采纳 ours/theirs/base/保留两侧 + resolved 状态
  - [ ] 视图模式：冲突文件进入 Conflict 模式

### 7) 冲突导航与摘要
- 实现目标：提供 JetBrains 风格的“冲突/变更概览与快速跳转”。
- 如何实现：
  - 计数：从冲突块模型统计 resolved/未 resolved。
  - 导航：上一/下一冲突跳转，支持键盘命令（可先做按钮）。
  - ScrollRuler：渲染冲突分布，点击跳转到对应 offset/hunk。
- 验收：
  - 冲突 demo 中：计数正确、跳转准确；解决/取消解决后计数实时更新；标尺点击定位正确。
- 本步 TODO：
  - [ ] 冲突导航与计数：上一/下一冲突 + 统计
  - [ ] ScrollRuler：冲突分布显示与点击跳转
  - [ ] StatusBar（增强）：未解决计数展示

### 8) Git 操作集成（hunk 级）
- 实现目标：把 UI 操作落到真实 git 工作流（部分暂存/还原/解决冲突后 add）。
- 如何实现：
  - hunk 级 stage/unstage：基于 diff 模型生成 patch，走 `git apply --cached`/`git apply -R` 等路径（或 libgit2 方案后定）。
  - revert：对工作区应用反向 patch。
  - 冲突解决写回：将合并结果写到工作区文件并 `git add`；刷新状态列表。
- 验收：
  - 在专用测试仓库中：
    - stage/unstage/revert 对选中 hunk 生效且 `git status`/`git diff` 匹配预期。
    - 冲突文件解决并写回后，`git status` 不再显示 unmerged。
- 本步 TODO：
  - [ ] Git 操作集成：stage/unstage/revert（文件与 hunk）
  - [ ] 冲突解决写回：保存结果 + `git add` + 状态刷新
  - [ ] RepoFileList（刷新）：操作后列表实时更新

### 9) 体验与性能优化
- 实现目标：大文件/大 diff 场景可用，交互顺滑，细节贴近 JetBrains。
- 如何实现：
  - 防抖：resize/reflow、diff 重算；缓存：行高/布局/渲染片段。
  - 错误提示：git 命令失败、patch 应用失败、文件 IO 失败给出可理解提示。
  - UI 微调：标尺、工具条、hover/选中态、空白渲染一致性。
- 验收：
  - 1-5 万行文件 diff：滚动与跳转无明显卡顿；内存/CPU 不爆炸（主观体验 + 简单计时日志）。
- 本步 TODO：
  - [ ] 性能与体验调优：防抖、缓存、基准样例
  - [ ] TopToolbar/StatusBar 细节：一致的禁用态/提示
  - [ ] ScrollRuler（优化）：交互与绘制性能

### 10) 进阶功能（可选）
- 实现目标：补齐“JetBrains 级”的非核心但高价值能力。
- 如何实现：按需逐项引入（历史对比、评论、AI、多仓库、外部修改检测、CommandPalette）。
- 验收：每项能力提供独立 demo/story 或在 `git-viewer` 中提供可切换开关，并附带最小场景说明。
- 本步 TODO：
  - [ ] 历史/任意 commit 对比
  - [ ] 评论/备注（可选）
  - [ ] 多仓库与外部修改检测
  - [ ] 键盘命令/CommandPalette
