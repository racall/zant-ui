# 文本

```tsx
<Text size={14} weight="regular" color={0xff1f1f1f}>Hello</Text>
<Heading level={1}>Settings</Heading>
<Paragraph size={14}>长文本会自动换行。</Paragraph>
<Text color={0xa6000000}>{`Count: ${count}`}</Text>
```

## 选项

`Text`：`key`、`size`、`weight`（`"thin"` / `"extra_light"` / `"light"` / `"regular"` / `"medium"` /
`"semibold"` / `"bold"` / `"extra_bold"` / `"black"`，或 `"100"`～`"900"` / `600`）、
`color`（ARGB）、`textAlign`（`"left"` / `"center"` / `"right"`）、`wrap`、
`lineHeight`、`maxLines`、`family`（`"sans"` / `"serif"` / `"mono"` / `"serif_cjk"` / `"math"`）。
手写 Rust：`ui::text(text, ui::TextOptions { .. })`；拼接 / 格式化用 `format!` 后 `zant::Str::copy(..)`
（或 `ui::frame_string(..)`），例如 `ui::text(format!("Count: {count}"), ..)`。

`Heading`（别名 `Title`）支持 `level` 1–5（30/24/20/16/14px）及 `color`、`weight`、`textAlign`；
`Paragraph` 默认自动换行、行高 1.5、使用次要文字色（`size`、`color`、`textAlign`、`lineHeight`）。
文本由 Skia Paragraph 排版，支持中英文和系统字体回退。
嵌套 `<Text>`（局部样式）编译成 `ui::rich_text`；公式 / 图表用 `ui::vector`（手写 Rust）。
