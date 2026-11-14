# 卡片

```tsx
<Card title="项目" extra="详情" padding={20} gap={12}>
  <Paragraph>说明文字</Paragraph>
</Card>
```

## 选项

`key`、`title`、`extra`、`children`、`width`、`height`、`gap`、`padding`、
`bordered`。手写 Rust：`ui::card(ui::CardOptions { .. })`。

卡片是带主题背景、边框和圆角的容器，自定义组件可以直接组合它。
