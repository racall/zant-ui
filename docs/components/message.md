# 全局提示 Message

```tsx
import { useState, wakeAfter, View, Button, Message } from "zant";

export default function App() {
  const [until, setUntil] = useState(0);
  const now = performance.now();
  if (now < until) wakeAfter(until - now);
  return <View>
    <Button text="保存" onClick={() => setUntil(performance.now() + 1800)} />
    {now < until && <Message key="saved" text="已保存" kind="success" />}
  </View>;
}
```

| 属性 | 默认值 / 说明 |
|---|---|
| `text` | 必填，提示文字 |
| `kind` | `"success"`；可选 `"info"` / `"warning"` / `"err"` |
| `top` | 16，距父容器顶部的像素数 |
| `iconColor` | 默认按类型取主题色 |
| `key` | `"message"`；多个提示使用不同 key |

放在窗口根容器中，顶部居中，带进出动效；点击会穿透。没有内置显示时长，应用控制是否渲染，`wakeAfter` 安排到期重建。
Rust：`ui::message(ui::MessageOptions { .. })`，错误类型为 `ui::MessageKind::Err`。
