# Markdown 测试夹具

`commonmark-0.31.2.json` 为 [CommonMark 0.31.2 官方示例](https://spec.commonmark.org/0.31.2/spec.json)的原始下载，包含 652 个输入和 HTML 预期，未修改。

CommonMark Spec，John MacFarlane 著，版权 © 2014–16 John MacFarlane，采用 [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/)；本夹具及其派生版本保持该许可，不适用仓库源码的 Apache-2.0。上游许可声明见 [commonmark/commonmark-spec LICENSE](https://github.com/commonmark/commonmark-spec/blob/0.31.2/LICENSE)。仅用于开发测试，不进入产品资源或发布构建。

`incremental-boundaries.json` 保存项目随机测试发现的增量边界反例，包括 Linux、Windows CI 样本与扩大验证发现的跨窗口内联上下文样本。`start`、`end` 是编辑前标量偏移，`replacement` 经标准换行规范化后应用；测试同时检查显式编辑区间及推导编辑区间的增量结果与全量结果一致。这是项目自有生成数据，使用仓库源码许可。
