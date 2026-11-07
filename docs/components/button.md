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
`size`（`.small` / `.middle` / `.large`，高 24/32/40）、`disabled`、`width`、
`font_size`、`text_align`、`label_padding`、`text_color`、`icon`（`ui.Icon`，见
[icon.md](icon.md)）、`icon_size`、`focusable`（`false` 时点击不抢焦点）、
`on_click`、`index_action`、`index`。

Hover/Pressed 使用 200/100ms 过渡（antd motionDurationMid / Fast）；只有指针在按钮内释放才触发点击。
Tab 聚焦，Space/Enter 激活；禁用状态忽略输入。
