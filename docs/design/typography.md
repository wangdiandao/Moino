# 排版

## 字体角色

| 角色 | 字体 | 字号与行高 |
|---|---|---|
| 正文 | 随包 Inter | `body.font_size` 为 16 px，`body.line_height` 为 1.6 |
| 界面 | 随包 Inter | 13 px，行高由控件与主题定义 |
| 代码 | 随包 JetBrains Mono | 14 px，行高 1.6 |
| 一级至六级标题 | 正文字体角色 | 36 / 28 / 24 / 20 / 16 / 16 px，行高 1.3 |
| 数学 | RaTeX 附带 KaTeX 字体 | 按公式布局 |

中日韩使用系统无衬线后备：Windows 的 Microsoft YaHei、macOS 的 PingFang SC、Linux 的 Noto Sans CJK SC。缺失字体按后备选择实际字体，并在外观诊断列出；完整缺失顺序见[预留范围](../planned.md#partial-typography)。跨平台字形差异允许，验证须记录实际字体。

正文几何由[布局](layout-geometry.md)定义，运行期样式读取与 Kit 映射由[主题](../contracts/theme.md)负责，字体许可见[参考资料](../references.md#字体与数据)。
