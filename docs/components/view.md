# 通用容器 View

```tsx
import { useState, useTheme, View, Row, Text, Button } from "zant";

export default function App() {
  const theme = useTheme();
  const [open, setOpen] = useState(false);
  return <View padding={16} gap={8} background={theme.colorBgContainer}>
    <Row gap={8} alignItems="center">
      <Text weight="semibold">详情</Text>
      <Button text="展开 / 收起" onClick={() => setOpen(!open)} />
    </Row>
    {open && <View key="details" motion="slideUp" motionOrigin="top" layoutTransition>
      <Text>详细内容</Text>
    </View>}
  </View>;
}
```

| 属性组 | 默认值 / 说明 |
|---|---|
| 主轴与对齐 | `direction="column"`、`justify="start"`、`alignItems="stretch"`；`flexDirection` / `justifyContent` 分别优先于前两项 |
| 间距 | `gap=0`，可分设 `rowGap` / `columnGap`；`padding` / `margin` 默认为 0，支持数字或 `{ top, right, bottom, left, horizontal, vertical }` |
| 尺寸 | `width` / `height` 支持数字或 `"100%"`；也可用 `widthPercent` / `heightPercent`；支持 `minWidth` / `minHeight` / `maxWidth` / `maxHeight` |
| Flex | `grow=0`、`shrink=0`、`basis` 未设、`order=0`、`alignSelf="auto"`、`wrap="nowrap"`、`alignContent="start"` |
| 定位 | `absolute=false`；`top` / `right` / `bottom` / `left` 未设；绝对定位相对父容器含内边距的区域 |
| 外观 | `background` / `borderColor` 未设、`borderWidth=0`、`radius=0`；阴影用 `shadowColor` / `shadowBlur=0` / `shadowOffsetY=0` |
| 溢出与输入 | `clip` / `scroll` / `blockPointer` / `selectable` / `unselectable` 均为 `false`；滚动需有确定高度 |
| 悬停与焦点 | `hoverBackground` / `hoverBorderColor` 未设；`showOnHover=false`、`focusScope=false`；`onResize` 在首次及尺寸变化后调用 |
| 点击 | `onClick`，或 `indexAction` + `index=0`（优先）；可点击时 `cursor="pointingHand"`，内部控件仍接收自己的点击 |
| 动效 | `motion="none"`、`motionOrigin="center"`、`layoutTransition=false`；进出动画使用稳定 `key` |
| 其他 | `key`、`children`、`style`（按源码顺序合并，见 [样式](style.md)） |

`motion` 可选 `fade` / `zoomBig` / `zoomBigFast` / `slideUp` / `slideDown` / `moveUp`；`motionOrigin` 为 `center` 或八个边 / 角（如 `top`、`topLeft`）。
`Row` / `Column` 默认横排 / 竖排、`alignItems="start"`，也支持以上容器动效；整块点击用 `Clickable`。当前只支持横向 wrap，不支持纵向 wrap / wrap-reverse。
Rust：`ui::view(ui::ViewOptions { .. })`；`Row` / `Column` 使用 `ui::BoxOptions`。
