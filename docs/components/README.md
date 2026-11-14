# 组件文档

每个组件一份文档。应用里用 TSX 标签（PascalCase）；手写 Rust 时对应 `zant::ui` 的 builder（括号里注明）。
全部组件的可交互示例：`crates/zant-examples/src/components_app/`
（`cargo run -p zant-examples --bin zant-components`，`ZANT_DARK=1` 深色）。
macOS 与 Windows 共用同一套 API；快捷键文字一律用 `keyLabel("mod+s")`
（⌘S / Ctrl+S），判断命令键用 `e.mod`，不要写死 ⌘。

| 组件 | 文档 |
|---|---|
| 通用容器（`View`，类似 RN View / div；Rust `ui::view`） | [view.md](view.md) |
| 布局（`Row` / `Column` / `Space` / `Divider`） | [layout.md](layout.md) |
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
| PDF 阅读器（`PdfViewer`）/ 单页预览 | [pdf.md](pdf.md) |
| 指针区域（`PointerArea`）/ 悬停样式 / 焦点范围 / 内存图片 | [pointer-area.md](pointer-area.md) |
| 虚拟列表（`List`） | [list.md](list.md) |
| 可点击区域（`Clickable`）/ 悬停显示（`showOnHover`） | [clickable.md](clickable.md) |
| 快捷键录入（`HotkeyInput`） | [hotkey-input.md](hotkey-input.md) |
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

## 其他公共能力

| TSX / `zant` 导入 | 手写 Rust | 说明 |
|---|---|---|
| `<RichText spans lineHeight onLink>` | `ui::rich_text(RichTextOptions { … })` | 富文本：一段里混排不同样式、链接（`RichSpan::link` + `onLink`）、行内绘制（`RichSpan::embed`）；`lineHeight` 默认 2.0，0 用字体自身行高 |
| （无标签） | `ui::vector(VectorOptions { drawing, width, copy, … })` | 保留式矢量绘制（公式、图表）；尺寸取绘制内容，`copy` 是被选中复制时的文字 |
| （无标签） | `ui::memo(ui::MemoOptions { key, hash }, build)` | 类似 React.memo：`hash` 不变时整棵子树跳过构建；build 要返回容器，闭包捕获的上下文要活过这一帧（用 `Rc`） |
| 子组件里的 `useState` | `ui::local_state::<T>(id, init)` | 组件实例自己的状态（类似 React 的 `useState`）：第一次构建时用 init 创建，返回的 `Rc<RefCell<T>>` 在实例一直被构建期间不变；某次重建不再请求这个 `id` 时释放（卸载）。id 用 `ui::child_id(parent_id, key)` 从父实例的 id 和 key 派生；`ui::memo` 跳过的子树里的状态保留 |
| （无标签） | `ui::with_key(node, key)` | 给一个已经建好的节点设置 key（为 memo 子树分配 key） |
| （无标签） | `ui::focus(key)` / `ui::scroll_into_view(key)` / `ui::scroll_to_end(key)` | 下一次布局后聚焦 / 滚到顶部 / 滚到底（`scroll_to_end` 无动效，聊天跟随新内容） |
| （无标签） | `ui::key_bounds(key)` | 上一次布局中这个 key 的位置和尺寸，浮层跟随控件定位用 |
| `useState` / `useEffect` / `useMemo` / `useRef` | `ui::effect` / `ui::request_rebuild` / `tsx::hooks` | 状态、副作用与缓存 |

自定义组件示例见生成项目的 `src/components/`（`Dashboard.tsx`、`StatCard.tsx`、`CounterCard.tsx`），
以及 [窗口配置](../window-config.md)。

更多主题：[桌面集成：多窗口、托盘、全局快捷键、剪贴板](../desktop.md)、[SQLite 持久化](../database.md)、
[Flex 与 CSS](../flex.md)、[动效](../motion.md)、[Markdown 与数学](../markdown.md)、
[TSX 语法与 hooks](../tsx.md)。所有组件外观以 Ant Design 5 为准，界面设计遵循
[DESIGN.md](../../DESIGN.md)。
