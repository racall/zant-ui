# 布局

```tsx
<Column gap={12} padding={24} alignItems="center">
  <Text>A</Text>
  <Row gap={8}>
    <Text>B</Text>
    <Text>C</Text>
  </Row>
</Column>
```

## 选项

`key`、`gap`、`rowGap`、`columnGap`、`padding`、`margin`、`justify`、`justifyContent`、
`alignItems`、`wrap`、`alignContent`、`alignSelf`、`order`、`width`、`height`、
`widthPercent`、`heightPercent`、`minWidth`、`minHeight`、`maxWidth`、
`maxHeight`、`grow`、`shrink`、`basis`、`absolute`、`top`、`right`、`bottom`、
`left`、`background`、`borderColor`、`borderWidth`、`radius`、`clip`、`scroll`、
`blockPointer`、`children`、`flexDirection`。手写 Rust：`ui::column` / `ui::row`
（`ui::BoxOptions { .. }`）。

还有这些容器选项：

- `selectable`：里面的文字可以拖选、双击选词、三击选段并 ⌘C / Ctrl+C 复制；`unselectable` 把子树排除在选择之外；
- `hoverBackground` / `hoverBorderColor`：指针在容器上时的背景 / 边框（200ms 过渡，不重建），列表行、卡片用；
  `showOnHover`：只在最近的带悬停样式的祖先被悬停时显示（行内的删除等次要按钮，见 [Clickable](clickable.md)）；
- `focusScope`：Tab / Shift+Tab 的焦点只在这个容器里循环（对话框、弹出面板；`Modal` 自带）；
- `onResize`：容器尺寸变化（包括第一次布局）后调用，用 `keyBounds`（手写 Rust）/ `onResize` 里读新尺寸，
  做随宽度变化的布局。

`Column` 主轴为纵向，`Row` 主轴为横向。`scroll` 为纵向滚动容器（需要确定高度），
`blockPointer` 吞掉空白处点击（弹层面板）。`Space` 是 antd `Space`（默认横向、
`gap` 8、居中对齐；只给 `width`/`height` 时可当固定占位），`Divider`（`ui::divider`）
是 1px 分隔线。`anchor` 等更多 CSS 对应关系见 [Flex 与 CSS](../flex.md)。
