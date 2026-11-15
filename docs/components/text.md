# 文本

```tsx
import { Column, Heading, Text, Paragraph } from "zant";

export default function App() {
  return <Column gap={8} width={320}>
    <Heading level={2}>设置</Heading>
    <Text>当前方案：<Text weight="bold">专业版</Text></Text>
    <Paragraph>长文本会在容器宽度内自动换行。</Paragraph>
    <Text maxLines={1}>空间不足时以省略号截断。</Text>
  </Column>;
}
```

| 组件 | 属性与默认值（均支持可选 `key`） |
|---|---|
| `Text` | `size=14`、`weight="regular"`、`color=主题 colorText`、`textAlign="left"`、`wrap=false`、`lineHeight=0`（字体默认）、`maxLines=0`（不限）、`family="sans"` |
| `Heading` / `Title` | `level=1`（1–5 对应 30/24/20/16/14px）、`weight="semibold"`、`color=主题 colorText`、`textAlign="left"` |
| `Paragraph` | `size=14`、`color=主题 colorTextSecondary`、`textAlign="left"`、`lineHeight=1.5`，自动换行 |

`weight` 可用 `thin` / `extra_light` / `light` / `regular` / `medium` / `semibold` / `bold` / `extra_bold` / `black`；`Text` 还接受数字 100–900（整百）。
`family` 可选 `sans` / `serif` / `mono` / `serif_cjk` / `math`；`textAlign` 可选 `left` / `center` / `right`。
嵌套 `Text` 编译成富文本，支持局部 `italic` / `underline` / `strike` / `background`；文字选择由外层容器的 `selectable` 开启。
Rust：`ui::text` / `ui::heading` / `ui::paragraph`，分别使用 `TextOptions` / `HeadingOptions` / `ParagraphOptions`。
