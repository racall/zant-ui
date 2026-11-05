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

Hover 或键盘聚焦后显示；提示层绘制在窗口最上层。
