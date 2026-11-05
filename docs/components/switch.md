# 开关

```zig
ui.@"switch"(.{
    .checked = self.enabled,
    .on_change = ui.boolAction(self, State.toggle),
})
```

## 选项

`key`、`checked`、`on_change`、`disabled`。

`switch` 是 Zig 关键字，因此 API 写作 `ui.@"switch"`。
