# 标题栏按钮 WindowControls（Windows）

窗口风格为 `hidden_titlebar` 时，Windows 上由运行时自动在根节点右上角叠加
Windows 11 风格的最小化 / 最大化（还原）/ 关闭按钮；macOS 保留系统红绿灯。应用不需要
自己放这个组件，只要给它留出位置：

```zig
const insets = ui.titlebarInsets(); // macOS: left = 红绿灯宽度；Windows: right = 3 × 46
ui.row(.{
    .height = 52,
    .padding = .{ .left = 16 + insets.left, .right = 16 + insets.right },
    .children = &.{ ... },
})
```

`window.titlebar_height` 设为自绘标题栏的高度，按钮会填满这个高度（未设置时为 32）。

## 行为

- 尺寸：46 px 宽、10 px 线宽 1 px 的图标；悬停浅色底，关闭按钮悬停红底（#C42B1C）白色图标；
  窗口失去焦点时图标变淡；深 / 浅色跟随主题。
- 最大化按钮向系统报告为 `HTMAXBUTTON`，Windows 11 悬停时显示贴靠布局浮层。
- 标题栏其余空白处可拖动窗口、双击最大化，交互控件仍可点击（`ZantHost.titlebar_hit`）。
- 关闭按钮走 `onCloseRequest`，可以否决（例如询问是否保存）。

## 手动使用

`ui.windowControls(.{ .height, .maximized, .active, .on_minimize, .on_maximize, .on_close })`
返回同样的按钮组（绝对定位在父容器右上角），用于需要自行组合的场景。
