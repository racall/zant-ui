# 下拉菜单 Dropdown Menu

antd `Dropdown` 菜单：8px 圆角浮层、4px 内边距、32px 菜单项，支持图标、右侧快捷键、
分组标题、分隔线、选中、禁用、危险项和自定义行内容。默认带 antd slide-up 进出场动效。

```zig
if (state.menu_open) {
    // 点击外部任意位置关闭（放在菜单前面）
    children.append(arena, ui.dismissLayer(.{ .on_dismiss = ui.action(state, State.closeMenu) })) catch {};
    children.append(arena, ui.styled(ui.dropdownMenu(.{
        .key = "menu",
        .width = 240,
        .items = &.{
            ui.MenuItem.group("文件"),
            .{ .key = "m-open", .label = "打开…", .icon = .folder, .extra = zant.keys.label("mod+o"), .on_click = ui.action(state, State.open) },
            ui.MenuItem.divider,
            .{ .key = "m-del", .label = "删除", .icon = .trash, .danger = true, .on_click = ui.action(state, State.remove) },
        },
    }), .{ .absolute = true, .top = 46, .right = 16 })) catch {};
}
```

- `MenuOptions`：`key`、`items`、`width`（默认 220）、`max_height`、`font_size`（14）、
  颜色覆盖 `background` / `text_color` / `extra_color` / `border_color`、`motion`、`motion_origin`。
- `MenuItem`：`key`、`label`、`icon`、`extra`（右侧快捷键，用 `zant.keys.label`）、`disabled`、
  `danger`、`selected`、`indent`（每级缩进 12px）、`on_click` 或 `index_action` + `index`、
  `content`（自定义行内容）；`MenuItem.divider`、`MenuItem.group("标题")`。
- `MenuItem.selected` 用 `color_primary_bg` 高亮当前项（如大纲中的当前章节）。
- `max_height` 设置后内容可滚动；`motion` / `motion_origin` 可改进出场动效与原点。
- 菜单面板 `block_pointer`：点在内边距、分隔线、禁用项上不会穿透关闭。

## 跟随触发按钮定位

弹层放在根容器里（这样 `ui.dismissLayer` 能盖住整个窗口），位置用 `ui.keyBounds(key)` 取
触发按钮上一帧的窗口坐标；下方放不下时翻到按钮上方（antd `bottomLeft` / `topLeft`）：

```zig
const measured = ui.keyBounds("menu");          // 菜单自身上一帧的尺寸
const menu_h: f32 = if (measured) |m| m.height else 160;
if (measured == null) ui.wakeAfter(1);           // 首帧用估计值，布局后再定位一次
if (ui.keyBounds("menu-btn")) |b| {
    const above = b.bottom() + 4 + menu_h > window_height - 8;
    const top = if (above) b.y - 4 - menu_h else b.bottom() + 4;
    children.append(arena, ui.dismissLayer(.{ .on_dismiss = ui.action(s, State.closeMenu) })) catch {};
    children.append(arena, ui.styled(ui.dropdownMenu(.{
        .key = "menu",
        .motion_origin = if (above) .bottom_left else .top_left,
        .items = items,
    }), .{ .absolute = true, .left = b.x, .top = top })) catch {};
}
```

完整示例见 `examples/components.zig`。
