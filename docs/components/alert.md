# 警告条

```tsx
<Alert title="已保存" description="所有更改都是最新的。" type="success" />
```

- `title`、`description`：默认空字符串；空描述不占位置。
- `type`：默认 `"info"`，还支持 `"success"` / `"warning"` / `"error_state"`（也可写 `"error-state"` / `"errorState"`）。
- `key`：节点标识。当前没有关闭按钮。

Rust：`ui::alert(ui::AlertOptions { .. })`。
