# 标题栏按钮 WindowControls（Windows）

```tsx
import { titlebarInsets, Row, Text } from "zant";

export default function App() {
  const insets = titlebarInsets();
  return <Row height={52} padding={{ left: 16 + insets.left, right: 16 + insets.right }}>
    <Text>我的应用</Text>
  </Row>;
}
```

在 `app.config.json` 中设置：

```json
{ "window": { "style": "hidden-titlebar", "titlebar_height": 52 } }
```

Windows 运行时自动添加最小化 / 最大化（还原）/ 关闭按钮，应用只需留位；macOS 使用系统红绿灯。TSX 没有 `WindowControls` 标签。

- `titlebarInsets()`：隐藏标题栏时 macOS 左侧 84、Windows 右侧 138；其他风格两侧为 0。
- `window.titlebar_height` 决定按钮高度（未设置或非正数时 Windows 用 32）；`macos.titlebar_height` / `windows.titlebar_height` 可分别覆盖。TSX `createWindow` 选项写作 `titlebarHeight`。
- 每个按钮宽 46；支持深浅主题及窗口失焦状态。Windows 11 最大化按钮支持系统贴靠布局。
- 标题栏空白处可拖动窗口、双击最大化；关闭走 `useCloseRequest`，回调可否决关闭。

手动 Rust 接口：`ui::window_controls(ui::WindowControlsOptions { .. })`，定位在父容器右上角。
选项：`height=32`、`maximized=false`、`active=true`，以及默认未设的 `on_minimize` / `on_maximize` / `on_close`（用 `ui::action`）。
