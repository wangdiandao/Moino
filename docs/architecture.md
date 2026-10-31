# 架构

## 分层与 crate

文档核心保持纯 Rust，能独立运行无窗口测试、属性测试与模糊测试。每个 crate 只负责下表职责，依赖只能向下；同层依赖也必须在允许列表内，禁止循环。只有 L1 及以上可依赖 GPUI Kit，L0 边界只传标准库或本表核心 crate 定义的类型。

| crate | 层 | 职责 | 允许的内部依赖 |
|---|---|---|---|
| `moino_jobs` | L0 | 任务身份、取消令牌、期限、优先级、时钟抽象、防抖与节流 | 无 |
| `moino_text` | L0 | 文本缓冲（ropey）、标量 / 字节 / UTF-16 偏移、字素与词边界、换行规范化 | 无 |
| `moino_markdown` | L0 | 无损增量 CST 解析器、方言扩展、标记区间、锚点算法、引用标签规范化 | `moino_text` |
| `moino_document` | L0 | 文档记录、块树与节点 ID、事务、选择与映射、历史、脏状态 | `moino_text`、`moino_markdown` |
| `moino_editing` | L0 | 编辑规则：Enter / Backspace / Tab、交互语法转换、格式命令、自动配对、表格结构命令 | `moino_text`、`moino_markdown`、`moino_document` |
| `moino_search` | L0 | 文档查找替换、正则预算、快速打开排序、工作区内容搜索 | `moino_text`、`moino_jobs` |
| `moino_io` | L0 | 编码检测、BOM / 换行 / 最终换行、原子保存、磁盘基线、文件监视、便携文档容器 | `moino_text`、`moino_jobs` |
| `moino_settings` | L0 | 设置 schema、作用域分层、JSONC 持久化、校验、迁移 | `moino_io` |
| `moino_recovery` | L0 | 恢复调度、恢复记录、保留与容量 | `moino_io`、`moino_document`、`moino_jobs` |
| `moino_html` | L0 | HTML 解析、安全子集净化、受控 HTML 模型 | 无 |
| `moino_math` | L0 | TeX 数学（RaTeX）解析与布局、编号与引用、宏与预算 | `moino_jobs` |
| `moino_diagram` | L0 | Mermaid（merman）渲染为 SVG、`sequence` / `flow` 语法转换 | `moino_jobs` |
| `moino_spell` | L0 | 拼写检查服务（spellbook）、词典管理、个人词典 | `moino_text`、`moino_jobs` |
| `moino_preview` | L0 | 文件类型识别，图片 / 纯文本 / PDF 页面（hayro）解码 | `moino_io`、`moino_jobs` |
| `moino_export` | L0 | 语义中间表示、各格式写出器与导入器、Typst 排版 | `moino_text`、`moino_markdown`、`moino_document`、`moino_html`、`moino_math`、`moino_diagram`、`moino_io`、`moino_jobs` |
| `moino_platform` | L0 | 操作系统适配：单实例 IPC、打印交接、文件关联、系统偏好探测 | 无 |
| `moino_theme` | L1 | 主题注册表、主题文件 schema、外观模式、覆盖、高对比、向 GPUI Kit 的主题投影 | `moino_settings` |
| `moino_ui` | L2 | 基于 GPUI Kit 的共享界面部件、图标、通知队列、对话框规则 | `moino_theme` |
| `moino_editor` | L2 | 即时预览表面：块投影、输入与输入法、选择绘制、标记展开、虚拟化、专用表面、源码模式 | `moino_theme`、`moino_ui`、`moino_text`、`moino_markdown`、`moino_document`、`moino_editing`、`moino_html`、`moino_math`、`moino_diagram`、`moino_jobs`、`moino_spell` |
| `moino_workspace` | L3 | 窗口与工作区：侧栏、文件树与列表、大纲、快速打开、查找栏、应用菜单、设置页面、恢复中心、预览、会话管理 | 上述 L0–L2 中除 `moino_editing` 以外的全部 crate |
| `moino` | L4 | 可执行文件：启动、命令行参数、单实例转发、菜单与快捷键装配 | `moino_workspace`、`moino_settings`、`moino_theme`、`moino_platform` |
| `moino_testkit` | 开发 | 向量加载、受控时钟、测试夹具 | `moino_text`、`moino_markdown`、`moino_document`、`moino_editing`、`moino_jobs` |


