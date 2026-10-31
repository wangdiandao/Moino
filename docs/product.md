# 产品

Moino（墨）是跨平台、高性能、轻量、本地优先的 Markdown 编辑器，以 Typora 的即时预览体验为对标。用户直接编辑排版后的正文，完整源码模式提供另一种编辑入口；具体交互见[编辑](modules/editing.md)、[选择与标记展开](modules/selection.md)和[显示模式](modules/view-modes.md)。

## 使用范围

每个窗口只有一个活动文档，不提供标签页；允许多窗口，最近文件和快速打开承担文档切换。

| 能力 | 内容与归属 |
|---|---|
| 文档 | 本地新建、打开、保存、另存、自动保存、恢复、外部冲突和镜像窗口；[会话](contracts/sessions.md)，其余见[预留主题](planned.md) |
| 编辑 | Markdown 块与内联、完整撤销重做、输入法；[编辑](modules/editing.md)、[历史](contracts/history.md) |
| 富内容 | 表格、代码、数学、图表、HTML、图片附件与链接；见[功能模块](modules/README.md) |
| 工作区 | 文件树、文件列表、大纲、查找替换、快速打开与内容搜索；见[预留主题](planned.md) |
| 显示 | 源码、专注、打字机、缩放、只读及非 Markdown 预览；[显示模式](modules/view-modes.md) |
| 个性化 | 分层设置、内置主题族、用户主题与覆盖；[设置](contracts/settings.md)、[主题](contracts/theme.md) |
| 输出 | 多格式导入导出和打印；[导出与打印](modules/export.md) |
| 可达性 | 全键盘、屏幕阅读器、高对比、减少动画、帮助与关于；[菜单](modules/menus-dialogs.md)及[预留主题](planned.md) |

首次启动打开空白未命名文档，使用即时预览，插入点位于首段起点。外观初值由[主题](contracts/theme.md#外观模式)定义，语言初值由[国际化](modules/i18n.md)定义；帮助与关于不得阻断编辑。

## 产品身份

产品名称与窗口标题见[国际化](modules/i18n.md)。可执行文件名为 `moino`，数据目录名为 `Moino`；当前应用标识为 `app.moino.Moino`，正式发布签名前须结合最终域名确定。项目采用 Apache-2.0；[参考资料](references.md)维护第三方许可边界。

## 平台与非目标

目标平台为 Windows、macOS、Linux。文件格式、正文编辑、历史、主题、菜单语义、计时与保存规则保持一致；系统对话框、原生窗口装饰、输入法候选窗和 macOS 菜单栏遵循平台。框架和原生绘制边界见[架构](architecture.md)，文档与网络边界见[安全](contracts/security.md)。

v1 不提供实时协作、账户、许可证激活、收费墙、云端文档存储、自动上传、插件市场、PDF 编辑、OCR、任意网页浏览、文档脚本、自动更新或遥测。媒体能力见[HTML 与媒体](modules/html-media.md)。

本文列出的能力共同构成 v1 范围；实施顺序见[路线图](roadmap.md)，预览版与完整发布的要求见[发布](release.md)。
