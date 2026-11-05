# 下拉菜单 Dropdown Menu

antd `Dropdown` 菜单：8px 圆角浮层、4px 内边距、32px 菜单项，支持图标、右侧快捷键、
分组标题、分隔线、选中、禁用、危险项和自定义行内容。默认带 antd slide-up 进出场动效。

```zig
if (state.menu_open) {
    // 点击外部任意位置关闭（放在菜单前面）
    children.append(ui.dismissLayer(.{ .on_dismiss = ui.action(state, State.closeMenu) }));
    children.append(ui.styled(ui.dropdownMenu(.{
        .key = "menu",
        .width = 240,
        .items = &.{
            ui.MenuItem.group("文件"),
            .{ .key = "m-open", .label = "打开…", .icon = .folder, .extra = "⌘O", .on_click = ui.action(state, State.open) },
            ui.MenuItem.divider,
            .{ .key = "m-del", .label = "删除", .icon = .trash, .danger = true, .on_click = ui.action(state, State.remove) },
        },
    }), .{ .absolute = true, .top = 46, .right = 16 }));
}
```

- `MenuItem.selected` 用 `color_primary_bg` 高亮当前项（如大纲中的当前章节）。
- `max_height` 设置后内容可滚动；`motion` / `motion_origin` 可改进出场动效与原点。
- 菜单面板 `block_pointer`：点在内边距、分隔线、禁用项上不会穿透关闭。
