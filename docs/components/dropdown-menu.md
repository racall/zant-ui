# 下拉菜单 `DropdownMenu`

```tsx
import { Button, DismissLayer, DropdownMenu, View, useState } from "zant";

export default function App() {
  const [open, setOpen] = useState(false);
  return <View>
    <Button onClick={() => setOpen(!open)}>菜单</Button>
    {open && <DismissLayer onDismiss={() => setOpen(false)} />}
    {open && <View absolute top={36} left={0}>
      <DropdownMenu key="file-menu" items={[
        { kind: "group", label: "文件" },
        { label: "打开", icon: "folder", onClick: () => setOpen(false) },
        { kind: "divider" },
        { label: "删除", icon: "trash", danger: true, onClick: () => setOpen(false) },
      ]} />
    </View>}
  </View>;
}
```

示例用绝对定位的 `View` 把菜单放在按钮下方。菜单不会自动定位、翻转或关闭，关闭由条目回调完成。
可移动布局应把弹层放在根容器，通过 Rust `ui::key_bounds(触发控件的 key)` 读取上一帧位置，
结合菜单高度与窗口高度决定放在按钮上方或下方；TSX 暂无 `keyBounds`。

| 属性 | 默认 / 说明 |
|---|---|
| `items` | 菜单项数组（必填） |
| `key`、`width`、`fontSize` | `"dropdown-menu"`、220、14 |
| `maxHeight` | 默认不限制；设置后可纵向滚动 |
| `background`、`textColor`、`extraColor`、`borderColor` | 默认主题色 |
| `motion`、`motionOrigin` | `"slide_up"`、`"top_right"`；可用值见 [动效](../motion.md) |

条目：`kind` 默认 `"item"`（另有 `"divider"` / `"group"`）；`key`、`label`、`icon`、`extra`（右侧文字，快捷键用 `keyLabel`）；
`disabled` / `danger` / `selected` 默认 false，`indent` 默认 0（每级 12px）；
`onClick` 或优先的 `indexAction` + `index`（默认 0）；`content` 可替代图标与文字，行高仍为 32。
面板内边距 4、圆角 8，默认阻挡空白处点击穿透；选中底色为 `colorPrimaryBg`。

Rust：`ui::dropdown_menu(ui::MenuOptions { items, .. })`，条目为 `ui::MenuItem`。
