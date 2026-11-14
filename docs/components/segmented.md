# 分段控制器 Segmented

antd `Segmented`：`colorBgLayout` 轨道，选中项是浮起的 `colorBgElevated` 滑块，
切换时滑块以 300ms ease-in-out 平滑滑到新位置（anchor + FLIP）。

```tsx
<Segmented
  key="mode"
  options={[
    { icon: "pencil" },
    { icon: "columns", tooltip: `分栏 ${keyLabel("mod+\\")}` },
    { icon: "book_open" },
  ]}
  selected={mode}
  onChange={setMode}
  height={28}         // antd: 28 small / 32 middle
  itemWidth={72}      // 可选，固定每项宽度
/>
```

`options` 每项：`label`、`icon`、`tooltip`（只有图标时必填）。其余选项：`key`、
`fontSize`（默认 13）、`block`。手写 Rust：`ui::segmented(ui::SegmentedOptions { .. })`。

核心视图切换建议“图标 + 文字”，保证不用猜就知道去哪（见 DESIGN.md）。
