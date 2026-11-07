# 图标 Icon / IconButton

Zant 自带一套线性图标（24×24 网格、统一线宽，Lucide 风格），矢量绘制，任意尺寸清晰，
颜色默认跟随主题。不要用文字字符（☰、⋯）冒充图标。

```zig
ui.icon(.{ .name = .search, .size = 16 })

// 方形图标按钮（antd 文字按钮的悬停/按下底色）
ui.iconButton(.{
    .key = "find",
    .icon = .search,
    .size = 17,          // 图标尺寸
    .box = 30,           // 按钮边长
    .tooltip = "查找 " ++ zant.keys.label("mod+f"), // ⌘F / Ctrl+F
    .active = state.search_open,   // 选中态底色
    .on_click = ui.action(state, State.toggleSearch),
})
```

`iconButton` 默认不抢焦点（工具栏按钮不应把焦点从输入框拿走），`focusable = true` 可改。

图标名：`menu list search eye eye_off more more_vertical close plus minus check
chevron_down/up/left/right arrow_down/up file file_text folder save copy download upload
link external_link pencil trash undo redo refresh settings sun moon columns keyboard book_open
maximize minimize focus type_size heading bold italic strikethrough code code_block quote
list_bullet list_ordered list_checks table sigma image minus_rule info alert lightbulb
clipboard monitor wifi wifi_off clock power`；
`window_minimize window_maximize window_restore window_close` 是 Windows 标题栏按钮用的
10px 细线字形（见 [window-controls.md](window-controls.md)）。

## 选项

`icon`：`key`、`name`、`size`（默认 18）、`color`、`stroke`（线宽，默认随尺寸缩放）。
`iconButton`：`key`、`icon`、`size`、`box`（默认 32）、`color`、`active`、
`active_background`、`disabled`、`focusable`、`tooltip`、`on_click`、`index_action`、`index`。
新增图标在 `src/core/icons.zig` 中按同样的 24×24 网格和线宽绘制。
