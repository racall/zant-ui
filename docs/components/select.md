# 下拉选择

```zig
ui.select(.{
    .options = &.{ "Starter", "Pro", "Enterprise" },
    .selected = self.plan,
    .on_change = ui.indexAction(self, State.setPlan),
})
```

## 选项

`key`、`options`（`[]const []const u8`）、`selected`、`on_change`、`disabled`、
`width`、`height`（默认 32）、`tooltip`。

弹出层会根据视口自动上翻/下翻；支持点击外部关闭。当前没有搜索和 typeahead。
