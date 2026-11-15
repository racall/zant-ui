# 卡片

```tsx
<Card title="项目" extra="3 项">
  <Paragraph>说明文字</Paragraph>
</Card>
```

- `title`、`extra`：可选字符串；任一存在时显示标题行，`extra` 是右侧文字。
- `children`：内容；`width`、`height` 默认随布局。
- `gap` 默认 12，`padding` 默认四边 20（也可传边距对象），`bordered` 默认 true。
- `key`：节点标识。背景与边框跟随主题。

Rust：`ui::card(ui::CardOptions { .. })`。
