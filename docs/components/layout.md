# 布局

```zig
ui.column(.{
    .gap = 12,
    .padding = ui.inset(24),
    .align_items = .center,
    .children = &.{
        ui.text("A", .{}),
        ui.row(.{ .gap = 8, .children = &.{ ui.text("B", .{}), ui.text("C", .{}) } }),
    },
})
```

## 选项

`key`、`gap`、`row_gap`、`column_gap`、`padding`、`margin`、`justify`、
`align_items`、`wrap`、`align_content`、`align_self`、`order`、`width`、`height`、
`width_percent`、`height_percent`、`min_width`、`min_height`、`max_width`、
`max_height`、`grow`、`shrink`、`basis`、`absolute`、`top`、`right`、`bottom`、
`left`、`background`、`border_color`、`border_width`、`radius`、`clip`、`scroll`、
`block_pointer`、`children`；CSS 写法的别名 `justify_content`、`flex_direction`。

还有这些容器选项：

- `selectable`：里面的文字可以拖选、双击选词、三击选段并 ⌘C / Ctrl+C 复制；`unselectable` 把子树排除在选择之外；
- `hover_background` / `hover_border_color`：指针在容器上时的背景 / 边框（200ms 过渡，不重建），列表行、卡片用；
  `show_on_hover`：只在最近的带悬停样式的祖先被悬停时显示（行内的删除等次要按钮，见 [Clickable](clickable.md)）；
- `focus_scope`：Tab / Shift+Tab 的焦点只在这个容器里循环（对话框、弹出面板；`ui.modal` 自带）；
- `on_resize`：容器尺寸变化（包括第一次布局）后调用，用 `ui.keyBounds(key)` 读新尺寸，做随宽度变化的布局。

`column` 主轴为纵向，`row` 主轴为横向。`scroll = true` 为纵向滚动容器（需要确定高度），
`block_pointer = true` 吞掉空白处点击（弹层面板）。`ui.space` 是 antd `Space`（默认横向、
`gap` 8、居中对齐；只给 `width`/`height` 时可当固定占位），`ui.divider(.{ .margin, .color })`
是 1px 分隔线。`anchor` 等更多 CSS 对应关系见 [Flex 与 CSS](../flex.md)。
