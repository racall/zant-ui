# 样式覆盖

```tsx
import { useTheme, View, Button } from "zant";

export default function App() {
  const theme = useTheme();
  return <View style={{ padding: 16, gap: 8, background: theme.colorBgContainer }}>
    <Button text="保存" variant="primary" style={{ width: 160, fontSize: 16 }} />
  </View>;
}
```

## TSX `style`

内置标签的 `style` 必须是对象字面量，只能包含该标签已有的属性（见各组件文档或 `zant.d.ts`）。按源码顺序合并，同名属性后写的覆盖先写的（下例分别为 16、8）：

```tsx
<View padding={8} style={{ padding: 16 }} />
<View style={{ padding: 16 }} padding={8} />
```

- 容器可设置布局、背景、边框及 `hoverBackground` / `hoverBorderColor`。
- `View` / `Column` / `Row` / `Clickable` 支持 `motion`、`motionOrigin`、`layoutTransition`，见 [View](view.md)。
- `Button` 没有 `background` / `borderColor` / `radius` / `height` 属性，放进 TSX `style` 也不支持。用 `variant` / `size` 配置外观。

## Rust `ui::styled`

Rust 可覆盖节点的通用外观、布局和文本；这套字段与 TSX `style` 不同：

```rust
ui::styled(ui::button(ui::ButtonOptions::default()), ui::Style {
    height: Some(40.0),
    radius: Some(8.0),
    ..Default::default()
})
```

- 外观：`background`、`border_color`、`border_width`、`radius` / `border_radius`、`shadow_color`、`shadow_blur`、`shadow_offset_y`、`clip`、`block_pointer`。
- 布局：`padding`、`margin`、宽高及百分比、`min_*` / `max_*`、`grow` / `shrink` / `basis`、`order`、`align_self`、`absolute`、四边偏移、`anchor`。
- 文本：`text_color` / `color`、`font_size`、`font_weight`、`text_align`、`line_height`、`wrap`。
- 动效：`motion`、`motion_origin`、`layout_transition`（`motion::Transition`）。

返回新节点，仅覆盖所给字段；作用于该节点，不递归改写复合组件的内部节点。颜色优先用主题 token。
