# 验收数据

[场景矩阵](scenarios.md)链接行为判定，`vectors/*.yaml` 保存精确输入输出。数据格式在此定义，检查与执行方式见[测试策略](../testing.md)。

工程与关键技术验证见[基础验证](m0.md)。

## 向量字段

YAML 根为列表，示例直接看[段落 Enter 数据](vectors/edit-basic.yaml)，不再另存同格式的教学副本。

| 字段 | 含义 |
|---|---|
| `name` | 全体正式数据中唯一的小写英文行为名，如 `enter-splits-paragraph`；无类别前缀或流水号 |
| `specs` | 相对当前 YAML 的归属章节链接列表 |
| `scenario` | 对应场景页锚点的自然名称 |
| `input` | 初始文本或状态 |
| `action` | 操作，纯解析可省略 |
| `expected` | 精确文本、选择或简化结构 |
| `description` | 中文断言说明 |

文本用 LF，`█` 表示插入点，`｢selection｣` 表示选区；空格与尾换行均有意义。不隐含字体或窗口条件，时序和几何必须显式声明并引用参数归属。扩展字段先明确执行约定再同步加载器，不能增加无法执行的数据。

`input` 与 `expected` 可以是空字符串，表示空文档或空结果，不能省略字段；其他字符串字段非空。加载器对缺失字段或类型错误返回诊断，不 panic。

核心文本执行器按 `action` 顺序执行逗号或 `then` 分隔的操作：`insert:"文本"` 为无交互转换的输入；`type:"文本" (auto_pair=true)` 显式启用键盘配对和语法转换；`composition:"文本"` 为已提交组合文本；`(group=1)` 表示同一连续输入键。`tab` / `shift-tab` 在列表中按源文本的容器宽度执行缩进 / 降级。未指定输入组的操作独立成组。解析数据的 `expected` 是只列出所需断言字段的简化结构投影，不能用它替代无损性和区间检查。

## 正式数据

| 文件 | 范围 |
|---|---|
| [edit-basic.yaml](vectors/edit-basic.yaml) | Enter、Backspace、粗体、选区替换、块转换 |
| [edit-structural.yaml](vectors/edit-structural.yaml) | 段落分隔、配对升级、整段格式、列表编号与组合提交 |
| [md-basic.yaml](vectors/md-basic.yaml) | 块、内联、表格、扩展与残缺语法 |
| [sel-basic.yaml](vectors/sel-basic.yaml) | 选择方向与位置映射 |
| [hist-basic.yaml](vectors/hist-basic.yaml) | 撤销重做、合并与重做栈 |

testkit 读取以上数据，核心集成测试执行文本产品断言。正常路径、错误、取消、超时、竞态和资源耗尽都须覆盖，实际缺口见[覆盖边界](scenarios.md#覆盖边界)；不能把关联名称或数据存在当作通过。
