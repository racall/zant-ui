# 文本

```zig
ui.text("Hello", .{ .size = 14, .weight = .regular, .color = 0xff1f1f1f })
ui.heading("Settings", .{ .level = 1 })
ui.paragraph("长文本会自动换行。", .{ .size = 14 })
ui.textFmt("Count: {d}", .{count}, .{ .color = 0xa6000000 })
```

## 选项

`key`、`size`、`weight`（`.thin` / `.extra_light` / `.light` / `.regular` / `.medium` / `.semibold` / `.bold` / `.extra_bold` / `.black`）、
`color`（ARGB）、`text_align`（`.left` / `.center` / `.right`）、`wrap`、
`line_height`、`max_lines`、`family`（`.sans` / `.serif` / `.mono` / `.serif_cjk` /
`.math`）。

`heading`（别名 `title`）支持 `level` 1–5（30/24/20/16/14px）及 `color`、`weight`、
`text_align`；`paragraph` 默认自动换行、行高 1.5、使用次要文字色（`size`、`color`、
`text_align`、`line_height`）。
文本由 Skia Paragraph 排版，支持中英文和系统字体回退。
