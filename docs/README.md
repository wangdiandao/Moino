# Moino 文档

文档定义 Moino（墨）的产品要求与工程约定。按产品、领域词汇、架构进入相关正文；阅读写法见[文档编写](AGENTS.md)，当前工程条件见[开发流程](development.md#工程目录与使用边界)。

| 入口 | 内容 |
|---|---|
| [产品](product.md) | 使用范围、平台、非目标与产品身份 |
| [领域词汇](CONTEXT.md) | 概念与统一用语 |
| [架构](architecture.md) | crate、依赖、运行时、接缝与错误边界 |
| [共享契约](contracts/README.md) | 文档、历史、设置、主题、会话和安全 |
| [功能模块](modules/README.md) | 编辑、选择、富内容、显示、菜单与语言 |
| [视觉设计](design/README.md) | 配色、排版、布局与图标 |
| [验收](acceptance/README.md) | 场景和正式输入输出数据 |
| [开发流程](development.md) | 环境、任务、分支、提交、依赖与上游 |
| [编码规范](coding-standards.md) | Rust、GPUI、类型、注释与日志 |
| [测试策略](testing.md) | 自动检查、平台实测与文档验收 |
| [扩展指南](cookbook/README.md) | 常见改动的实施步骤 |
| [路线图](roadmap.md) | 里程碑、依赖和退出范围 |
| [发布](release.md) | 交付、安装、签名与发布门槛 |
| [预留范围](planned.md) | 未编写主题和已有正文的未决细节 |
| [参考资料](references.md) | 资料来源、依赖、字体与许可证 |
| [代理约定](../AGENTS.md) | 任务执行与 API 查阅 |

共享机制归 `contracts/`，用户行为归 `modules/`，参数归 `design/`。要求在对应正文定义一次，验收链接其判定依据；进度与结果遵循[开发交付](development.md#文档先行工作流)。
