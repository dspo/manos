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

## 实现步骤（迭代版）
1) 基础设施：RepoState + Git 服务（status/diff/merge 数据获取）；文件面板；应用骨架。
2) 二方 diff：diff 计算与 hunk 模型；Split 视图（虚拟化、gutter、滚动同步）；Inline 视图；折叠上下文；忽略空白。
3) 冲突视图：三方 merge 模型；ConflictBlock + 操作；三栏同步滚动；未解决计数与导航。
4) Git 操作：stage/unstage/revert/hunk apply；标记已解决；状态刷新。
5) 体验与性能：标尺、工具条优化；键盘命令；布局缓存与防抖；大文件基准测试。
6) 进阶：历史/任意 commit 对比；评论/AI 提示；多仓库与外部修改检测。

## TODO 迭代计划（按基础/重要/紧急优先排序）
- [ ] 仓库与状态服务：Git 状态拉取（porcelain v2）、diff/merge 基础 API、异步任务封装。
- [ ] 文本模型与数据层：rope 文档加载、行访问抽象、diff/merge 数据结构（hunk/冲突块）。
- [ ] 应用骨架：gpui 入口、全局状态（RepoState/DiffState）与事件总线 wiring。
- [ ] 文件列表 RepoFileList：状态分组/过滤、打开文件操作、行内 stage/unstage/revert。
- [ ] Diff 计算管线：忽略空白选项、二方 diff 生成 hunk + 行内 spans。
- [ ] DiffViewport + 虚拟化：窗口化渲染、行高缓存、滚动同步（双栏）。
- [ ] Gutter：双行号、变更/冲突标记、点击跳转同步。
- [ ] Split 视图渲染：左右列绑定 ScrollModel，hunk 折叠与上下文展开。
- [ ] Inline 视图渲染：单列流 + 双行号 gutter + hunk 折叠。
- [ ] HunkControls：悬浮/贴边工具条（采纳、还原、折叠），根据模式切换按钮。
- [ ] 三方 merge 计算：base/ours/theirs 生成冲突块，支持忽略空白。
- [ ] ConflictBlock 渲染：三栏对齐，采纳 ours/theirs/base/保留两侧，未解决标记。
- [ ] 冲突导航与计数：上一/下一冲突，顶部/右侧摘要提示。
- [ ] Git 操作集成：stage/unstage 文件与 hunk，revert hunk，标记已解决后写回文件。
- [ ] ScrollRuler：变更/冲突分布标尺，点击跳转与滚动同步。
- [ ] TopToolbar：模式切换、忽略空白、上下文折叠、导航、Git 操作入口。
- [ ] StatusBar：仓库/文件状态、未解决计数、光标位置、忽略空白显示。
- [ ] 键盘命令/CommandPalette（可选）：跳 hunk/冲突、采纳 ours/theirs、切换模式、折叠/展开。
- [ ] 性能与体验调优：防抖 resize/reflow、布局缓存、1-5 万行基准测试。
- [ ] 进阶能力（后续）：历史/任意 commit 对比；评论/AI 提示；多仓库；外部修改检测与重载。
