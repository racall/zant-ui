# 可点击区域 `ui.clickable` 与悬停显示 `show_on_hover`

## `ui.clickable`

整块区域可点击的容器（列表行、卡片），里面的按钮照常接收自己的点击：

```zig
ui.clickable(.{
    .box = .{
        .flex_direction = .row,
        .padding = ui.insetXY(12, 10),
        .radius = 10,
        .hover_background = theme.color_fill_tertiary,
        .children = &.{ icon, content, ui.row(.{ .show_on_hover = true, .children = &.{delete_button} }) },
    },
    .index_action = ui.indexAction(self, State.openRow),
    .index = i,
})
```

选项：`box`（容器的 `BoxOptions`）、`on_click` 或 `index_action` + `index`、`cursor`（默认手形）。
按下后移出区域再松开不算点击。

## `BoxOptions.show_on_hover`

只在最近的带悬停样式（`hover_background` / `hover_border_color`）的祖先被悬停时绘制，随悬停过渡
淡入淡出：行内的删除、更多等次要操作不必在每一行都显示（CSS `.row:hover .actions`）。
选中的行可以直接设为 `false` 让操作常显。
