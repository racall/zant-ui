# 按钮

```tsx
import { Button, useState } from "zant";

export default function App() {
  const [saved, setSaved] = useState(false);
  return <Button variant="primary" onClick={() => setSaved(true)}>
    {saved ? "已保存" : "保存"}
  </Button>;
}
```

| 属性 | 默认 / 说明 |
|---|---|
| `text` / 文字子节点 | 空字符串 |
| `variant` | `"default"`；常用 `"primary"` / `"text"` / `"link"`；另有 `"plain"` / `"caption"` / `"caption_close"`（标题栏用） |
| `size` | `"middle"`；`"small"` / `"middle"` / `"large"` 高 24 / 32 / 40 |
| `disabled`、`focusable` | false、true；关闭 focusable 后点击不抢焦点 |
| `width`、`fontSize`、`textColor` | 可选覆盖；`textAlign` 默认 `"center"`，`labelPadding` 默认 0 |
| `icon`、`iconSize` | `"none"`、18；见 [图标](icon.md) |
| `onClick` | 点击回调；或 `indexAction` + `index`（默认 0，优先于 onClick） |
| `key` | 节点标识 |

Tab 聚焦，Space / Enter 激活；移出按钮后释放不触发点击，禁用时忽略输入。
Rust：`ui::button(ui::ButtonOptions { .. })`，回调用 `ui::action` / `ui::index_action`。
