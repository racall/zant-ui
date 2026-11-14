# 按钮

```tsx
<Button
  text="Save Changes"
  variant="primary"
  size="middle"
  onClick={() => save()}
/>
```

## 选项

`key`、`text`、`variant`（`"primary"` / `"default"` / `"text"` / `"link"`）、
`size`（`"small"` / `"middle"` / `"large"`，高 24/32/40）、`disabled`、`width`、
`fontSize`、`textAlign`、`labelPadding`、`textColor`、`icon`（`Icon` 名字，见
[icon.md](icon.md)）、`iconSize`、`focusable`（`false` 时点击不抢焦点）、
`onClick`、`indexAction` + `index`、`children`（文字）。
手写 Rust：`ui::button(ui::ButtonOptions { .. })`。

Hover/Pressed 使用 200/100ms 过渡（antd motionDurationMid / Fast）；只有指针在按钮内释放才触发点击。
Tab 聚焦，Space/Enter 激活；禁用状态忽略输入。
