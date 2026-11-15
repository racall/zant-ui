# 复选框

```tsx
import { Checkbox, useState } from "zant";

export default function App() {
  const [checked, setChecked] = useState(false);
  return <Checkbox label="接收通知" checked={checked} onChange={setChecked} />;
}
```

- `label` / 文字子节点：默认空；`checked`、`disabled` 默认 false，`fontSize` 默认 14。
- `onChange(checked: boolean)`：把新值写回状态；`key` 是节点标识。

点击标签或方框切换；Tab 聚焦，Space 激活。
Rust：`ui::checkbox(ui::ToggleOptions { .. })`，回调用 `ui::bool_action`。
