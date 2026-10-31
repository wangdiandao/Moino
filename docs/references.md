# 参考资料

外部资料与许可证清单；产品依赖版本见[架构](architecture.md#依赖基线)，引入和升级按[开发流程](development.md#依赖与上游)重新核对。

## 产品与文档组织

- Typora：产品体验的对标对象。Moino 的规格是根据对其行为的内部分析独立重写的，不复制其代码、主题或资源。
- Widget Workshop：用户指定的本机参考目录 `E:/repos/Widget Workshop/docs`。参考其产品、领域词汇、架构、共享契约 `contracts/`、功能模块 `modules/` 与开发操作的职责划分，以及单一权威定义和验证范围的写法。Moino 保留独立的产品要求、工程约定与验收数据，不引入参考项目的产品规则、SDK 或工具。该路径只用于本机溯源，不作为仓库导航链接。

## 框架与上游项目

| 项目 | 用途 | 许可证 |
|---|---|---|
| [GPUI](https://github.com/zed-industries/zed)（以 `gpui-pre` 快照发布） | 界面框架 | Apache-2.0 |
| [GPUI Kit](https://github.com/longbridge/gpui-kit)（`gpui-kit`、`gpui-base`、`gpui-component`），文档 <https://gpui-kit.com> | 组件、TextView 与 `editor_host`、`Editor`、主题投影目标、测试工具 | Apache-2.0 |
| [Zed](https://github.com/zed-industries/zed) 主题系统 | 只参考架构与 JSON 约定（主题族、语义颜色键、外观模式、主题覆盖） | Zed 应用代码为 GPL-3.0，Moino 不复制其代码 |
| [RaTeX](https://github.com/erweixin/RaTeX) | TeX 数学解析与排版 | MIT |
| [merman](https://github.com/Latias94/merman) | Mermaid 图表 | MIT OR Apache-2.0 |
| [Typst](https://github.com/typst/typst) | 分页导出排版 | Apache-2.0 |
| [hayro](https://github.com/LaurenzV/hayro) | PDF 预览 | Apache-2.0 OR MIT |
| [spellbook](https://github.com/helix-editor/spellbook) | 拼写检查 | MPL-2.0 |

## 规范与标准

| 标准 | 用途 |
|---|---|
| [CommonMark 0.31.2](https://spec.commonmark.org/0.31.2/) | 基础 Markdown 语法与一致性样例 |
| [GitHub Flavored Markdown 0.29-gfm](https://github.github.com/gfm/) | 表格、删除线、任务列表、自动链接扩展 |
| [Unicode 标准附件 #29](https://www.unicode.org/reports/tr29/) | 字素簇与词边界 |
| [Unicode 标准附件 #9](https://www.unicode.org/reports/tr9/) | 双向文字 |
| [Unicode 标准附件 #15](https://www.unicode.org/reports/tr15/) | NFC 规范化 |
| [WCAG 2.2](https://www.w3.org/TR/WCAG22/) | 对比度、焦点与键盘可达性 |
| [YAML 1.2](https://yaml.org/spec/1.2.2/) | 文档元数据与验收向量 |
| [JSON Schema](https://json-schema.org/) | 设置、主题与语义文档 JSON 的校验 |
| [TextBundle](http://textbundle.org/spec/) | 便携文档容器 |
| [Conventional Commits](https://www.conventionalcommits.org/) | 提交信息格式 |
| [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/naming.html) | 命名规范 |

## 依赖许可证

| crate | 许可证 |
|---|---|
| `gpui-kit` | Apache-2.0 |
| `ropey` | MIT OR Apache-2.0 |
| `unicode-segmentation` | MIT OR Apache-2.0 |
| `ratex-layout` 等 RaTeX crate | MIT |
| `merman` | MIT OR Apache-2.0 |
| `typst` 系列 | Apache-2.0 |
| `hayro` | Apache-2.0 OR MIT |
| `spellbook` | MPL-2.0 |
| `syntect` | MIT |
| `two-face` | MIT OR Apache-2.0 |
| `fancy-regex` | MIT |
| `interprocess` | 0BSD OR Apache-2.0 |
| `keyring` | MIT OR Apache-2.0 |
| `saphyr` | MIT OR Apache-2.0 |
| `docx-rs` | MIT |
| `epub-builder` | MPL-2.0 |
| `notify` | CC0-1.0 |
| `trash` | MIT |
| `encoding_rs` | (Apache-2.0 OR MIT) AND BSD-3-Clause |
| `chardetng` | Apache-2.0 OR MIT |
| `zip` | MIT |
| `image` | MIT OR Apache-2.0 |
| `resvg` | Apache-2.0 OR MIT |
| `rust-i18n` | MIT |
| `tracing` | MIT |
| `thiserror`、`anyhow` | MIT OR Apache-2.0 |
| `pulldown-cmark`（仅开发依赖） | MIT |
| `proptest`（仅开发依赖） | MIT OR Apache-2.0 |
| `insta`（仅开发依赖） | Apache-2.0 |
| `cargo-packager`（工具） | Apache-2.0 OR MIT |

## 字体与数据

| 资源 | 用途 | 许可证 |
|---|---|---|
| [Inter](https://github.com/rsms/inter) | 正文与界面字体 | SIL OFL 1.1 |
| [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono) | 代码字体 | SIL OFL 1.1 |
| KaTeX 字体（随 RaTeX 提供） | 数学字体 | SIL OFL 1.1 |
| Libertinus Serif（取自固定 `typst-assets 0.15.1`，未修改） | M0 PDF 固定样本字体；不替代产品字体选择 | [SIL OFL 1.1](../assets/fonts/libertinus-ofl.txt) |
| Hunspell `en_US` 词典（基于 SCOWL） | 随包拼写词典 | 以随包附带的许可证文本为准，打包前逐项核对 |

## 许可边界

项目使用 [Apache-2.0](../LICENSE)。依赖允许 MIT、Apache-2.0（包括 LLVM 例外）、BSD、ISC、Zlib、Unicode、CC0、0BSD 与 MPL-2.0，字体为 OFL-1.1，由 `deny.toml` 和 cargo-deny 检查。LLVM 例外来自归档写入工具链的传递依赖，保留其许可文本。

merman 使用显式 `all-diagrams` 与 `complete-svg`，不启用含 EPL-2.0 代码的 ELK 特性。Typst 原型的传递依赖 citationberg 引入旧版 quick-xml；仅编译受信固定样本，World 拒绝外部 CSL、参考文献、包和其他文件。两条 XML 拒绝服务公告的限定处置见 `deny.toml`，正式导出接入外部资源前必须重新审查；不得扩大该例外。未维护公告检查工作区直接依赖，传递依赖随固定上游版本单独跟踪。

不启用 `typst-kit/embedded-fonts`；它的默认集合含 GUST、Bitstream 与带 GPL 字体例外的字体。原型只嵌入上述未修改的 Libertinus Serif，保留其版权声明与 OFL；中日韩通过系统字体解析，不能把本机可用字体视为正式随包字体已准备就绪。

Zed 应用等 GPL 代码只参考架构与约定，不复制源码。词典清单记录各自许可并逐份核对再分发条件；来源清单不能替代引入时检查。
