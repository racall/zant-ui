# 组件文档

每个组件一份文档，API 均为声明式 `ui.*` builder。全部组件的可交互示例：
`examples/components.zig`（`zig build run-components`，`ZANT_DARK=1` 深色）。
macOS 与 Windows 共用同一套 API；快捷键文字一律用 `zant.keys.label("mod+s")`
（⌘S / Ctrl+S），判断命令键用 `key.modifiers.primary()`，不要写死 ⌘。

| 组件 | 文档 |
|---|---|
| 布局 | [layout.md](layout.md) |
| 文本、标题、段落 | [text.md](text.md) |
| 按钮、链接 | [button.md](button.md), [link.md](link.md) |
| 输入框 | [input.md](input.md) |
| 复选框 | [checkbox.md](checkbox.md) |
| 开关 | [switch.md](switch.md) |
| 单选组 | [radio-group.md](radio-group.md) |
| 下拉选择 | [select.md](select.md) |
| 标签页 | [tabs.md](tabs.md) |
| 提示 | [tooltip.md](tooltip.md) |
| 图片 | [image.md](image.md) |
| 卡片 | [card.md](card.md) |
| 标签 | [tag.md](tag.md) |
| 警告 | [alert.md](alert.md) |
| 头像 | [avatar.md](avatar.md) |
| 进度条 | [progress.md](progress.md) |
| PDF 预览 | [pdf.md](pdf.md) |
| 图标、图标按钮 | [icon.md](icon.md) |
| 下拉菜单 | [dropdown-menu.md](dropdown-menu.md) |
| 分段控制器 | [segmented.md](segmented.md) |
| 对话框 | [modal.md](modal.md) |
| 全局提示 | [message.md](message.md) |
| 快捷键列表 | [shortcut-list.md](shortcut-list.md) |
| 点击外部关闭 | [dismiss-layer.md](dismiss-layer.md) |
| 标题栏按钮 | [window-controls.md](window-controls.md) |
| Markdown 编辑器/预览 | [markdown.md](markdown.md) |
| 样式覆盖 | [style.md](style.md) |

自定义组件示例见生成项目的 `src/components/stat_card.zig`，以及
[窗口配置](../window-config.md)。

更多主题：[SQLite 持久化](../database.md)、[Flex 与 CSS](../flex.md)、[动效](../motion.md)、
[Markdown 与数学](../markdown.md)。所有组件外观以 Ant Design 5 为准，界面设计遵循
[DESIGN.md](../../DESIGN.md)。
