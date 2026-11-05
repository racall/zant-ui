# 标签

```zig
ui.tag(.{ .text = "Beta" })
ui.tag(.{ .text = "完成", .color = 0xff389e0d, .background = 0xfff6ffed })
```

## 选项

`key`、`text`、`color`（文字 ARGB）、`background`（背景 ARGB）。

用于状态标记；颜色由调用方直接指定。
