# 快捷键列表 ShortcutList

分组的“操作 — 按键”列表，自动均衡到多列；通常作为 `Modal` 的内容
（`keyLabel("mod+/")` 打开）。
按键文字用 `keyLabel` 生成，macOS 显示 ⇧⌘S，Windows 显示 Ctrl+Shift+S。

```tsx
const groups = [
  { title: "文件", items: [
    { label: "保存", keys: keyLabel("mod+s") },
    { label: "另存为…", keys: keyLabel("mod+shift+s") },
  ] },
  { title: "查找", items: [
    // 平台习惯不同时：macOS ⌥⌘F，Windows Ctrl+H —— 手写 Rust 用 keys::pick；TSX 用 isMacos
    { label: "替换", keys: isMacos ? keyLabel("alt+mod+f") : keyLabel("mod+h") },
    { label: "下一个 / 上一个", keys: keyLabel("enter shift+enter") },
  ] },
];

<ShortcutList groups={groups} columns={2} />   // 默认列宽 macOS 250，Windows 310
```

选项：`key`、`groups`（`{ title, items: { label, keys }[] }`）、`columns`（默认 2）、
`columnWidth`、`columnGap`（40）、`rowHeight`（28）、`fontSize`（13）。
手写 Rust：`ui::shortcut_list(ui::ShortcutListOptions { groups, .. })`，类型是
`ui::ShortcutGroup` / `ui::Shortcut`。

应用只写快捷键数据；列表样式与弹窗都来自框架，保证各应用一致。
