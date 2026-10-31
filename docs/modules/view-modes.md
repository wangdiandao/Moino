# 显示模式与文件预览

## 源码模式

源码与即时预览互斥并共享同一文档。源码模式用 GPUI Kit `Editor` 编辑全文，历史见[撤销契约](../contracts/history.md#历史边界)。Markdown 高亮经 Editor 接缝使用[文档 CST](../contracts/markdown-dialect.md#基础方言与扩展)，不再用代码高亮器解释方言。

## 非 Markdown 文件

`moino_preview` 识别类型并只读解码图片、纯文本与 PDF；其他类型显示文件信息和“用系统应用打开”。PDF 由 hayro 生成页面位图后交 GPUI，不另分发原生 PDF 库。解码失败显示原因并保留系统应用入口。打印走[导出流程](export.md#提交与打印)。
