# 布局

```tsx
<Column gap={12} padding={24}>
  <Text>A</Text>
  <Row gap={8} alignItems="center">
    <Text>B</Text>
    <Text>C</Text>
  </Row>
</Column>
```

`Column` 纵向、`Row` 横向；通用容器用 [View](view.md)。以下为两者的布局选项：

| 属性 | 默认 / 说明 |
|---|---|
| `key`、`children`、`flexDirection` | 标识、内容、覆盖排列方向（row / column） |
| `gap`、`rowGap`、`columnGap` | gap 默认 0；后两者覆盖对应轴 |
| `padding`、`margin` | 0；数字或 `{ horizontal, vertical }` / `{ top, right, bottom, left }` |
| `justify` / `justifyContent` | start；start / center / end / space_between / space_around / space_evenly；后者优先 |
| `alignItems`、`alignSelf` | start、auto；支持 start / center / end / stretch |
| `wrap`、`alignContent`、`order` | nowrap、start、0；目前只支持横向 wrap，不支持 wrap_reverse |
| `width` / `height`、`widthPercent` / `heightPercent` | 默认自动；百分比单位为 0–100 |
| `minWidth` / `minHeight`、`maxWidth` / `maxHeight` | 默认不限制 |
| `grow`、`shrink`、`basis` | 0、0、自动 |
| `absolute`、`top` / `right` / `bottom` / `left` | 默认 false、未设置；相对父容器含 padding 的盒子 |
| `background`、`borderColor`、`borderWidth`、`radius` | 默认无颜色、0、0 |
| `clip`、`scroll`、`blockPointer` | 默认 false；scroll 纵向滚动，需要确定高度；blockPointer 阻挡空白处点击 |
| `selectable`、`unselectable` | 默认 false；启用文字拖选复制 / 排除子树 |
| `hoverBackground`、`hoverBorderColor`、`showOnHover` | 悬停样式 / 悬停显示，见 [Clickable](clickable.md) |
| `focusScope`、`onResize` | 默认 false、无；限制 Tab 范围 / 尺寸变化后通知（无参数，含首次布局） |
| `motion`、`motionOrigin`、`layoutTransition` | none、center、false；进出场与布局过渡（需稳定 `key`），见 [动效](../motion.md) |

`onResize` 不传尺寸；Rust 可通过 `ui::key_bounds(key)` 查询布局结果。
`Space` 支持 `key`、`children`、`direction`（默认 row）、`gap`（8）、`alignItems`（center）、`justify`（start）、`width`、`height`。
`Divider` 是 1px 横线，支持 `key`、`margin`（默认 0）、`color`（默认主题 colorBorderSecondary）。

```tsx
<Space><Button>取消</Button><Button variant="primary">确定</Button></Space>
<Divider margin={{ vertical: 12 }} />
```

Rust：`ui::column` / `ui::row` 使用 `ui::BoxOptions`；`ui::space` / `ui::divider` 分别用 `SpaceOptions` / `DividerOptions`。
更多约束见 [Flex 与 CSS](../flex.md)。
