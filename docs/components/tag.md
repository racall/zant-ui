# 标签

```tsx
import { useTheme, Row, Tag } from "zant";

export default function App() {
  const theme = useTheme();
  return <Row gap={8}>
    <Tag>测试版</Tag>
    <Tag text="已完成" color={theme.colorSuccessText} background={theme.colorSuccessBg} />
  </Row>;
}
```

- `text` / 文本子节点：默认空文字。
- `color`：默认主题 `colorText`；`background`：默认 `colorFillSecondary`。
- `key`：可选节点标识。颜色支持 ARGB 数字或颜色字符串。

Rust：`ui::tag(ui::TagOptions { .. })`。
