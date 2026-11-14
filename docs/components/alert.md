# 警告条

```tsx
<Alert title="已保存" description="所有更改都是最新的。" type="success" />
```

## 选项

`title`、`description`、`type`（`"info"` / `"success"` / `"warning"` / `"error_state"`）、`key`。
手写 Rust：`ui::alert(ui::AlertOptions { .. })`。

用于页面内提示；当前没有关闭按钮。
