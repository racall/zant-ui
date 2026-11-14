# 可点击区域 `Clickable` 与悬停显示 `showOnHover`

## `Clickable`

整块区域可点击的容器（列表行、卡片），里面的按钮照常接收自己的点击：

```tsx
<Clickable
  direction="row"
  padding={{ horizontal: 12, vertical: 10 }}
  radius={10}
  hoverBackground={theme.colorFillTertiary}
  indexAction={openRow}
  index={i}
>
  {icon}
  {content}
  <Row showOnHover>
    {deleteButton}
  </Row>
</Clickable>
```

选项：`onClick` 或 `indexAction` + `index`、`cursor`（默认手形），以及容器的全部布局 / 外观属性。
手写 Rust：`ui::clickable(ui::ClickableOptions { box, on_click, index_action, index, cursor })`。
按下后移出区域再松开不算点击。

## `showOnHover`

只在最近的带悬停样式（`hoverBackground` / `hoverBorderColor`）的祖先被悬停时绘制，随悬停过渡
淡入淡出：行内的删除、更多等次要操作不必在每一行都显示（CSS `.row:hover .actions`）。
选中的行可以直接设为 `false` 让操作常显。
