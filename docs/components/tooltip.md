# 提示

```zig
ui.tooltip(.{
    .text = "复制链接",
    .child = ui.button(.{
        .text = "复制",
        .on_click = ui.action(self, State.copy),
    }),
})
```

## 选项

`text`、`child`。

悬停约 450ms 后显示在目标下方（zoom 动效），放不下时翻到上方；提示层绘制在窗口最上层。
`ui.image`、`ui.select`、`ui.iconButton` 和 `SegmentedOption` 也有 `tooltip` 字段。
提示里的快捷键用 `zant.keys.label("mod+c")` 生成，不要写死 ⌘。
