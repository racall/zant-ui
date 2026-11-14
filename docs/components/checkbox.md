# 复选框

```tsx
const [notifications, setNotifications] = useState(false);
<Checkbox label="Notifications" checked={notifications} onChange={setNotifications} />
```

## 选项

`key`、`label`、`checked`、`onChange`（`(checked: boolean) => void`）、`disabled`、`fontSize`。
手写 Rust：`ui::checkbox(ui::ToggleOptions { .. })`，回调用 `ui::bool_action`。

点击标签或方框切换；Tab 聚焦，Space 激活。回调参数为 `bool`。
