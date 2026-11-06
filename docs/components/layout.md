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
`block_pointer`、`children`。

`column` 主轴为纵向，`row` 主轴为横向。`scroll = true` 为纵向滚动容器（需要确定高度），
`block_pointer = true` 吞掉空白处点击（弹层面板）。`ui.space` 是 antd `Space`（默认横向、
`gap` 8、居中对齐；只给 `width`/`height` 时可当固定占位），`ui.divider(.{ .margin, .color })`
是 1px 分隔线。`anchor` 等更多 CSS 对应关系见 [Flex 与 CSS](../flex.md)。