`moino_testkit` 仅作 dev-dependency，不进入发布构建。目录与命名见[编码规范](coding-standards.md#命名)，分层检查见[测试策略](testing.md#ci-门禁)。新增块、命令、设置、主题 token、格式、语言和图表通过注册表接入，步骤见[扩展指南](cookbook/README.md)。

## GPUI Kit 使用边界

只通过 `gpui-kit` 门面及其再导出使用 GPUI，不直接依赖 `gpui-pre`。使用默认上游后端，启用 `component` 与 `syntect`，关闭 `assets` 和 `gpui-fast`；精确版本见[依赖基线](architecture.md#依赖基线)，升级与上游补丁见[开发流程](development.md#依赖与上游)。

标准界面优先复用 Kit 的 `Root`、`TitleBar`、`AppMenuBar`、`NativeMenu`、`PopupMenu`、`Sidebar`、`Tree`、`VirtualList`、`Popover`、`Tooltip`、`Notification`、`Dialog`、`Resizable`、`Scrollbar`、`Form`、`Settings`、`Input`、`Select`、`Switch`、`Kbd`、`Diff`、`Editor`、`TextView`。只有缺少行为或不能表达规定几何时才在 `moino_ui` 自建，并在 PR 说明理由。产品任何位置都不嵌入网页引擎。

## 关键接缝

`moino_editor` 将文档块与内联 token 投影为 `BlockNode`，交给 `gpui_base::text::editor_host` 排字、换行和布局行内对象，取回 `LeafGeometry` 做命中、插入点及输入法候选框定位。叶键由 `NodeId` 派生，叶内 UTF-8 字节与[文档标量位置](contracts/document-model.md#文本与位置)显式转换；行内数学和图片等原子对象占一个 U+FFFC。

Moino 拥有文档身份、修订、历史、选择、焦点、无障碍、滚动及原生输入；插入点通过装饰区间绘制，输入法实现 `EntityInputHandler`。可见区虚拟化、滚动锚点、投影和几何访问封装在渲染接缝内，Kit 只接收投影结果。专用表面见[代码块](modules/code-blocks.md#编辑表面)，源码模式见[显示模式](modules/view-modes.md#源码模式)，主题接缝见[主题投影](contracts/theme.md#投影到-gpui-kit)。

菜单、快捷键、按钮、上下文菜单、文件事件与自动任务都调用同一个业务命令或服务。注册项包含命令 ID、本地化标签、作用域、启用条件、历史策略及默认快捷键。只读与不可用原因按命令定义显示，不悄悄隐藏入口。命令 ID 与 GPUI action 一一对应，具体命名见[编码规范](coding-standards.md#命名)；命令目录和快捷键完整规则仍见[预留范围](planned.md#commands-keymap)。

单实例 IPC、打印交接、文件关联与系统偏好探测仅由 `moino_platform` 提供安全接口。设置、恢复、缓存、日志、临时导出分别使用平台标准配置、数据、缓存和临时目录下的产品数据目录，不写安装目录。

## 运行时模型

进程、文档会话与窗口关系见[会话](contracts/sessions.md)。只使用 GPUI 前台、后台执行器，禁止直接创建 tokio 运行时或线程池；HTTP 使用 Kit 客户端。

主线程负责输入、事务提交、布局和绘制，不做阻塞 I/O、全文解析、搜索、数学图表布局、导出、哈希与解码。单批派生工作连续占用不得超过 8 ms，超出时在事务边界让出。

异步工作携带所有者、身份、取消令牌与期限，提交前重新校验文档、世代、修订及适用的主题版本。期限只在前台且事件循环可运行时有效；阻塞、休眠或调度延迟不能被当作计时参数变化，恢复后先校验身份。L0 只经 `moino_jobs` 的时钟与调度抽象使用时间，不直接调用系统时钟或 sleep；上层适配 GPUI 执行器，完整任务状态见[预留范围](planned.md#jobs)。

启动顺序为 `gpui_kit::application()` → 读取有效设置 → `gpui_kit::init` 与主题投影 → 注册命令快捷键 → 单实例监听 → 检查恢复记录 → `gpui_kit::open_window`。不直接构造 `Root`；关闭流程仍需在[持久化](planned.md#persistence)补全。

## 错误模型

跨 crate 错误携带稳定点分错误码、本地化消息键、可重试标志和必要细节；不向用户展示底层异常对象或堆栈。错误类型写法见[编码规范](coding-standards.md#错误处理)。可选渲染、转换、导出、拼写和上传失败时，普通文本编辑与保存保持可用；显示具体原因，不移除入口或显示伪成功。

| 失败 | 处理 |
|---|---|
| 不完整 Markdown | 按[方言](contracts/markdown-dialect.md#cst-与标记区间)保留文本，不弹阻塞错误 |
| 数学或图表 | 行列诊断、过期标记或原文本，不删除源内容 |
| 渲染依赖 | 允许本地重试，回退文本 |
| 保存 | 常驻通知提供重试、另存、复制；保留脏状态和恢复点 |
| 导出 | 说明阶段与原因，保留旧目标，不修改文档 |
| 文件冲突 | 三方比较或显式选择，同时保留当前文本与磁盘候选 |
| 模型不一致 | 按[文档保护](contracts/document-model.md#文档不变量)处理 |
| 超大资源 | 有限降级或拒绝并说明，不截断后冒充成功 |

## 依赖基线

具体版本在首次引入时固定于工作区；声明与变更流程见[开发流程](development.md#依赖与上游)，许可见[参考资料](references.md#许可边界)。

| 用途 | crate（引入时固定版本） | 所在 crate |
|---|---|---|
| UI 框架 | `gpui-kit =0.8.1`（固定 `gpui-pre =0.3.10`，含 GPUI、`gpui-base`、`gpui-component`） | L1 以上 |
| 文本缓冲 | `ropey 2.0.0-beta.1`（与 GPUI Kit 相同） | `moino_text` |
| 数学 | `ratex-parser` / `ratex-layout` / `ratex-svg 0.1.15` | `moino_math` |
| 图表 | `merman 0.9.2` | `moino_diagram` |
| 代码高亮 | GPUI Kit `syntect` 特性 | `moino_editor` |
| 分页导出 | `typst` / `typst-pdf` / `typst-render 0.15.1` | `moino_export` |
| 文档格式 | `docx-rs 0.4.22`、`epub-builder 0.8.3`、`zip` | `moino_export`、`moino_io` |
| PDF 预览 | `hayro 0.8.0` | `moino_preview` |
| 拼写 | `spellbook 0.4.2` | `moino_spell` |
| 正则 | `fancy-regex 0.20` | `moino_search` |
| YAML | `saphyr 0.3.3` | `moino_markdown` |
| 编码 | `encoding_rs`、`chardetng` | `moino_io` |
| 文件监视 / 回收站 | `notify`（GPUI Kit 同版本）、`trash` | `moino_io` |
| 单实例 IPC | `interprocess 2.4.7` | `moino_platform` |
| 凭据 | `keyring 4.2.0` | `moino_workspace` |
| 图片 | `image`、`resvg`（GPUI Kit 同版本） | `moino_preview`、`moino_editor` |
| 本地化 | `rust-i18n`（GPUI Kit 同版本） | L2 以上 |
| 日志 | `tracing` | 全部 |
| 差分测试 | `pulldown-cmark`（仅 dev-dependency） | `moino_markdown` |


## 基础设施

普通文件采用同目录临时写入、刷新到持久介质、校验后原子替换，不使用数据库。恢复记录每条一个目录，包含带校验和与完成标记的 `record.json`、正文及附件，损坏记录隔离。

文件选择使用 GPUI 原生对话框；notify 事件合并 200 ms 后重新 stat。回收优先使用 trash，不可用时转应用回收区。编码使用 encoding_rs 与 chardetng，YAML 使用 saphyr 安全子集；容器解包和图片解码按[安全边界](contracts/security.md)实施检查与预算。

fancy-regex 搜索在可取消任务中运行，支持前后查找，预算为 500 ms。凭据归系统凭据存储，本地化资源见[国际化](modules/i18n.md)，日志见[编码规范](coding-standards.md#日志与隐私)。
