# 验收数据

[场景矩阵](scenarios.md)链接行为判定，`vectors/*.yaml` 保存精确输入输出。数据格式在此定义，检查与执行方式见[测试策略](../testing.md)。

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

## 正式数据

| 文件 | 范围 |
|---|---|
| [edit-basic.yaml](vectors/edit-basic.yaml) | Enter、Backspace、粗体、选区替换、块转换 |
| [md-basic.yaml](vectors/md-basic.yaml) | 块、内联、表格、扩展与残缺语法 |
| [sel-basic.yaml](vectors/sel-basic.yaml) | 选择方向与位置映射 |
| [hist-basic.yaml](vectors/hist-basic.yaml) | 撤销重做、合并与重做栈 |

规划中的 testkit 读取以上数据。正常路径、错误、取消、超时、竞态和资源耗尽都须覆盖，实际缺口见[覆盖边界](scenarios.md#覆盖边界)；不能把关联名称或数据存在当作通过。
