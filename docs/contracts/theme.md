# 主题系统

`moino_theme` 维护注册表、主题族、样式与外观，是颜色和排版的唯一来源；界面与正文只能读取有效 token。读取扩展方法不得与 Kit 的 `cx.theme()` 冲突。系统参照 Zed 的架构与 JSON 约定，许可边界见[参考资料](../references.md#许可边界)。

## 主题文件

主题使用 UTF-8 JSONC，允许行注释和块注释。JSON Schema 由类型生成，随包提供 `moino-theme-v1.schema.json`；版本变化保持兼容或提供迁移。

| 层级 | 字段与含义 |
|---|---|
| 主题族 | `$schema`、`id`、`name`、`localized_name`、`author`、`themes` 数组 |
| 主题 | `id`、`name`、`localized_name`、`appearance`（`light` / `dark`）、`style` |
| `style` | 界面与状态颜色使用带点号的扁平键；`syntax` 存语法样式，`content` 存正文 token |
| `syntax` 类别 | `color`、`background_color`、`font_style`（`normal` / `italic`）、`font_weight`（100–900） |
| `content` | 颜色键以 `content.` 开头；`typography` 与 `layout` 分别存排版和几何键 |

颜色只接受 `#RRGGBB`、`#RRGGBBAA`；数值按所属设计章节校验。颜色值见[内置主题](../design/ink-theme.md)，`body.font_size`、`body.line_height` 等键见[排版](../design/typography.md)，`content.max_width` 等键见[布局](../design/layout-geometry.md)。

未知键保留并忽略，给出诊断；缺失键回退同外观的内置主题。字体只能用族名，禁止 URL、文件路径和其他资源引用，违反则整个主题校验失败。

## 注册表与加载

内置主题编译进程序，ID 保留，不允许用户覆盖；复制为用户主题必须换 ID。内置数据缺损是构建错误，运行时加载失败则回退编译进程序的同一取值。

用户主题从配置目录 `Moino/themes/*.json` 加载并监视，变化合并 200 ms 后重载受影响主题族。无效主题不可选；当前主题重载失败时保留上一个有效值并显示文件位置诊断。安全模式只加载内置主题，忽略用户主题和覆盖，仍允许打开设置并恢复默认。

## 外观模式

`theme.mode` 可为 `system`、`light`、`dark`，默认 `system`；`theme.light`、`theme.dark` 默认对应[内置主题族](../design/ink-theme.md#主题族身份)的浅色和深色 ID。跟随系统时，外观变化在下一次绘制生效。

切换不改正文、脏状态或历史，通过滚动锚点保持首个可见位置。

## 主题覆盖

`theme_overrides` 以 `"*"` 或主题 ID 为键，值结构同 `style`，取代用户 CSS。合成顺序从低到高为内置默认 → 当前主题 → 全局覆盖 → 当前主题覆盖 → 高对比 → 无障碍强制规则。

只能覆盖 schema 内 token；无效项逐项拒绝并诊断，其他项照常生效。覆盖不能加载资源、执行脚本、隐藏安全确认或改变命中区域。

## 高对比覆盖层

`accessibility.high_contrast` 可为 `system`、`on`、`off`，默认 `system`。开启时由主题系统维护配对表，至少覆盖[设计配对](../design/ink-theme.md#对比度)，并按其中阈值检查正文、链接、标记、控件与焦点。

不达标前景在 OKLCH 中保持色相，向远离背景方向调整明度，必要时降低彩度。覆盖不改字体、字号、行高或换行宽度，不悄悄整体替换色板；设置页显示是否生效及来源。内置主题的具体修正见[高对比取值](../design/ink-theme.md#高对比)。

## 投影到 GPUI Kit

有效主题生成 Kit 的 `ThemeColor`、`HighlightTheme` 和 `TextViewStyle` 后刷新窗口。未列的键按最接近语义推导，映射实现必须有测试。

| GPUI Kit 键 | Moino token |
|---|---|
| `background` | `background` |
| `foreground` | `text` |
| `border`、`input.border` | `border` |
| `muted.background` / `muted.foreground` | `element.hover` / `text.muted` |
| `primary.background` / `primary.foreground` | `accent` / `accent.foreground` |
| `accent.background` / `accent.foreground` | `element.selected` / `text` |
| `popover.background` / `popover.foreground` | `surface.background` / `text` |
| `list.active.background` / `list.active.border` | `list.active.background` / `list.active.border` |
| `link.foreground` / `link.hover.foreground` | `content.link` / `content.link.hover` |
| `danger.background` / `danger.foreground` | `error` / `accent.foreground` |
| `scrollbar.thumb.background` | `scrollbar.thumb.background` |
| `title_bar.background` / `title_bar.border` | `title_bar.background` / `border` |
| `status_bar.background` | `status_bar.background` |
| `ring` | `focus.ring` |


字体取[字体角色](../design/typography.md#字体角色)，投影后验证控件命中尺寸。缺失字体的选择和诊断见[排版](../design/typography.md)。

## 失效

每次有效主题改变递增主题修订。依赖主题的排版、高亮、数学、图表和图片占位都将主题修订纳入[任务身份](../architecture.md#运行时模型)，旧结果不得提交。图表合并重绘时机仍需在[时序](../planned.md#motion-timing)定义。
