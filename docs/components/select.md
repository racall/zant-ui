# 下拉选择

```tsx
<Select options={["Starter", "Pro", "Enterprise"]} selected={plan} onChange={setPlan} />
```

## 选项

`key`、`options`（`string[]`）、`selected`、`onChange`、`disabled`、
`width`、`height`（默认 32）、`tooltip`。手写 Rust：`ui::select(ui::SelectOptions { .. })`。

弹出层会根据视口自动上翻/下翻；支持点击外部关闭。当前没有搜索和 typeahead。
