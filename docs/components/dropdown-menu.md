# 下拉菜单 Dropdown Menu

antd `Dropdown` 菜单：8px 圆角浮层、4px 内边距、32px 菜单项，支持图标、右侧快捷键、
分组标题、分隔线、选中、禁用、危险项和自定义行内容。默认带 antd slide-up 进出场动效。

```tsx
{menuOpen && (
  <>
    {/* 点击外部任意位置关闭（放在菜单前面） */}
    <DismissLayer onDismiss={() => setMenuOpen(false)} />
    <DropdownMenu
      key="menu"
      width={240}
      absolute
      top={46}
      right={16}
      items={[
        { kind: "group", label: "文件" },
        { key: "m-open", label: "打开…", icon: "folder", extra: keyLabel("mod+o"), onClick: open },
        { kind: "divider" },
        { key: "m-del", label: "删除", icon: "trash", danger: true, onClick: remove },
      ]}
    />
  </>
)}
```

- `DropdownMenu`：`key`、`items`、`width`（默认 220）、`maxHeight`、`fontSize`（14）、
  颜色覆盖 `background` / `textColor` / `extraColor` / `borderColor`、`motion`、`motionOrigin`，
  以及容器的定位属性（`absolute`、`left`、`top` …）。
- `items` 的项：`key`、`label`、`icon`、`extra`（右侧快捷键，用 `keyLabel`）、`disabled`、
  `danger`、`selected`、`indent`（每级缩进 12px）、`onClick` 或 `indexAction` + `index`、
  `content`（自定义行内容）；另有 `kind: "divider"` / `kind: "group"` 两种项。
- `selected` 用 `colorPrimaryBg` 高亮当前项（如大纲中的当前章节）。
- `maxHeight` 设置后内容可滚动；`motion` / `motionOrigin` 可改进出场动效与原点。
- 菜单面板 `blockPointer`：点在内边距、分隔线、禁用项上不会穿透关闭。

手写 Rust：`ui::dropdown_menu(ui::MenuOptions { items, .. })`，项是 `ui::MenuItem`。

## 跟随触发按钮定位

弹层放在根容器里（这样 `DismissLayer` 能盖住整个窗口），位置用 `ui::key_bounds(key)` 取
触发按钮上一帧的窗口坐标；下方放不下时翻到按钮上方（antd `bottomLeft` / `topLeft`）：

```rust
let measured = ui::key_bounds("menu");          // 菜单自身上一帧的尺寸
let menu_h = measured.map(|m| m.height).unwrap_or(160.0);
if measured.is_none() { ui::wake_after(1); }     // 首帧用估计值，布局后再定位一次
if let Some(b) = ui::key_bounds("menu-btn") {
    let above = b.bottom() + 4.0 + menu_h > window_height - 8.0;
    let top = if above { b.y - 4.0 - menu_h } else { b.bottom() + 4.0 };
    // DismissLayer + DropdownMenu(absolute=true, left=b.x, top, motion_origin=…)
}
```

TSX 里没有 `keyBounds`，同样的定位在 `View` 上用 `absolute` + 固定坐标或 `onResize` 组合实现；
完整示例见 `crates/zant-examples/src/components_app/`。
