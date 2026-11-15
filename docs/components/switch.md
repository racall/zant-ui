# 开关

```tsx
import { useState, Switch } from "zant";

export default function App() {
  const [enabled, setEnabled] = useState(false);
  return <Switch label="启用通知" checked={enabled} onChange={setEnabled} />;
}
```

| 属性 | 默认值 / 说明 |
|---|---|
| `checked`、`onChange` | `false`；回调收到新的布尔值 |
| `label` / 文本子节点 | 默认空标签 |
| `disabled`、`fontSize` | `false`、14（标签字号） |
| `key` | 可选节点标识 |

Rust：`ui::r#switch(ui::ToggleOptions { .. })`，与 Checkbox 共用选项，回调用 `ui::bool_action`。
