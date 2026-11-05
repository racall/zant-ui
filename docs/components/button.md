# 按钮

```zig
ui.button(.{
    .text = "Save Changes",
    .variant = .primary,
    .size = .middle,
    .on_click = ui.action(self, State.save),
})
```

## 选项

`key`、`text`、`variant`（`.primary` / `.default` / `.text` / `.link`）、
`size`（`.small` / `.middle` / `.large`）、`disabled`、`width`、
`on_click`、`index_action`、`index`。

Hover/Pressed 使用 160/100ms 过渡；只有指针在按钮内释放才触发点击。
Tab 聚焦，Space/Enter 激活；禁用状态忽略输入。
