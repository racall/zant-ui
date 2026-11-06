# 样式覆盖

所有组件都可以先用自身 options 配置；需要统一覆盖时，再套一层
`ui.styled`：

```zig
ui.styled(
    ui.button(.{ .text = "保存", .variant = .primary }),
    .{
        .background = 0xff111111,
        .border_color = 0xff111111,
        .radius = 4,
        .height = 44,
    },
)
```

## 可覆盖字段

- 外观：`background`、`border_color`、`border_width`、`radius`（别名 `border_radius`）、
  `shadow_color`、`shadow_blur`、`shadow_offset_y`、`clip`、`block_pointer`。
- 布局：`padding`、`margin`、`width`、`height`、`width_percent`、`height_percent`、
  `min_width`、`min_height`、`max_width`、`max_height`、`grow`、`shrink`、`basis`、
  `order`、`align_self`、`absolute`、`top`、`right`、`bottom`、`left`、`anchor`。
- 文本：`text_color`（别名 `color`）、`font_size`、`font_weight`、`text_align`、
  `line_height`、`wrap`。
- 动效：`motion`、`motion_origin`、`layout_transition`（见 [动效](../motion.md)）。

规则：

- `ui.styled` 返回新的 Node，不修改原组件。
- 布局属性会覆盖组件的默认尺寸，因此可以统一按钮/卡片/输入框风格。
- 按钮背景和边框支持 `background` / `border_color` 覆盖。
- 文本颜色/字号/字重会写入组件的文本样式。
- 更细的交互状态（hover、variant）仍由组件 options 控制。

这样既保留每个组件的默认语义，又允许项目建立自己的设计系统。

## 组件覆盖矩阵

所有 `ui.*` 组件都可以套 `ui.styled`：

| 组件 | background | border | radius | 布局/文本 |
|---|---|---|---|---|
| column/row/card/space | ✅ | ✅ | ✅ | ✅ |
| text/heading/paragraph | ✅ 背景 | ✅ | ✅ | ✅ |
| button/link | ✅ | ✅ | ✅ | ✅ |
| input | ✅ | ✅ | ✅ | ✅ |
| checkbox/switch/radio | ✅ 外层 | ✅ 外层 | ✅ 外层 | ✅ |
| select | ✅ | ✅ | ✅ | ✅ |
| image | ✅ 占位/背景 | ✅ | ✅ | ✅ |
| tabs/tag/alert/avatar | ✅ | ✅ | ✅ | ✅ |
| progress | ✅ | ✅ | ✅ | ✅ |
| pdfPreview | ✅ 外框 | ✅ | ✅ | ✅ |

内部状态色（如 checkbox 勾选色、switch 轨道色）仍由主题和组件 options
控制；需要完全自定义时可以继续扩展该组件的 paint 逻辑。
