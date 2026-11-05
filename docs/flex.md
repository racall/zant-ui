# Flex 布局与 CSS 对照

## 已支持

| CSS | Zant |
|---|---|
| `display: flex` | `ui.column` / `ui.row` |
| `flex-direction` | `direction` 或 `flex_direction` |
| `justify-content` | `justify` 或 `justify_content` |
| `align-items` | `align_items` |
| `gap` | `gap` |
| `flex-grow` | `grow` |
| `flex-shrink` | `shrink` |
| `flex-basis` | `basis` |
| `width/height` | `width` / `height`（像素） |
| `min/max-width/height` | `min_width` 等 |
| `padding/margin` | `padding` / `margin` |

## 尚未完整支持

当前布局是**高性能单行 Flex**，不宣称完整 CSS：

- `flex-wrap`
- 百分比尺寸（`width: 50%`）
- `align-content`
- `order` / `align-self`
- `flex` shorthand
- `auto` 尺寸和 `calc()`

完整 CSS Flex 需要多行布局算法和两轮 intrinsic sizing，会改变现有布局引擎；
需要单独重构并补基准测试。当前 `nowrap` 路径保持轻量，是性能优先的实现。
