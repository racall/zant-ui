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

`key`、`gap`、`padding`、`margin`、`justify`、`align_items`、`width`、
`height`、`min_width`、`min_height`、`max_width`、`max_height`、`grow`、
`shrink`、`basis`、`background`、`border_color`、`border_width`、`radius`、
`clip`、`children`。

`column` 主轴为纵向，`row` 主轴为横向。`space` 是固定间距占位，`divider`
是分隔线。当前只支持单行 Flex，不支持 wrap 和百分比。
