# 场景矩阵

每行是检查入口，必须结果只在判定章节定义。正式数据通过 `scenario` 关联，不在此复制断言正文。


## 编辑

| 场景 | 判定章节 | 数据 |
|---|---|---|
| <a id="enter-splits-paragraph"></a>Enter 拆分段落 | [正文](../modules/editing.md#enter) | [edit-basic.yaml](vectors/edit-basic.yaml) |
| <a id="enter-exits-heading"></a>Enter 退出标题 | [正文](../modules/editing.md#enter) | [edit-basic.yaml](vectors/edit-basic.yaml) |
| <a id="list-enter"></a>列表 Enter | [正文](../modules/editing.md#enter) | [edit-structural.yaml](vectors/edit-structural.yaml) |
| <a id="tab-indentation"></a>列表缩进与逻辑选择 | [正文](../modules/editing.md#tab-与缩进) | [edit-structural.yaml](vectors/edit-structural.yaml) |
| <a id="backspace-merges-blocks"></a>Backspace 合并 | [正文](../modules/editing.md#backspace-与-delete) | [edit-basic.yaml](vectors/edit-basic.yaml) |
| <a id="syntax-conversion"></a>语法转换 | [正文](../modules/editing.md#交互语法转换) | [edit-structural.yaml](vectors/edit-structural.yaml) |
| <a id="automatic-pairing"></a>自动配对 | [正文](../modules/editing.md#自动配对与智能标点) | [edit-structural.yaml](vectors/edit-structural.yaml) |
| <a id="formatting-toggle"></a>格式命令切换 | [正文](../modules/editing.md#格式命令) | [edit-basic.yaml](vectors/edit-basic.yaml) |
| <a id="ime-composition"></a>IME 组合 | [正文](../modules/editing.md#输入法组合) | [edit-structural.yaml](vectors/edit-structural.yaml) |

## 选择与标记展开

| 场景 | 判定章节 | 数据 |
|---|---|---|
| <a id="selection-direction"></a>选择语义方向 | [正文](../modules/selection.md#逻辑选择) | [sel-basic.yaml](vectors/sel-basic.yaml) |
| <a id="marker-expansion"></a>标记展开与折叠 | [正文](../modules/selection.md#标记展开与折叠) | [sel-basic.yaml](vectors/sel-basic.yaml) |
| <a id="multiple-click-selection"></a>多击 | [正文](../modules/selection.md#多击与拖选) | 待补充 |
| <a id="caret-visibility"></a>滚动保障 | [正文](../modules/selection.md#滚动保障) | 待补充 |

## 文档模型

| 场景 | 判定章节 | 数据 |
|---|---|---|
| <a id="transaction-atomicity"></a>事务原子性 | [正文](../contracts/document-model.md#事务契约) | 待补充 |
| <a id="selection-mapping"></a>选择映射 | [正文](../contracts/document-model.md#选择与映射) | 待补充 |
| <a id="node-identity"></a>节点 ID 稳定 | [正文](../contracts/markdown-dialect.md#增量解析与节点身份) | 待补充 |
| <a id="snapshot-isolation"></a>快照隔离 | [正文](../contracts/document-model.md#事务契约) | 待补充 |

## Markdown 解析

| 场景 | 判定章节 | 数据 |
|---|---|---|
| <a id="basic-block-parsing"></a>基础块解析 | [正文](../contracts/markdown-dialect.md#块解析优先级) | [md-basic.yaml](vectors/md-basic.yaml) |
| <a id="inline-parsing"></a>内联语法 | [正文](../contracts/markdown-dialect.md#cst-与标记区间) | [md-basic.yaml](vectors/md-basic.yaml) |
| <a id="table-parsing"></a>表格 | [正文](../contracts/markdown-dialect.md#块解析优先级) | [md-basic.yaml](vectors/md-basic.yaml) |
| <a id="extension-parsing"></a>扩展语法 | [正文](../contracts/markdown-dialect.md#基础方言与扩展) | [md-basic.yaml](vectors/md-basic.yaml) |
| <a id="incremental-parsing"></a>增量解析与 ID 稳定 | [正文](../contracts/markdown-dialect.md#增量解析与节点身份) | 待补充 |
| <a id="syntax-conformance"></a>规范一致性 | [正文](../contracts/markdown-dialect.md#基础方言与扩展) | 待补充 |

## 撤销重做

| 场景 | 判定章节 | 数据 |
|---|---|---|
| <a id="input-history"></a>输入合并 | [正文](../contracts/history.md#撤销单元) | [hist-basic.yaml](vectors/hist-basic.yaml) |
| <a id="structural-history"></a>结构命令独立成组 | [正文](../contracts/history.md#历史边界) | 待补充 |
| <a id="redo-history"></a>重做栈清空 | [正文](../contracts/history.md#撤销单元) | [hist-basic.yaml](vectors/hist-basic.yaml) |
| <a id="history-capacity"></a>栈容量上限 | [正文](../contracts/history.md#历史预算) | 待补充 |

## 主题与排版

| 场景 | 判定章节 | 数据 |
|---|---|---|
| <a id="theme-layout"></a>主题与排版 | [正文](../contracts/theme.md#外观模式) | 待补充 |
| <a id="invalid-theme"></a>无效主题 | [正文](../contracts/theme.md#注册表与加载) | 待补充 |
| <a id="theme-overrides"></a>主题覆盖与安全模式 | [正文](../contracts/theme.md#主题覆盖) | 待补充 |
| <a id="high-contrast"></a>高对比覆盖 | [正文](../contracts/theme.md#高对比覆盖层) | 待补充 |

## 覆盖边界

标记展开场景的数据实际检查位置映射；段落 Enter 场景也关联选区替换。这些既有关系不表示对应行为已充分覆盖，须补全而不能仅靠改名修饰。主题场景中的字号、缩放、双向文字交叉判定还依赖[未决细节](../planned.md#partial-i18n)。

组合提交的文本向量检查提交事务与配对隔离，不验证原生候选窗或界面的临时组合态。
