# 标签页

```zig
ui.tabs(.{
    .items = &.{
        .{ .label = "General", .content = generalView(self) },
        .{ .label = "Members", .content = membersView(self) },
    },
    .active = self.tab,
    .on_change = ui.indexAction(self, State.setTab),
})
```

## 选项

`key`、`items`（`[]TabItem`）、`active`、`on_change`。

`TabItem` 包含 `label` 和 `content`。只挂载当前选中的内容；跨 Tab 保留的
状态请放到 Store。
