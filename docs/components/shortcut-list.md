# 快捷键列表 ShortcutList

分组的“操作 — 按键”列表，自动均衡到多列；通常作为 `ui.modal` 的内容
（`zant.keys.label("mod+/")` 打开）。
按键文字用 `zant.keys.label` 生成，macOS 显示 ⇧⌘S，Windows 显示 Ctrl+Shift+S。

```zig
const groups = [_]ui.ShortcutGroup{
    .{ .title = "文件", .items = &.{
        .{ .label = "保存", .keys = zant.keys.label("mod+s") },
        .{ .label = "另存为…", .keys = zant.keys.label("mod+shift+s") },
    } },
    .{ .title = "查找", .items = &.{
        // 平台习惯不同时：macOS ⌥⌘F，Windows Ctrl+H
        .{ .label = "替换", .keys = zant.keys.pick(zant.keys.label("alt+mod+f"), zant.keys.label("mod+h")) },
        .{ .label = "下一个 / 上一个", .keys = zant.keys.label("enter shift+enter") },
    } },
};

ui.shortcutList(.{ .groups = &groups, .columns = 2 }) // 默认列宽 macOS 250，Windows 310
```

选项：`key`、`groups`（`ShortcutGroup{ title, items }`，`Shortcut{ label, keys }`）、
`columns`（默认 2）、`column_width`、`column_gap`（40）、`row_height`（28）、`font_size`（13）。

应用只写快捷键数据；列表样式与弹窗都来自框架，保证各应用一致。
