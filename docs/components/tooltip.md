# 提示 Tooltip

```tsx
<Tooltip text="复制链接">
  <Button text="复制" />
</Tooltip>
```

- `text`：必填提示文字。
- `child` / 唯一子元素：必填其一，提示附着到该节点。
- `key`：可选节点标识。

悬停 450ms 后显示在目标下方，放不下时翻到上方；带缩放动效，绘制在窗口最上层。
`Image`、`Select`、`IconButton` 及 `Segmented` 的每个选项也支持 `tooltip`。
快捷键文字用 `keyLabel("mod+c")` 生成。
Rust：`ui::tooltip(ui::TooltipOptions { text, child })`。
