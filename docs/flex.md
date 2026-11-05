# Flex 布局与 CSS 对照

## 已支持

| CSS | Zant |
|---|---|
| `display:flex` | `ui.column` / `ui.row` |
| `flex-direction` | `direction` / `flex_direction` |
| `justify-content` | `justify` / `justify_content` |
| `align-items` | `align_items` |
| `align-self` | `align_self` |
| `align-content` | `align_content`（行换行时） |
| `flex-wrap: wrap` | `wrap = .wrap`（横向） |
| `gap` | `gap` |
| `row-gap` / `column-gap` | `row_gap` / `column_gap` |
| `order` | `order` |
| `flex-grow` | `grow` |
| `flex-shrink` | `shrink` |
| `flex-basis` | `basis` |
| `width: %` | `width_percent` |
| `height: %` | `height_percent` |
| `min/max-width/height` | 像素值 |
| `padding/margin` | `padding` / `margin` |

`align-content` 支持 start/center/end/space-between/space-around；`stretch`
当前等同 start（不拉伸行高）。

## 示例

```zig
ui.row(.{
    .wrap = .wrap,
    .column_gap = 12,
    .row_gap = 8,
    .align_content = .space_between,
    .children = &.{
        ui.card(.{ .width_percent = 33, .children = &.{ ... } }),
        ui.card(.{ .width_percent = 33, .children = &.{ ... } }),
        ui.card(.{ .width_percent = 33, .children = &.{ ... } }),
    },
})
```

## 尚未支持

- **纵向 wrap**（`direction = .column` 时的 flex-wrap）
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
