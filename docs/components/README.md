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
| PDF 阅读器（`zant.pdf.view`）/ 单页预览 | [pdf.md](pdf.md) |
| 指针区域（`ui.pointerArea`）/ 悬停样式 / 焦点范围 / 内存图片 | [pointer-area.md](pointer-area.md) |
| 虚拟列表（`ui.list`） | [list.md](list.md) |
| 可点击区域（`ui.clickable`）/ 悬停显示（`show_on_hover`） | [clickable.md](clickable.md) |
| 快捷键录入（`ui.hotkeyInput`） | [hotkey-input.md](hotkey-input.md) |
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

## 其他 `ui.*` 函数

| 函数 | 说明 |
|---|---|
| `ui.richText(.{ .spans, .line_height, .on_link })` | 富文本：一段里混排不同样式、链接（`RichSpan.link` + `on_link`）、行内绘制（`RichSpan.embed`）；`line_height` 默认 2.0，0 用字体自身行高 |
| `ui.vector(.{ .drawing, .width, .copy })` | 保留式矢量绘制（公式、图表）；尺寸取绘制内容，`copy` 是被选中复制时的文字 |
| `ui.memo(.{ .key, .hash }, ctx, build)` | 类似 React.memo：`hash` 不变时整棵子树跳过构建；`build` 要返回 `ui.column` / `ui.row`，`ctx` 和子树里回调的上下文要活过这一帧 |
| `ui.withKey(node, key)` | 给一个已经建好的节点设置 key（为 memo 子树分配 key） |
| `ui.focus(key)` | 下一次布局后让这个 key 的控件获得键盘焦点（例如打开时聚焦搜索框） |
| `ui.scrollIntoView(key)` | 滚动最近的滚动容器，把这个控件滚到顶部（有动效，关闭动效时直接跳） |
| `ui.scrollToEnd(key)` | 下一次布局后把这个滚动容器滚到底（无动效，聊天跟随新内容） |
| `ui.keyBounds(key)` | 上一次布局中这个 key 的位置和尺寸，浮层跟随控件定位用 |

自定义组件示例见生成项目的 `src/components/stat_card.zig`，以及
[窗口配置](../window-config.md)。

更多主题：[桌面集成：多窗口、托盘、全局快捷键、剪贴板](../desktop.md)、[SQLite 持久化](../database.md)、[Flex 与 CSS](../flex.md)、[动效](../motion.md)、
[Markdown 与数学](../markdown.md)。所有组件外观以 Ant Design 5 为准，界面设计遵循
[DESIGN.md](../../DESIGN.md)。
