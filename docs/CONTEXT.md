# 领域词汇

这里只解释概念；行为、格式与参数以对应正文为准。

## 产品与界面

**墨 / Moino**（Moino）：本产品的简体中文名与其他语言名称。

**即时预览**（live preview）：直接编辑排版后文档的主表面。

**源码模式**（source mode）：显示完整 Markdown 源文本的全文编辑表面。

**非 Markdown 预览**（file preview）：对图片、纯文本、PDF 等非 Markdown 文件的只读呈现。

**主内容视口**（content viewport）：负责正文垂直滚动的唯一区域。

**正文盒**（content box）：主内容视口内水平居中、宽度受主题最大宽限制的正文区域。

**侧栏**（sidebar）：窗口左侧的文件 / 大纲面板。

**状态栏**（status bar）：窗口底部的模式切换、字数与拼写状态区域。

**应用菜单面板**（app menu panel）：占满窗口的菜单面板（新建、打开、导出、主题、设置等）。

**菜单栏**（menu bar）：承载应用菜单的系统或窗口内界面。

**浮动格式工具栏**（floating toolbar）：位于正文区域的悬浮格式操作入口。

**覆盖层**（overlay）：不属于文档内容的浮动界面：通知、查找栏、快速打开、上下文菜单、块工具条、对话框。

**原生装饰**（native decorations）：操作系统绘制的窗口标题栏与边框（默认）。

**自绘标题栏**（custom title bar）：由应用绘制的可选窗口标题区。

**Primary 键**（Primary）：Windows / Linux 的 Ctrl，macOS 的 Command。

**安全模式**（safe mode）：用于限制外部可选能力的故障排查启动模式。

## 文档与语法

**文档**（document）：一份 Markdown 文本及其派生结构、文件属性与会话状态。

**权威文本**（authoritative text）：文档持久化所依据的正文。

**派生结构**（derived state）：从文档正文计算的块树、索引或呈现数据。

**块**（block）：文档的块级节点，例如段落、标题、列表、表格、代码块。

**叶块**（leaf block）：直接承载内联文本的块，例如段落、标题、表格单元格。

**容器块**（container block）：包含其他块的块，例如引用、列表、列表项、提示块。

**内联 token**（inline token）：叶块内的内联语法单元，例如强调、链接、行内代码、行内数学。

