# 头像

```zig
ui.avatar(.{ .text = "XL", .size = 40 })
ui.avatar(.{ .text = "A", .size = 36, .background = 0xff1677ff, .color = 0xffffffff })
```

## 选项

`key`、`text`、`size`、`background`、`color`。

显示文字头像；不加载图片。默认浅主色底 + 主色文字；只设 `background` 时文字默认白色。
