# 开关

```tsx
const [enabled, setEnabled] = useState(false);
<Switch checked={enabled} onChange={setEnabled} />
```

## 选项

`key`、`label`、`checked`、`onChange`（`(checked: boolean) => void`）、`disabled`、`fontSize`
（与 Checkbox 共用 `ToggleOptions`）。手写 Rust：`ui::r#switch(ui::ToggleOptions { .. })`
（`switch` 是 Rust 关键字，builder 名写作 `r#switch`）。
