# 点击外部关闭 DismissLayer

铺满父容器的透明点击层，放在弹出层（菜单、面板）**之前**：点击弹层外任意位置调用
`on_dismiss`。默认不绘制；传 `background` 可作为遮罩（`ui.modal` 内部就是这样用的）。

```zig
children.append(ui.dismissLayer(.{ .on_dismiss = ui.action(state, State.closeMenu) }));
children.append(menu);
```

弹层本身应设 `block_pointer = true`（`ui.dropdownMenu` 已默认），点在其空白处不会穿透。
