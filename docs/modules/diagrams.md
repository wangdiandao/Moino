# 图表

`moino_diagram` 封装 merman，无浏览器地解析、布局为 SVG，再由 resvg 绘入 GPUI；导出复用矢量结果。

原生显示的 SVG 使用 merman `SvgPipeline::resvg_safe()`，把 HTML 标签转换为 SVG 文本并清理不受支持的呈现。不能直接使用面向浏览器的默认输出，否则 resvg 会保留图形却丢失 `foreignObject` 中的标签；验收同时检查形状与文字。

## 语法与渲染

围栏包括 `mermaid`、`sequence`、`flow`。Mermaid 范围包含 flowchart、sequenceDiagram、gantt、classDiagram、stateDiagram-v2、erDiagram、pie、mindmap、timeline、journey 等声明类型。后两类由小型解析器转为等价 Mermaid 源码，采用 Mermaid 风格。

颜色取[图表 token](../design/ink-theme.md#正文颜色)，图表配置不能覆盖其范围以外的应用样式。尺寸通过引擎配置调节，各类型实际差异在对应语法说明中列出，语义黄金样例不能替代几何验证。通用失败策略见[错误模型](../architecture.md#错误模型)。
