# 提示

```tsx
<Tooltip text="复制链接">
  <Button text="复制" onClick={() => copy()} />
</Tooltip>
```

## 选项

`text`、`child`（也可以写成唯一的子元素）。手写 Rust：`ui::tooltip(ui::TooltipOptions { .. })`。

悬停约 450ms 后显示在目标下方（zoom 动效），放不下时翻到上方；提示层绘制在窗口最上层。
`Image`、`Select`、`IconButton` 和 `Segmented` 的选项也有 `tooltip` 字段。
提示里的快捷键用 `keyLabel("mod+c")` 生成，不要写死 ⌘。
