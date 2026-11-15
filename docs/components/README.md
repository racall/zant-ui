# 组件文档

推荐用 TSX 编写应用（编译为 Rust，没有 JS 运行时）。把下面内容放在 `src/App.tsx`：

```tsx
import { Button, View, useState } from "zant";

export default function App() {
  const [count, setCount] = useState(0);
  return <View padding={24}>
    <Button onClick={() => setCount(count + 1)}>点击：{count}</Button>
  </View>;
}
```

`src/` 只放 `.tsx` / `.ts`，Rust 放在 `native/`；生成项目已配置 `native/main.rs`（`zant::app!();`）与
`native/build.rs`（`fn main() { zant_build::tsx_app(); }`），入口与模块注册由构建器生成。
`zant run` 默认原地热刷新组件；配置变更重启，见 [脚手架与热更新](../scaffold.md)。

示例片段中的组件与 hooks 均从 `"zant"` 导入；状态声明放在函数组件中。
组件属性以 `crates/zant-tsx/zant.d.ts` 为准，通用覆盖见 [样式](style.md)。

更多主题：[TSX 与 hooks](../tsx.md)、[原生模块](../tsx-native.md)、[状态](../state.md)、
[窗口配置](../window-config.md)、[桌面集成](../desktop.md)、[动效](../motion.md)。
界面遵循 [DESIGN.md](../../DESIGN.md)，颜色通过 `useTheme()` 获取主题 token。

## 索引

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

交互示例：`./scripts/cargo.sh run -p zant-examples --bin zant-components`（`ZANT_DARK=1` 深色），源码在 `crates/zant-examples/src/components_app/`。
快捷键文字用 `keyLabel("mod+s")`，按键判断用 `e.mod`，适配 macOS / Windows。
