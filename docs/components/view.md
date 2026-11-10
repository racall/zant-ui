# 通用容器 `ui.view`

类似 React Native 的 `View` 或 HTML 的 `div`，是最基础的通用容器。实现在 `src/components/view.zig`。

```zig
ui.view(.{
    .padding = ui.inset(16),
    .gap = 8,
    .radius = 8,
    .background = theme.color_bg_container,
    .on_click = ui.action(self, State.open),
    .children = &.{
        ui.text("标题", .{ .weight = .semibold }),
        ui.view(.{ .direction = .row, .gap = 8, .children = &.{ a, b } }),
    },
})
```

## 与 `ui.column` / `ui.row` 的区别

| | `ui.view` | `ui.column` / `ui.row` |
|---|---|---|
| 主轴 | `direction`，默认 `.column`（RN 默认） | 固定 column / row |
| `align_items` | `.stretch`：子元素横向撑满，同块级 div / RN View | `.start` |
| `shrink` | 0：不被父容器压扁（RN 默认） | 0 |
| 阴影 | `shadow_color` / `shadow_blur` / `shadow_offset_y` | 需 `ui.styled` |
| 点击 | `on_click` 或 `index_action` + `index`（整块可点，内部按钮照常接收点击） | 需 `ui.clickable` |
| 内联样式 | `style: ui.Style`，最后应用（优先级最高） | 需 `ui.styled` |

## 选项

`ui.BoxOptions` 的全部字段，名字相同（见 [layout.md](layout.md)）：`key`、`gap`、`padding`、`margin`、
`justify` / `justify_content`、`align_items`、`wrap`、宽高与百分比、`min_*` / `max_*`、`grow` / `shrink` / `basis`、
绝对定位、`background`、`border_*`、`radius`、`clip`、`scroll`、`hover_background`、`show_on_hover`、
`focus_scope`、`on_resize`、`children` 等。另外有：

- `direction`：主轴；CSS 写法的 `flex_direction` 设置时优先；
- `shadow_color` / `shadow_blur` / `shadow_offset_y`：投影，克制使用（浮起的卡片、弹层）；
- `on_click`、`index_action` + `index`、`cursor`（默认手形）：整块可点击；
- `style`：`ui.Style` 覆盖，与 `ui.styled(ui.view(..), style)` 等价。

## 与 RN / CSS 对照

| RN `style` / CSS | `ui.view` |
|---|---|
| `flexDirection` / `flex-direction` | `direction`（或 `flex_direction`） |
| `justifyContent` / `alignItems` | `justify_content` / `align_items` |
| `flexGrow` / `flexShrink` / `flexBasis` | `grow` / `shrink` / `basis` |
| `padding` / `margin` | `padding = ui.inset(..)` / `ui.insetXY(..)` |
| `backgroundColor` | `background`（ARGB `u32`，优先用主题 token） |
| `borderRadius` / `borderWidth` / `borderColor` | `radius` / `border_width` / `border_color` |
| `position: absolute` + `top/left/...` | `absolute = true` + `top` / `left` / ... |
| `overflow: hidden` / `scroll` | `clip = true` / `scroll = true` |
| `onPress` / `onclick` | `on_click` |
