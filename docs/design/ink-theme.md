# 墨绿主题族

## 主题族身份

- 内置主题族 ID 为 `moino`，显示名"墨"（英文 "Moino"），包含两个主题：浅色"宣纸"（ID `moino-paper`，英文 "Paper"）与深色"墨池"（ID `moino-inkstone`，英文 "Inkstone"）。
- 配色方向为"宣纸墨绿"：浅色主题以暖白宣纸为纸面、带绿调的墨色为正文、深墨绿为强调；深色主题以近黑的墨绿"墨池"为纸面、浅灰绿为正文、玉色为强调。
- 正文不是悬浮纸张卡片：正文背景与窗口背景相同，侧栏与状态栏使用略深（宣纸）或略深暗（墨池）的平面色，以细分隔线区分，阴影只用于弹层与对话框。

## 界面颜色

- 界面 token 取下表值。

| token | 用途 | 宣纸 | 墨池 |
|---|---|---|---|
| `background` | 窗口底色 | `#F7F6F1` | `#111714` |
| `editor.background` | 正文背景 | `#F7F6F1` | `#111714` |
| `panel.background` | 侧栏背景 | `#EFEDE6` | `#0D1210` |
| `status_bar.background` | 占位状态栏背景 | `#EFEDE6` | `#0D1210` |
| `title_bar.background` | 自绘标题栏背景 | `#EFEDE6` | `#0D1210` |
| `surface.background` | 菜单、弹层、通知 | `#FDFCF9` | `#18201C` |
| `elevated_surface.background` | 对话框、快速打开 | `#FDFCF9` | `#1B2420` |
| `border` | 分隔线、边框 | `#DDDBD2` | `#25302A` |
| `border.variant` | 弱分隔 | `#E7E5DD` | `#1E2823` |
| `text` | 主要文字 | `#1E2622` | `#D3DCD6` |
| `text.muted` | 次要文字 | `#5B6660` | `#93A299` |
| `text.placeholder` | 占位文字 | `#68726C` | `#82918A` |
| `text.disabled` | 禁用文字 | `#A3ABA6` | `#56645C` |
| `text.accent` | 强调文字 | `#2D6A4F` | `#5DB38A` |
| `icon` | 图标 | `#3F4A44` | `#B9C4BD` |
| `icon.muted` | 弱化图标 | `#6B756F` | `#84928A` |
| `icon.disabled` | 禁用图标 | `#B2B8B3` | `#4A5650` |
| `accent` | 墨绿强调色 | `#2D6A4F` | `#5DB38A` |
| `accent.hover` | 强调色悬停 | `#24573F` | `#6FC49A` |
| `accent.foreground` | 强调底上的文字 | `#FFFFFF` | `#0D1210` |
| `focus.ring` | 键盘焦点环 | `#2D6A4F` | `#6FC49A` |
| `element.hover` | 控件悬停背景 | `#E8E6DE` | `#1B2420` |
| `element.active` | 控件按下背景（悬停色混合 20% 强调色） | `#C3CDC1` | `#284135` |
| `element.selected` | 选中项背景 | `#E3E8E1` | `#1C2A23` |
| `list.active.background` | 活动文件行背景 | `#E3E8E1` | `#1C2A23` |
| `list.active.border` | 活动文件 4 px 左边线 | `#2D6A4F` | `#5DB38A` |
| `scrollbar.thumb.background` | 滚动条滑块 | `#C9CCC4` | `#2F3A34` |
| `scrollbar.thumb.hover_background` | 滚动条滑块悬停 | `#B3B7AE` | `#3D4A43` |
| `scrollbar.track.background` | 滚动条轨道 | `#00000000` | `#00000000` |
| `tooltip.background` | 提示背景 | `#2A332E` | `#2A3530` |
| `tooltip.foreground` | 提示文字 | `#F2F4F1` | `#E1E8E3` |
| `modal.scrim` | 模态遮罩 | `#00000040` | `#00000073` |
| `search.match.background` | 搜索命中 | `#E9DDB0` | `#3C3A26` |
| `search.match.current.background` | 当前命中 | `#DFC26A` | `#6A5A24` |
| `search.match.current.border` | 当前命中边线 | `#9A7420` | `#D0A848` |

- 活动文件（4 px 左边线 + 背景）与键盘焦点（2 px `focus.ring` 框）在形状上不同，二者可以同时出现，不得合并为同一种视觉。

## 状态颜色

- 状态 token 取下表值；状态色与品牌墨绿保持色相区分，错误、警告、成功、信息在任何场合都必须同时配有文字或图形，不只依赖颜色。

