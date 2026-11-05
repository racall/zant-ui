# 卡片

```zig
ui.card(.{
    .title = "项目",
    .extra = "详情",
    .padding = ui.inset(20),
    .gap = 12,
    .children = &.{
        ui.paragraph("说明文字", .{}),
    },
})
```

## 选项

`key`、`title`、`extra`、`children`、`width`、`height`、`gap`、`padding`、
`bordered`。

卡片是带主题背景、边框和圆角的容器，自定义组件可以直接组合它。
