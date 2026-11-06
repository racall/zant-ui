# 分段控制器 Segmented

antd `Segmented`：`color_bg_layout` 轨道，选中项是浮起的 `color_bg_elevated` 滑块，
切换时滑块以 300ms ease-in-out 平滑滑到新位置（anchor + FLIP）。

```zig
ui.segmented(.{
    .key = "mode",
    .options = &.{
        .{ .icon = .pencil, .label = "编辑" },
        .{ .icon = .columns, .label = "分栏", .tooltip = "分栏 " ++ zant.keys.label("mod+\\") },
        .{ .icon = .book_open, .label = "预览" },
    },
    .selected = state.mode,
    .on_change = ui.indexAction(state, State.onMode),
    .height = 28,         // antd: 28 small / 32 middle
    .item_width = 72,     // 可选，固定每项宽度
})
```

`SegmentedOption`：`label`、`icon`、`tooltip`（只有图标时必填）。其余选项：`key`、
`font_size`（默认 13）。

核心视图切换建议“图标 + 文字”，保证不用猜就知道去哪（见 DESIGN.md）。
