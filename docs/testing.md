# 测试策略

## 按改动选择验证

纯文档执行[文档验收](#文档验收)。Rust 实现运行适用门禁与相关测试，行为变化同步验收数据；界面、平台变化另作真实系统验证。入口尚未建立的边界见[工程说明](development.md#工程目录与使用边界)，不能将一次性检查视为正式工具已交付。

## 测试层次

| 层次 | 范围 | 工具与位置 |
|---|---|---|
| 单元测试 | 每个 crate 的内部逻辑 | 模块内 `#[cfg(test)]` |
| 属性测试 | L0 不变量：文本缓冲与偏移转换、事务与反向编辑、选择映射、增量解析与全量解析一致、节点 ID 稳定 | `proptest` |
| 一致性测试 | [声明方言](contracts/markdown-dialect.md#基础方言与扩展)的规范样例；与 `pulldown-cmark` 作语义结构差分 | `crates/moino_markdown/tests/`（规划中） |
| 向量测试 | `docs/acceptance/vectors/*.yaml` 中的文本、时序与几何样例 | `moino_testkit` 读取 YAML 生成测试 |
| 界面集成测试 | 真实组件在无头窗口中运行，模拟键盘、指针与滚动，断言状态、焦点、布局与无障碍属性 | `#[gpui_kit::test]` 与 GPUI Kit 测试工具，启用 `gpui-kit/test-support` |
| 快照测试 | 语义 JSON、HTML 导出、设置导出、诊断文本 | `insta` |
| 模糊测试 | 解析器、事务应用、容器解包、HTML 净化 | `cargo-fuzz`，在每夜 CI 运行 |
| 基准测试 | [质量目标（预留）](planned.md#quality)中的性能指标 | `divan` 或 `criterion`，结果随 CI 归档 |
| 手动验证 | 输入法、打印、字体、屏幕阅读器、高对比、文件关联、单实例、拖放 | 下方各平台检查清单 |


单元测试在模块内，跨模块测试在 `crates/<crate>/tests/`；名称表达行为和条件，非显然断言链接契约。只测公开接口，内部观察仅经 `test-support` 特性。

全部计时用受控时钟：界面推进 GPUI 测试执行器，L0 用 testkit，不真实 sleep；后台任务由测试控制完成顺序以检查过期提交。固定文档、设置、字体和内置浅色主题，显式条件遵循[向量格式](acceptance/README.md#向量字段)。

## CI 门禁

| 检查 | 触发 | 平台 |
|---|---|---|
| `cargo fmt --all --check` | 每个 PR 与 `main` | Linux |
| `cargo clippy --workspace --all-targets -- -D warnings` | 每个 PR 与 `main` | Windows、macOS、Linux |
| `cargo test --workspace --locked` | 每个 PR 与 `main` | Windows、macOS、Linux |
| `cargo deny check` | 每个 PR 与 `main` | Linux |
| `cargo xtask layers` | 每个 PR 与 `main` | 三平台 |
| `cargo xtask docs`、`cargo xtask vectors`、`cargo xtask trace` | 每个 PR 与 `main` | 三平台 |
| L0 覆盖率门槛 | 每个 PR 与 `main` | Linux |
| 模糊测试与基准测试（规划中） | 每夜 | Linux |


L0 行覆盖率不低于 80%，由 cargo llvm-cov 计算；其他 crate 只报告。CI 测试使用锁文件。`cargo xtask trace` 生成规格、场景、向量与 Rust 测试来源的关联报告作为构建产物，不提交仓库；加载与链接校验不代表产品断言执行，向量状态明确为 `not-executed`，测试关联本身不记录执行成功。标记格式和原型边界见[基础验证](acceptance/m0.md#自动检查)。

## 手动验证清单

每个里程碑结束前在 Windows、macOS、Linux 检查：

- 拼音、日文、韩文、Emoji 的候选、组合、提交与取消。
- 系统打印交接与分页 PDF；[字体角色](design/typography.md#字体角色)、中日韩后备和彩色 Emoji。
- 讲述人或 NVDA、VoiceOver、Orca 的正文、模式和错误语义；高对比与减少动画。
- 文件关联、重复启动转发，文件与图片拖放、侧栏移动。

## 专项检查

| 主题 | 检查范围与判定依据 |
|---|---|
| 编辑与模型 | [场景矩阵](acceptance/scenarios.md)与正式 YAML，增量/全量一致、CST 无损、节点身份与快照隔离 |
| 渲染与输入 | [渲染接缝](architecture.md#关键接缝)的强调、链接、原子对象命中、插入点亲和性、选择、标记和 Windows 候选框；无障碍文本暴露 |
| 专用表面 | [挂载几何](design/layout-geometry.md#编辑表面)、历史归并、提交、源码往返；各语言和别名的高亮快照 |
| 数学与图表 | [数学](modules/math.md)的基线、编号、引用、宏和失败；[图表](modules/diagrams.md)的类型、转换、主题重绘和尺寸差异 |
| 主题 | [加载与覆盖](contracts/theme.md)的校验、回退、优先级、安全模式和 Kit 投影；设计对比表复核 |
| 会话与文件 | [单实例和租约](contracts/sessions.md)、同路径定位、镜像、修订缺号；原子写入故障、冲突、恢复损坏记录、文件事件合并与正则预算 |
| HTML 与拼写 | [净化和媒体](modules/html-media.md)的数据保留、诊断、无操作时无请求；[词典](modules/spellcheck.md)建议、个人数据、哈希失败与原子安装 |
| 输出与预览 | [导出](modules/export.md)用中日韩、长表、公式、标题样本核对分页、字体、书签、搜索、体积与耗时；[PDF 预览](modules/view-modes.md#非-markdown-文件)的文字、矢量、图片与字体 |
| 界面 | [菜单](modules/menus-dialogs.md)跨平台派发、键盘导航和正文不移动；[图标](design/icons.md)几何与可访问名称；[国际化](modules/i18n.md)系统映射、用户覆盖和标题 |

每项实测按[开发交付](development.md#文档先行工作流)记录结果。实际延迟、输入法、打印驱动、字体抗锯齿、复杂图表及原生控件须实测，不用受控输入或技术方案相同替代真实一致性。

## 文档验收

检查归属是否唯一、入口是否可达、文件与锚点是否有效，正文是否直接表达当前要求，且没有头部元信息、条款代码、历史补丁或空泛模板节。根目录双语事实一致，预留范围不伪装为完整契约。

解析正式 YAML，检查名称唯一、`specs` 章节和 `scenario` 锚点有效；逐项核对语义覆盖。重排文档不改输入、操作和预期结果。数据存在或引用正确都不等于产品断言通过；发布完整门槛见[发布](release.md#发布门槛)。