**标记**（marker / delimiter）：表达语法的字符序列，例如 `**`、`#`、`` ` ``、`$`。

**标记展开 / 折叠**（marker expansion / collapse）：同一语法的源码标记与排版形态之间的显示切换。

**具体语法树**（CST, concrete syntax tree）：保留 Markdown 标记及其原始文本区间的无损语法结构。

**节点 ID**（`NodeId`）：块在文档会话中的身份。

**提示块**（callout）：以 `[!NOTE]` 等类型行开头的引用块。

**文档元数据**（front matter）：文件开头由 `---` 包围的 YAML 块。

**文内目录**（TOC）：独占一行的 `[toc]` 生成的派生目录块。

**引用定义**（reference definition）：`[label]: url` 形式的链接定义。

**脚注**（footnote）：`[^label]` 引用与 `[^label]: 内容` 定义。

## 编辑模型

**标量偏移**（`ScalarOffset`）：以 Unicode 标量值计数的文本位置，是所有逻辑位置的单位。

**字节偏移**（`ByteOffset`）：以 UTF-8 字节计数的文本位置。

**UTF-16 偏移**（`Utf16Offset`）：以 UTF-16 码元计数的文本位置。

**字素簇**（grapheme cluster）：用户感知的字符单位。

**事务**（transaction）：一次原子文本修改：有序编辑集合、反向编辑、选择前后状态、来源与合并键。

**修订号**（`Revision`）：标识文档内容版本的序号，与重载世代区分。

**世代**（`Epoch`）：标识文档装载批次的身份，与内容修订区分。

**选择**（selection）：包含锚点、焦点、方向、表面与亲和性的逻辑选择。

**锚点 / 焦点**（anchor / focus）：选择的起点与当前端点；焦点可以位于锚点之前（反向选择）。

**亲和性**（affinity）：位置落在折叠标记或软换行边界时偏向前侧还是后侧。

**格式意图**（formatting intent）：没有选区时建立的待输入格式状态。

**脏状态**（dirty）：当前逻辑内容或文件输出属性与最后一次成功写入的快照不等价。

**保存快照**（saved snapshot）：最后一次成功写入磁盘的内容与文件属性。

**磁盘基线**（disk baseline）：打开或保存时记录的文件字节哈希、大小、修改时间与文件身份。

**专用编辑表面**（dedicated surface）：在代码块、数学源码或 HTML 块原位置编辑其源文本的独立输入表面。

**滚动锚点**（scroll anchor）：以"节点 + 文本偏移 + 视口偏移"表达的滚动位置，用于重排后保持可见内容不跳动。

**忙碌模式**（busy mode）：文档超过普通负载阈值时降低非必要渲染频率的运行模式。

## 运行时与持久化

**任务**（job）：属于某文档、视图或操作的异步工作单元。

**过期结果**（stale result）：所属文档、版本或目标已经失效的任务结果。

**屏障**（barrier）：保存或导出前，等待相关文本提交或资源就绪的同步边界。

**派生失效**（invalidation）：文本修改后标记需要重算的派生结构范围。

**文档会话**（document session）：进程中共享文档状态的实体，区别于显示它的窗口。

**视图**（view）：某文档会话的显示入口及其选择、滚动状态。

**镜像窗口**（mirror）：显示同一文档会话但当前未持有编辑租约的视图。

**编辑租约**（edit lease）：标识文档当前编辑所有者的资格。

**恢复点**（recovery point）：写入恢复区的完整文本与文件属性快照。

**恢复中心**（recovery center）：浏览、比较、恢复与删除恢复记录的界面。

**原子保存**（atomic save）：完整新内容准备成功后一次性替换旧目标的保存方式。

**外部冲突**（external conflict）：文档有未保存修改时磁盘文件被其他程序修改的状态。

**工作区**（workspace）：用户选择的一个本地目录，作为侧栏文件树的根。

**便携文档容器**（text bundle）：将 Markdown 与附件组织在一起的 `.textbundle` 或 `.textpack` 便携文档。

## 主题与呈现

**主题族**（theme family）：一个主题文件中的一组主题，例如"墨"族的宣纸与墨池。

**外观**（appearance）：主题的明暗类别：`light` 或 `dark`。

**外观模式**（appearance mode）：主题选择方式：跟随系统、固定浅色或固定深色。

**宣纸 / 墨池**（Moino Paper / Moino Inkstone）：内置墨绿主题族的浅色与深色主题。

**颜色 token**（color token）：表示界面或正文颜色语义的命名键，例如 `editor.background`。

**内容 token**（content token）：表示正文排版与配色语义的命名键。

**主题覆盖**（theme overrides）：对主题 token 的局部修改值。

**高对比覆盖层**（high-contrast overlay）：在主题之上按对比度规则计算的覆盖值。

**主题投影**（theme projection）：把有效主题转换为界面与正文呈现所需样式的过程。

**块投影**（block projection）：把文档块与内联语法转换为可排版视图结构的过程。

## 导出

**语义中间表示**（semantic IR）：从冻结修订生成、不含运行期状态的导出用文档结构。

**语义文档 JSON**（`moino-document`）：语义中间表示的 JSON 序列化格式。

**导出快照**（export snapshot）：导出任务冻结的文档修订及其资源状态。

**导出配置**（export profile）：一组命名的导出参数（格式、主题、资源策略、纸张等）。

**资源策略**（resource policy）：导出时对图片等资源的处理：保留链接、复制附件、单文件嵌入。
