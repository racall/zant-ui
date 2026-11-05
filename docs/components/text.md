# 文本

```zig
ui.text("Hello", .{ .size = 14, .weight = .regular, .color = 0xff1f1f1f })
ui.heading("Settings", .{ .level = 1 })
ui.paragraph("长文本会自动换行。", .{ .size = 14 })
ui.textFmt("Count: {d}", .{count}, .{ .color = 0xa6000000 })
```

## 选项

`key`、`size`、`weight`（`.regular` / `.medium` / `.semibold` / `.bold`）、
`color`（ARGB）、`text_align`（`.left` / `.center` / `.right`）、`wrap`、
`line_height`、`max_lines`。

`heading` 支持 `level` 1–5；`paragraph` 默认自动换行并使用次要文字色。
文本由 Skia Paragraph 排版，支持中英文和系统字体回退。
