# 预留范围

本页登记尚无正文的主题和未决细节，不跟踪实现进度，不另设默认值。补全时改所属章节与验收，并更新入口。

## 尚无正文

| 主题 | 预期位置 | 范围 |
|---|---|---|
| <a id="quality"></a>quality | `docs/quality.md` | 性能指标、基准环境、资源预算与降级 |
| <a id="config-catalog"></a>config-catalog | `docs/config-catalog.md` | 设置键、值域、作用域和默认值的归属索引 |
| <a id="rendering"></a>rendering | `docs/contracts/rendering.md` | 渲染阶段、增量队列、派生失效、异步提交、屏障 |
| <a id="jobs"></a>jobs | `docs/contracts/jobs.md` | 任务状态、优先级、并发、缓存、取消、错误呈现 |
| <a id="persistence"></a>persistence | `docs/contracts/persistence.md` | 保存、自动保存、关闭与退出、外部冲突 |
| <a id="commands-keymap"></a>commands-keymap | `docs/contracts/commands-keymap.md` | 命令注册、命令目录、快捷键与平台映射 |
| <a id="filesystem"></a>filesystem | `docs/contracts/filesystem.md` | 文件授权、路径规范化、监视、回收站 |
| <a id="formats"></a>formats | `docs/contracts/formats.md` | 文件编码、元数据历史键、便携文档容器 |
| <a id="export-ir"></a>export-ir | `docs/contracts/export-ir.md` | 语义中间表示与语义文档 JSON |
| <a id="startup-cli"></a>startup-cli | `docs/modules/startup-cli.md` | 启动、恢复入口、命令行参数、文件关联 |
| <a id="tables"></a>tables | `docs/modules/tables.md` | 表格编辑 |
| <a id="images-links"></a>images-links | `docs/modules/images-links.md` | 图片、附件、上传、外部处理、链接 |
| <a id="clipboard"></a>clipboard | `docs/modules/clipboard.md` | 剪贴板、拖放、粘贴转换 |
| <a id="outline-stats"></a>outline-stats | `docs/modules/outline-stats.md` | 标题索引、大纲、文内目录、脚注预览、统计 |
| <a id="search"></a>search | `docs/modules/search.md` | 文档查找替换、正则、快速打开、工作区搜索 |
| <a id="workspace"></a>workspace | `docs/modules/workspace.md` | 文件库、工作区、文件树与列表、排序、文件操作 |
| <a id="recovery"></a>recovery | `docs/modules/recovery.md` | 恢复调度、恢复记录、保留与容量、恢复中心、崩溃重启 |
| <a id="import"></a>import | `docs/modules/import.md` | 导入与降级映射 |
| <a id="accessibility"></a>accessibility | `docs/modules/accessibility.md` | 键盘可达、辅助技术、对比度、减少动画 |
| <a id="components"></a>components | `docs/design/components.md` | 控件状态、边框阴影、组件状态矩阵、设置项顺序、空状态 |
| <a id="motion-timing"></a>motion-timing | `docs/design/motion-timing.md` | 时间语义、计时表、动画参数 |

## 已有正文的未决细节

| 归属 | 待明确 |
|---|---|
| <a id="partial-sessions"></a>[sessions](contracts/sessions.md) | 租约移交、残留端点恢复、错误协议 |
| <a id="partial-settings"></a>[settings](contracts/settings.md) | 文件层允许列表、迁移和交换字段 |
| <a id="partial-security"></a>[security](contracts/security.md) | 文件授权、外部处理参数、解包和解码限额 |
| <a id="partial-code-blocks"></a>[code-blocks](modules/code-blocks.md) | 完整语言目录、默认语言、信息行序列化、跨表面历史 |
| <a id="partial-math"></a>[math](modules/math.md) | 命令能力、编号过程、宏和公式预算 |
| <a id="partial-diagrams"></a>[diagrams](modules/diagrams.md) | 各类型完整语法、尺寸差异、错误状态和预算 |
| <a id="partial-export"></a>[export](modules/export.md) | IR 字段、格式映射、导入降级、资源策略、配置与状态机 |
| <a id="partial-spellcheck"></a>[spellcheck](modules/spellcheck.md) | 语言识别、时序、取消和词典管理 |
| <a id="partial-view-modes"></a>[view-modes](modules/view-modes.md) | 专注、打字机、缩放、只读和锁定 |
| <a id="partial-menus"></a>[menus](modules/menus-dialogs.md) | 顶层和上下文菜单、工具栏、通知与对话框 |
| <a id="partial-i18n"></a>[i18n](modules/i18n.md) | 双向文字、区域格式与辅助技术交叉行为 |
| <a id="partial-typography"></a>[typography](design/typography.md) | 块间距、token 值域与完整后备顺序 |
| <a id="partial-layout"></a>[layout](design/layout-geometry.md) | 窗口尺寸、区域、覆盖层与滚动几何 |
| <a id="partial-icons"></a>[icons](design/icons.md) | 各具体图标的坐标 |
| <a id="partial-editing"></a>[editing](modules/editing.md) | 零长度格式意图与标记对的关系 |

## 验收数据

<a id="acceptance"></a>场景与正式数据见[验收](acceptance/README.md)，没有数据的行为仍需补样例；名称或文件规划不表示覆盖。
