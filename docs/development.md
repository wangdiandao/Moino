# 开发流程

## 工程目录与使用边界

工程使用 Cargo 工作区，按里程碑引入有实际实现的 crate，不预建其余模块。M0 的技术验证使用独立 example 入口，不作为正式编辑器发布；运行与证据要求见[基础验证](acceptance/m0.md)。文档任务不顺带初始化工程。

| 位置 | 职责 |
|---|---|
| `docs/` | [规格与工程文档](README.md) |
| `crates/` | [分层 Rust crate](architecture.md#分层与-crate) |
| `xtask/` | Rust 编写的分层、文档、向量和追溯检查，不另维护多语言工具体系 |
| `assets/` | 字体与[发布资源](release.md#交付物) |
| `.github/` | GitHub Actions、任务和 [PR 模板](../.github/pull_request_template.md) |

目标仓库公开托管于 GitHub，主要由编码代理实现、用户审查；Windows 优先开发与手动验证，CI 从工程建立起覆盖三平台。

## 环境准备

| 平台 | 前置工具 |
|---|---|
| 全部 | rustup、Git、GitHub CLI；固定工具链见[编码规范](coding-standards.md#工具链与工作区) |
| Windows | Visual Studio 2022 Build Tools，MSVC 和 Windows SDK |
| macOS | Xcode Command Line Tools |
| Ubuntu 24.04 | gcc、g++、clang、libfontconfig-dev、libwayland-dev、libxkbcommon-x11-dev、libx11-xcb-dev、libssl-dev、libzstd-dev、libasound2-dev、vulkan-validationlayers、libvulkan1 |

专项工具包括 cargo-deny、cargo-llvm-cov、需要 nightly 的 cargo-fuzz，以及打包用 cargo-packager；按实际检查安装。代理资料查阅遵循[根约定](../AGENTS.md#api-资料)。

## 文档先行工作流

先核对任务范围与归属正文。行为、接口、默认值、尺寸、时序或失败处理变化时，先改正文，再同步场景、精确数据和引用；已有定义充分就直接引用。取舍的结论与理由融入原章节，未决边界明确说明，不自行填默认值。

不得借“优化”隐式改变历史分组、窄屏布局、导出默认值或主题字体；过期结果覆盖、数据丢失和不安全执行是应修复的缺陷，不为复现而保留。

随后实现并执行[必要验证](testing.md#按改动选择验证)。实现和验证进度只在 GitHub Issues、Projects 与 PR 跟踪；文档不记录完成度。报告实际命令、平台、提交、结果与未验证部分，不把设计、模拟或未运行内容写成通过。

## 任务与 Issue

任务约等于一个 PR，Issue 包含目标、归属章节、范围、验收场景、同步文档和排除项，使用规划中的 `.github/ISSUE_TEMPLATE/task.yml`。工作顺序见[路线图](roadmap.md)，开始前确认依赖已合并，完成时 PR 用 `Closes #N` 关联。

## 分支、提交与 PR

从 `main` 建 `<type>/<area>-<slug>` 短分支；type 为 feat、fix、docs、refactor、test、chore、spike，area 为小写职责或 crate。提交与 Issue、PR 标题用英文 Conventional Commits，scope 小写，正文中文。

PR 包含目的、Issue、归属章节、文档变化、实际验证、未验证项及公共 API 的签名、用途；破坏性变更附 diff。行为与验收同 PR 更新，代码按规范整理后提交。经审查、CI 后 squash 合并，`main` 保护线性历史；单维护者不要求额外审批人数。

审查先说明目的范围，再判断需求方案，最后列具体问题、已做验证及局限。范围外问题另记 Issue，不混入当前改动。

## 常用命令

工作区建立后使用 `cargo run -p moino` 启动，`cargo check --workspace --all-targets` 类型检查；统一提交门禁及规划中的 xtask 入口见[CI 门禁](testing.md#ci-门禁)，不在多处复制命令清单。

## 依赖与上游

依赖只在根 `[workspace.dependencies]` 声明版本，成员用 `dep.workspace = true`。先确认现有依赖无法满足，核对维护、平台、体积和[许可证](references.md#许可边界)，PR 说明用途；改变架构先改职责边界。优先与 Kit 对齐，不使用 git 依赖。

GPUI Kit 升级单独 PR：修改精确版本和锁文件，核对发布说明及 API，修正后执行全部自动测试与三平台手动检查，列出受影响 API 和行为。能力缺口先提上游 Issue 或 PR；只有阻塞开发时才允许 `[patch.crates-io]` 指向 fork，并用跟踪 Issue 写清移除条件，上游合并后移除。