| token | 宣纸 | 墨池 |
|---|---|---|
| `error` / `error.background` / `error.border` | `#B42318` / `#FBE9E7` / `#E5A39B` | `#F08A7E` / `#2B1715` / `#6B2E27` |
| `warning` / `warning.background` / `warning.border` | `#8A5200` / `#FBF1DD` / `#E2C17E` | `#F3C16A` / `#2A2214` / `#6A5424` |
| `success` / `success.background` / `success.border` | `#1F6F43` / `#E5F1E9` / `#9FCBB0` | `#8DD6AA` / `#13261B` / `#2E5A40` |
| `info` / `info.background` / `info.border` | `#34548A` / `#E8EEF6` / `#A9BCDB` | `#8FB4E8` / `#16202D` / `#34496B` |

## 正文颜色

- 正文 token（主题文件 `content` 节）取下表值。

| token | 用途 | 宣纸 | 墨池 |
|---|---|---|---|
| `content.text` | 正文 | `#1E2622` | `#D3DCD6` |
| `content.text.muted` | 图注、元信息 | `#5B6660` | `#93A299` |
| `content.heading` | 标题文字 | `#17201C` | `#E2E9E4` |
| `content.heading.rule` | H1、H2 下方 1 px 细线 | `#DDDBD2` | `#25302A` |
| `content.link` | 链接 | `#1F6B4A` | `#7DC6A0` |
| `content.link.hover` | 链接悬停 | `#17543A` | `#9AD6B6` |
| `content.marker` | 展开时的语法标记 | `#9AA69F` | `#5E6D65` |
| `content.selection` | 文本选择背景 | `#D2E3D8` | `#29423A` |
| `content.code.background` | 行内代码背景 | `#EEECE5` | `#17201B` |
| `content.code_block.background` | 代码块背景 | `#EEECE5` | `#17201B` |
| `content.raw_block.background` | 获得焦点的代码 / 数学 / HTML 编辑表面 | `#F2F1EB` | `#141C18` |
| `content.quote.border` | 引用左边线 | `#C9D6CE` | `#2F4A3E` |
| `content.quote.text` | 引用正文 | `#4A5550` | `#A9B6AE` |
| `content.table.border` | 表格边线 | `#DDDBD2` | `#25302A` |
| `content.table.header.background` | 表头背景 | `#EFEDE6` | `#18201C` |
| `content.rule` | 水平分隔线 | `#DDDBD2` | `#2C3832` |
| `content.mark.background` | `==高亮==` 背景 | `#F4E8A8` | `#4E4724` |
| `content.task.border` | 任务框边线 | `#8C9690` | `#6E7D75` |
| `content.task.checked.background` | 已勾选任务框 | `#2D6A4F` | `#5DB38A` |
| `content.task.check` | 勾选符号 | `#FFFFFF` | `#0D1210` |
| `content.heading_badge.border` | H3–H6 等级标识边框 | `#C9CFCB` | `#3A4740` |
| `content.heading_badge.text` | H3–H6 等级标识文字 | `#6B756F` | `#93A299` |
| `content.image.placeholder.background` | 图片占位背景 | `#EEECE5` | `#17201B` |
| `content.image.placeholder.text` | 图片占位文字 | `#5B6660` | `#93A299` |

- 提示块颜色取下表值：标题与左边线使用"颜色"，背景使用"背景"，正文使用 `content.text`。

| 类型 | 宣纸 颜色 / 背景 | 墨池 颜色 / 背景 |
|---|---|---|
| `note` | `#34548A` / `#E8EEF6` | `#8FB4E8` / `#16202D` |
| `tip` | `#2D6A4F` / `#E5EFE9` | `#5DB38A` / `#14231C` |
| `important` | `#6A4C93` / `#EEE9F4` | `#B79BE0` / `#1E1A2A` |
| `warning` | `#8A5200` / `#F7EFDC` | `#E3B866` / `#2A2214` |
| `caution` | `#A8402F` / `#F7E7E3` | `#E3826E` / `#2B1A17` |

- 图表颜色取下表值，并通过[图表](../modules/diagrams.md)定义的映射传给图表引擎；图表配置不得覆盖这些值以外的应用样式。

| token | 用途 | 宣纸 | 墨池 |
|---|---|---|---|
| `content.diagram.node.background` | 节点填充 | `#EEF3EF` | `#18241E` |
| `content.diagram.node.border` | 节点边线 | `#2D6A4F` | `#5DB38A` |
| `content.diagram.text` | 图表文字 | `#1E2622` | `#D3DCD6` |
| `content.diagram.edge` | 连线与箭头 | `#5B6660` | `#93A299` |
| `content.diagram.cluster.background` | 分组背景 | `#F2F1EB` | `#141C18` |

## 语法高亮

- 语法 token 采用 Zed 兼容名称，取下表值；未列出的语法类别继承 `content.text`。名称中的"朱砂""靛青""古铜""黛青"是配色意图，便于后续调色保持一致。

