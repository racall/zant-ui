# Flex 布局与 CSS 对照

## 已支持

| CSS | Zant |
|---|---|
| `display:flex` | `Column` / `Row`（Rust `ui::column` / `ui::row`） |
| `flex-direction` | `Row` / `Column`，或 `flexDirection` |
| `justify-content` | `justify` / `justifyContent` |
| `align-items` | `alignItems` |
| `align-self` | `alignSelf` |
| `align-content` | `alignContent`（行换行时） |
| `flex-wrap: wrap` | `wrap="wrap"`（横向） |
| `gap` | `gap` |
| `row-gap` / `column-gap` | `rowGap` / `columnGap` |
| `order` | `order` |
| `flex-grow` | `grow` |
| `flex-shrink` | `shrink` |
| `flex-basis` | `basis` |
| `width: %` | `widthPercent`（或 `width="50%"`） |
| `height: %` | `heightPercent`（或 `height="50%"`） |
| `min/max-width/height` | 像素值 |
| `padding/margin` | `padding` / `margin` |

`align-content` 支持 start/center/end/space-between/space-around；`stretch`
当前等同 start（不拉伸行高）。

## 示例

```tsx
<Row wrap="wrap" columnGap={12} rowGap={8} alignContent="space-between">
  <Card widthPercent={33}>…</Card>
  <Card widthPercent={33}>…</Card>
  <Card widthPercent={33}>…</Card>
</Row>
```

手写 Rust 用 `ui::row(ui::BoxOptions { wrap: Some(flex::Wrap::Wrap), column_gap: 12.0,
row_gap: 8.0, align_content: Some(flex::AlignContent::SpaceBetween), children: nodes![..], .. })`
（或 `ui::styled` 设置 `width_percent`）。

## 绝对定位

- `absolute = true` 时相对父元素的**含内边距盒子**定位；`left` / `right` / `top` / `bottom`
  与百分比尺寸同 CSS。同时设 `left` 与 `right`（且无宽度）时拉伸到两侧之间。
- `anchor = n`：绝对定位子节点取第 n 个流式兄弟节点的 x / 宽度（未设置纵向位置时也取
  y / 高度），配合 `layout_transition` 实现分段控制器滑块、标签页下划线的平滑移动。

## 尚未支持

- **纵向 wrap**（`Column` 方向上的 flex-wrap）
- `wrap-reverse`
- `align-content: stretch`
- `flex` 简写
- `auto` / `calc()` / `min-content` / `max-content`
- 百分比 min/max

## 性能

- `nowrap` 保持原有单行快速路径。
- 只有使用 `wrap` 时才进入多行布局。
- 布局测试覆盖：wrap、百分比、order、align-self，以及原有 grow/shrink/
  min/max/gap/margin/justify。
