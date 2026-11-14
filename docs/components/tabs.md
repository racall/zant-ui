# 标签页

```tsx
<Tabs
  items={[
    { label: "General", content: <GeneralView /> },
    { label: "Members", content: <MembersView /> },
  ]}
  active={tab}
  onChange={setTab}
/>
```

## 选项

`key`、`items`（`{ label, content }[]`）、`active`、`onChange`。
手写 Rust：`ui::tabs(ui::TabsOptions { items: vec![ui::TabItem { .. }], .. })`。

只挂载当前选中的内容；跨 Tab 保留的状态请放到 store（`createStore` / `useStore`）。
