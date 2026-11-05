# 快捷键列表 ShortcutList

分组的“操作 — 按键”列表，自动均衡到多列；通常作为 `ui.modal` 的内容（⌘/ 打开）。

```zig
const groups = [_]ui.ShortcutGroup{
    .{ .title = "文件", .items = &.{
        .{ .label = "保存", .keys = "⌘S" },
        .{ .label = "另存为…", .keys = "⇧⌘S" },
    } },
    .{ .title = "查找", .items = &.{
        .{ .label = "下一个 / 上一个", .keys = "↵  ⇧↵" },
    } },
};

ui.shortcutList(.{ .groups = &groups, .columns = 2, .column_width = 250 })
```

应用只写快捷键数据；列表样式与弹窗都来自框架，保证各应用一致。
