# 链接

```tsx
<Link text="文档" onClick={() => openDocs()} />
```

## 选项

`text`、`onClick`、`key`、`children`。手写 Rust：`ui::link(ui::LinkOptions { .. })`。

视觉上使用链接色；行为完全由 `onClick` 决定，不会自动打开浏览器。
