# Moino 开发约定

先读[文档入口](docs/README.md)、[产品](docs/product.md)、[领域词汇](docs/CONTEXT.md)和[架构](docs/architecture.md)，再读受影响正文与验收。遵循[文档写法](docs/AGENTS.md)、[开发流程](docs/development.md)、[编码规范](docs/coding-standards.md)、[测试策略](docs/testing.md)及[安全边界](docs/contracts/security.md)，不在此复制要求。

## 执行

已授权工作直接完成，文档先行不增加审批。保留任务外用户改动，不顺带创建工程骨架、升级依赖或扩大范围。仓库与官方资料能查证的问题自行查证；重大未决产品或架构取舍用 grill-me 澄清，确认后直接改所属正文，不另建决策记录。

## API 资料

处理界面、交互与 GPUI 机制前，安装并阅读 `gpui-kit` 和 `gpui-kit-design-guides`：`npx skills add longbridge/gpui-kit`；仓库不保存技能副本。GPUI API 以工作区固定版本源码和 docs.rs 为准，不凭记忆编写。

涉及其他库、框架、SDK、API、CLI 或云服务的具体语法、配置、调试、迁移和用法，优先使用 Context7，即使熟悉也须查询：以正式库名和完整问题解析 ID，按版本、相关性、来源与片段质量选择，每次查询聚焦一个概念。普通重构、业务逻辑、通用脚本和文档整理无需查询；其中涉及具体 API 结论时仍须核对。Context7 未覆盖时补官方资料并说明缺口。
