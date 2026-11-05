# 链接

```zig
ui.link(.{
    .text = "文档",
    .on_click = ui.action(self, State.openDocs),
})
```

## 选项

`text`、`on_click`、`key`。

视觉上使用链接色；行为完全由 `on_click` 决定，不会自动打开浏览器。
