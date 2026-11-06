# 复选框

```zig
ui.checkbox(.{
    .label = "Notifications",
    .checked = self.notifications,
    .on_change = ui.boolAction(self, State.onNotify),
})
```

## 选项

`key`、`label`、`checked`、`on_change`、`disabled`、`font_size`。

点击标签或方框切换；Tab 聚焦，Space 激活。回调参数为 `bool`。