| token | 意图 | 宣纸 | 墨池 | 字形 |
|---|---|---|---|---|
| `comment`、`comment.doc` | 灰绿 | `#616B65` | `#7E8C84` | 斜体 |
| `keyword`、`tag`、`punctuation.list_marker` | 墨绿 | `#2D6A4F` | `#6FC49A` | 常规 |
| `string`、`text.literal` | 朱砂 | `#A8432A` | `#E08D72` | 常规 |
| `string.escape`、`string.special`、`constant`、`boolean`、`preproc`、`punctuation.special` | 紫 | `#6A4C93` | `#B79BE0` | 常规 |
| `string.regex` | 赭 | `#8A5200` | `#E3B866` | 常规 |
| `number` | 靛青 | `#34548A` | `#8FB4E8` | 常规 |
| `function`、`constructor` | 黛青 | `#1F6470` | `#6CC0C9` | 常规 |
| `type`、`attribute`、`label`、`enum`、`variant` | 古铜 | `#7A5A1E` | `#D9B26A` | 常规 |
| `property` | 青灰 | `#3D5A50` | `#A9C4B6` | 常规 |
| `operator` | 墨灰 | `#4F5A54` | `#93A299` | 常规 |
| `punctuation`、`punctuation.bracket`、`punctuation.delimiter` | 次要 | `#5B6660` | `#93A299` | 常规 |
| `variable`、`embedded` | 正文 | `#1E2622` | `#D3DCD6` | 常规 |
| `variable.special` | 紫 | `#6A4C93` | `#B79BE0` | 常规 |
| `title` | 标题 | `#17201C` | `#E2E9E4` | 粗体 |
| `emphasis` | 正文 | `#1E2622` | `#D3DCD6` | 斜体 |
| `emphasis.strong` | 正文 | `#1E2622` | `#D3DCD6` | 粗体 |
| `link_text` | 链接 | `#1F6B4A` | `#7DC6A0` | 常规 |
| `link_uri` | 灰绿 | `#616B65` | `#7E8C84` | 下划线 |
| `hint` | 灰绿 | `#616B65` | `#7E8C84` | 斜体 |
| `predictive` | 标记 | `#9AA69F` | `#5E6D65` | 斜体 |

## 对比度

- 普通界面文字与正文对其背景的对比度至少 4.5:1；大字至少 3:1；焦点环与关键非文本控件至少 3:1。`content.marker`、`text.disabled`、`icon.disabled` 与 `predictive` 属于弱化或禁用表现，在主题保真配置下可以低于 4.5:1，但不得是访问其内容的唯一途径。
- 主要配对的对比度（WCAG 2 相对亮度，近似值）如下；每次修改 token 后必须重新计算整表，自动检查工具规划为 `cargo xtask theme-contrast`（规划中）。

| 前景 / 背景 | 宣纸 | 墨池 |
|---|---|---|
| `text` / `editor.background` | 14:1 | 13:1 |
| `text.muted` / `editor.background` | 5.5:1 | 6.8:1 |
| `text.placeholder` / `editor.background` | 4.6:1 | 5.5:1 |
| `content.link` / `editor.background` | 6.0:1 | 9.0:1 |
| `accent` / `editor.background` | 5.9:1 | 7.2:1 |
| `accent.foreground` / `accent` | 6.4:1 | 7.4:1 |
| `content.quote.text` / `editor.background` | 7.2:1 | 8.6:1 |
| `error` / `editor.background` | 6.1:1 | 7.5:1 |
| `warning` / `editor.background` | 5.9:1 | 10.9:1 |
| `success` / `editor.background` | 5.7:1 | 10.7:1 |
| `info` / `editor.background` | 7.0:1 | 8.5:1 |
| `content.marker` / `editor.background` | 2.3:1（弱化） | 3.3:1（弱化） |
| `comment` / `content.code_block.background` | 4.7:1 | 4.7:1 |
| `string` / `content.code_block.background` | 5.1:1 | 6.5:1 |
| `number` / `content.code_block.background` | 6.4:1 | 7.8:1 |
| `keyword` / `content.code_block.background` | 5.4:1 | 8.0:1 |
| `function` / `content.code_block.background` | 5.7:1 | 8.0:1 |
| `type` / `content.code_block.background` | 5.4:1 | 8.4:1 |

## 高对比

- 高对比覆盖层按[主题系统](../contracts/theme.md)的计算规则生成；对本主题族，其结果必须至少把 `content.marker` 提升到 `text.muted` 的取值，把 `text.disabled` 提升到不低于 3:1，并为 `content.selection` 增加 1 px `focus.ring` 外框。合成限制见[高对比机制](../contracts/theme.md#高对比覆盖层)。
