# 菜单与对话框

macOS 使用系统菜单栏；Windows 与 Linux 使用 GPUI Kit `AppMenuBar`，Alt 或 F10 显示，支持方向键、首字母和 Escape 逐层退出。状态栏左端的按钮打开应用菜单面板，帮助与首次使用提示说明隐藏入口。

菜单显示与标题区占用见[布局](../design/layout-geometry.md#窗口装饰与菜单)。所有项来自命令注册表，调用边界见[架构](../architecture.md#关键接缝)。
