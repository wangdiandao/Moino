# 编码规范

## 工具链与工作区

目标工作区用 `rust-toolchain.toml` 固定 Rust 1.99.1，不低于依赖最低要求 1.95，包含 rustfmt、clippy；升级工具链单独提交。全部 crate 使用 edition 2024、resolver 3，提交 `Cargo.lock`。

依赖声明与引入流程见[开发流程](development.md#依赖与上游)。lint 在 `[workspace.lints]` 定义，成员声明 `[lints] workspace = true`；工作区格式为 rustfmt、`max_width = 100`，不手动对齐或关闭。文本编码见[文档写法](AGENTS.md#写法)，由规划中的 `.gitattributes`、`.editorconfig` 固定。

## 命名

| 对象 | 规则 | 示例 |
|---|---|---|
| crate、目录、库名 | `moino_<area>`，三者一致 | `crates/moino_text` |
| 模块与文件 | `snake_case`；子模块用 `foo.rs` + `foo/`，不使用 `mod.rs` | `selection.rs`、`selection/mapping.rs` |
| 类型、trait、枚举变体 | `UpperCamelCase`；缩写按普通单词处理 | `HtmlBlock`、`TocBlock`、`Utf16Offset` |
| 函数、方法、变量、字段 | `snake_case` | `apply_transaction` |
| 布尔读取 | `is_`、`has_`、`can_`、`should_` 前缀；跨层公开类型的读取方法只用 `is_` / `has_` | `is_dirty()`、`has_selection()` |
| 构造函数 | `new`、`with_*`、`from_*`、`try_from_*` | `Revision::new` |
| 转换 | `as_*`（廉价借用）、`to_*`（复制或计算）、`into_*`（消费） | `as_str`、`to_scalar`、`into_inner` |
| 读取方法 | 不加 `get_` 前缀 | `revision()` |
| 常量与静态量 | `SCREAMING_SNAKE_CASE`；时长使用 `Duration` 而不是毫秒整数 | `AUTOSAVE_OUTER_DELAY: Duration` |
| 枚举类型名 | 以变体"是什么"命名，避免 `Kind` 后缀；外部 crate 的名称保持原样 | `BlockType` 而非 `BlockKind`（除非没有更准确的名称） |
| 上下文变量 | `cx` 只表示 GPUI 的 `App`、`Context<T>`、`AsyncApp`；其他上下文写全称 | `trigger: &ComboboxTriggerContext` |
| 命令 ID | `<family>.<name>`，第二段 lowerCamelCase | `inline.bold`、`edit.pastePlain` |
| GPUI action | 命名空间为命令族，名称 UpperCamelCase | `inline::Bold`、`edit::PastePlain` |
| 设置键 | 点分 `snake_case`，首段为命名空间 | `editor.auto_pair`、`theme.mode` |
| 主题 token | 点分小写，段内 `snake_case` | `content.code_block.background` |
| 本地化键 | 点分小写 | `menu.file.save` |
| 错误码 | 点分小写 `snake_case`，首段为子系统 | `save.target_changed` |
| 环境变量 | `MOINO_` 前缀 | `MOINO_LOG` |
| Cargo 特性 | 短横线小写 | `test-support` |
| 测试函数 | 使用表达行为与条件的名称 | `enter_splits_paragraph`、`save_fails_when_target_changed` |
| 文档与资源文件 | 短横线小写英文 | `layout-geometry.md`、`moino-theme-v1.schema.json` |


## 类型设计

身份使用 newtype，例如 `DocumentId`、`NodeId`、`Epoch`、`Revision`、`LeaseId`、`JobId`、`RequestId`，不跨 crate 传裸整数身份。文本位置使用 `ScalarOffset`、`ByteOffset`、`Utf16Offset`，禁止裸 `usize` 偏移，经 `moino_text` 显式转换；语义见[文档模型](contracts/document-model.md#文本与位置)。

跨层公开快照、能力、上下文和选项使用私有字段、builder 与读取方法，新增字段不造成破坏性变更；不会增长的区间、坐标等值类型可公开字段。非布尔设置方法用 `with_`，读取用字段名；全布尔类型的设置用字段名、读取用 `is_` / `has_`。常用组合判断集中成方法，可能扩展的公开枚举标 `#[non_exhaustive]`。

用户可见排序使用显式比较规则，不依赖集合遍历顺序。

## 错误处理

库使用 thiserror 定义错误，anyhow 仅用于可执行文件、测试与 xtask；跨层字段见[错误模型](architecture.md#错误模型)。用户输入、磁盘、网络与平台结果不用 `unwrap`，`expect` 只表达真正不变量并说明原因。热路径用 `debug_assert!`，模型破坏走[文档保护](contracts/document-model.md#文档不变量)。忽略结果须用 `let _ =` 并在同一行解释。

## unsafe

工作区 `unsafe_code = "deny"`，L0 另加 `#![forbid(unsafe_code)]`。唯一例外是 `moino_platform`，可为操作系统接口声明 `#![allow(unsafe_code)]`；每块前写 `// SAFETY:`，PR 列出位置和前提。

## GPUI 与异步代码

运行时和启动见[架构](architecture.md#运行时模型)，不在 await 期间持有 GPUI 实体借用。元素保持一条 builder 链，分支用 `when`、`when_some`、`when_none`、`map`，不逐项修改临时变量。无状态组件用 `RenderOnce`，跨帧状态、订阅或焦点用 `Entity`；`ElementId` 稳定，键控状态注释说明身份要求。

文案见[国际化](modules/i18n.md)，样式见[主题](contracts/theme.md)，业务派发见[命令接缝](architecture.md#关键接缝)。桌面按钮用默认光标，链接用指针光标。平台 `cfg(target_os = …)` 只在 `moino_platform`，平台差异必须写入所属契约。

## 注释与文档注释

注释使用英文，只解释代码无法表达的约束，不写逐行翻译或修改来历。crate `lib.rs` 用 `//!` 说明职责、层级和允许依赖，公开项写 `///`，`missing_docs` 为 warn。

不提交注释掉的代码；TODO 关联 Issue，如 `// TODO(#123): …`。lint 拒绝 `unwrap_used`、`todo!()`、`dbg!()` 和打印宏，并提示类型转换风险。

## 日志与隐私

使用 tracing，每任务一个 span，记录类型、耗时、错误码和匿名任务 ID，落入滚动本地日志。记录边界见[安全](contracts/security.md#网络与本地数据)。`error` 表示需用户处理，`warn` 表示自动恢复异常，`info` 记生命周期；`debug`、`trace` 只在开发构建或显式启用时输出。
