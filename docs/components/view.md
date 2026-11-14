# 通用容器 `View`

类似 React Native 的 `View` 或 HTML 的 `div`，是最基础的通用容器。
实现在 `crates/zant/src/components/view.rs`。

```tsx
<View
  padding={16}
  gap={8}
  radius={8}
  background={theme.colorBgContainer}
  onClick={() => open()}
>
  <Text weight="semibold">标题</Text>
  <View direction="row" gap={8}>{a}{b}</View>
</View>
```

手写 Rust：`ui::view(ui::ViewOptions { .. })`（`ViewOptions` 包含 `BoxOptions` 的全部字段）。

## 与 `Row` / `Column` 的区别

| | `View` | `Row` / `Column` |
|---|---|---|
| 主轴 | `direction`，默认 `"column"`（RN 默认） | 固定 row / column |
| `alignItems` | `"stretch"`：子元素横向撑满，同块级 div / RN View | `"start"` |
| `shrink` | 0：不被父容器压扁（RN 默认） | 0 |
| 阴影 | `shadowColor` / `shadowBlur` / `shadowOffsetY` | 需 `style` / `ui::styled` |
| 点击 | `onClick` 或 `indexAction` + `index`（整块可点，内部按钮照常接收点击） | 需 `Clickable` |
| 内联样式 | `style` 对象（优先级最高） | `style` 对象 |

## 选项

`BoxOptions` 的全部字段，名字相同（见 [layout.md](layout.md)）：`key`、`gap`、`padding`、`margin`、
`justify` / `justifyContent`、`alignItems`、`wrap`、宽高与百分比、`min*` / `max*`、`grow` / `shrink` / `basis`、
绝对定位、`background`、`border*`、`radius`、`clip`、`scroll`、`hoverBackground`、
`showOnHover`、`focusScope`、`onResize`、`children` 等。另外有：

- `direction`：主轴；CSS 写法 `flexDirection` 设置时优先；
- `shadowColor` / `shadowBlur` / `shadowOffsetY`：投影，克制使用（浮起的卡片、弹层）；
- `onClick`、`indexAction` + `index`、`cursor`（默认手形）：整块可点击；
- `style`：样式覆盖，与 `ui::styled(ui::view(..), style)` 等价。

## 与 RN / CSS 对照

| RN `style` / CSS | `View` |
|---|---|
| `flexDirection` / `flex-direction` | `direction`（或 `flexDirection`） |
| `justifyContent` / `alignItems` | `justifyContent` / `alignItems` |
| `flexGrow` / `flexShrink` / `flexBasis` | `grow` / `shrink` / `basis` |
| `padding` / `margin` | 数字或 `{ top, right, bottom, left, horizontal, vertical }` |
| `backgroundColor` | `background`（ARGB `u32`，优先用主题 token） |
| `borderRadius` / `borderWidth` / `borderColor` | `radius` / `borderWidth` / `borderColor` |
| `position: absolute` + `top/left/...` | `absolute` + `top` / `left` / ... |
| `overflow: hidden` / `scroll` | `clip` / `scroll` |
| `onPress` / `onclick` | `onClick` |
