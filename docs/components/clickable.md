# 可点击区域 `Clickable`

```tsx
import { Clickable, Row, Text, useState, useTheme } from "zant";

export default function App() {
  const [count, setCount] = useState(0);
  const theme = useTheme();
  return <Clickable flexDirection="row" padding={12}
    hoverBackground={theme.colorFillTertiary} onClick={() => setCount(count + 1)}>
    <Text>点击次数：{count}</Text>
    <Row showOnHover><Text>打开</Text></Row>
  </Clickable>;
}
```

- `onClick` 或 `indexAction` + `index`（默认 0；indexAction 优先）。
- `cursor` 默认 `"pointing_hand"`；其余容器属性见 [布局](layout.md)。默认纵向排列，用 `flexDirection="row"` 改为横向。
- 区域内的按钮接收自己的点击；移出区域后释放不触发回调。

`showOnHover` 只在最近的带 `hoverBackground` / `hoverBorderColor` 的祖先悬停时绘制，并随悬停淡入淡出。
可在操作容器上使用（如 `<Row showOnHover>…</Row>`）；设 false 则常显。

Rust：`ui::clickable(ui::ClickableOptions { r#box, on_click, .. })`。
