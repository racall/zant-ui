# 单选组

```zig
ui.radioGroup(.{
    .options = &.{
        .{ .label = "Basic" },
        .{ .label = "Pro" },
        .{ .label = "Enterprise", .disabled = true },
    },
    .selected = self.plan,
    .on_change = ui.indexAction(self, State.setPlan),
})
```

## 选项

`key`、`options`（`[]RadioOption`）、`selected`、`direction`、`gap`、
`on_change`。

`RadioOption` 包含 `label` 和 `disabled`。选中索引通过 `usize` 回调。
