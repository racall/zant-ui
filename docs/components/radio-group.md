# 单选组

```tsx
<RadioGroup
  options={[
    { label: "Basic" },
    { label: "Pro" },
    { label: "Enterprise", disabled: true },
  ]}
  selected={plan}
  onChange={setPlan}
/>
```

## 选项

`key`、`options`（`{ label, disabled }[]`）、`selected`、`direction`、`gap`、
`onChange`。手写 Rust：`ui::radio_group(ui::RadioGroupOptions { .. })`，回调用 `ui::index_action`。

`options` 每项包含 `label` 和 `disabled`。选中索引通过 `usize` 回调（TSX 里是 `number`）。
