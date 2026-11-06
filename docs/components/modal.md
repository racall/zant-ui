# 对话框 Modal

antd `Modal`：半透明遮罩（`rgba(0,0,0,.45)`）淡入，卡片以 zoom 动效弹出；标题、关闭按钮、
内容与可选的底部按钮。点击遮罩（`mask_closable`）或 ✕ 调用 `on_close`；Esc 由应用的按键
处理调用同一个关闭动作。打开时作为窗口根的最后一个子节点渲染。

```zig
if (state.help_open) children.append(ui.modal(.{
    .key = "help",
    .title = "键盘快捷键",
    .width = null,                      // null = 按内容；默认 520
    .on_close = ui.action(state, State.closeHelp),
    .content = ui.shortcutList(.{ .groups = &shortcut_groups }),
    .footer = ui.button(.{ .text = "知道了", .variant = .primary, .on_click = ui.action(state, State.closeHelp) }),
}));
```

选项：`key`、`title`、`content`、`footer`（null 无底栏）、`width`（默认 520，null 按内容）、
`on_close`、`closable`（右上角关闭按钮）、`mask_closable`、`mask`（遮罩 ARGB，默认 `0x73000000`）。
