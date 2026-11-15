# 快捷键列表 ShortcutList

```tsx
import { ShortcutList, keyLabel } from "zant";

export default function App() {
  return <ShortcutList columns={1} groups={[
    { title: "文件", items: [
      { label: "保存", keys: keyLabel("mod+s") },
      { label: "另存为", keys: keyLabel("mod+shift+s") },
    ] },
  ]} />;
}
```

| 属性 | 默认值 / 说明 |
|---|---|
| `groups` | TSX 必填；`{ title, items: { label, keys }[] }[]`，各字段必填 |
| `columns` | 2，限制在 1–8；按组分配到当前最短的列 |
| `columnWidth` | macOS 250，Windows 310 |
| `columnGap`、`rowHeight`、`fontSize` | 40、28、13 |
| `key` | `"shortcuts"` |

只展示快捷键，不注册按键处理。`keyLabel` 生成平台文字（如 ⇧⌘S / Ctrl+Shift+S）；可作为 [Modal](modal.md) 的内容，注意列表总宽度。
Rust：`ui::shortcut_list`，使用 `ui::ShortcutListOptions` / `ui::ShortcutGroup` / `ui::Shortcut`。
