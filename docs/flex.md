# Flex 布局与 CSS 对照

```tsx
import { View, Row, Text } from "zant";

export default function App() {
  return <View padding={24} gap={12}>
    <Row gap={12} alignItems="center">
      <View grow={1}><Text>随剩余空间伸展</Text></View>
      <View width={120} shrink={0}><Text>固定宽度</Text></View>
    </Row>
    <Row wrap="wrap" columnGap={12} rowGap={8}>
      <View width={200}><Text>第一项</Text></View>
      <View width={200}><Text>第二项</Text></View>
    </Row>
  </View>;
}
```

| CSS | TSX 属性 / 组件 |
|---|---|
| `display:flex` / `flex-direction` | `View`、`Column`、`Row` / `flexDirection` |
| `justify-content` | `justify` / `justifyContent`：start、center、end、space-between、space-around、space-evenly |
| `align-items` / `align-self` | `alignItems` / `alignSelf`：start、center、end、stretch；后者另有 auto |
| `flex-wrap` / `align-content` | 横向 `wrap="wrap"` / `alignContent` |
| `gap` / `row-gap` / `column-gap` | `gap` / `rowGap` / `columnGap` |
| `order` / `flex-grow` / `flex-shrink` / `flex-basis` | `order` / `grow` / `shrink` / `basis` |
| 宽高与百分比 | `width` / `height`；`widthPercent={50}` 或 `width="50%"` |
| min/max、内外边距 | `minWidth` / `maxWidth` / `minHeight` / `maxHeight`、`padding` / `margin` |

百分比取 0–100。横向多项的百分比宽度还需给 gap 留空间；不要用三个 33% 再加间距。
固定高度控件默认不收缩；长文本用 `Paragraph`。Rust 对应用 `ui::BoxOptions` 与 `ui::Style`
的 snake_case 字段，Flex 枚举位于 `zant::layout::flex`。

## 绝对定位

```tsx
import { View, Text } from "zant";

export default function App() {
  return <View height={120} padding={16}>
    <View absolute left={16} right={16} top={16} height={32}>
      <Text>左右同时指定、无 width 时横向拉伸</Text>
    </View>
  </View>;
}
```

- 定位和百分比相对父元素的含内边距盒子；绝对定位节点不参与 Flex 分配。
- Rust `Style.anchor = Some(n)` 跟随第 n 个流式兄弟的 x / 宽度；未指定纵向位置时也跟随 y / 高度。
  配合 `layout_transition`，见 [动效](motion.md)。
- 当前不支持纵向 wrap、wrap-reverse、flex 简写、auto/calc、min-content/max-content、百分比 min/max。
- `alignContent` 支持 start/center/end/space-between/space-around；stretch 当前等同 start。
