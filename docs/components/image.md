# 图片

```tsx
<Image src="assets/cover.png" width={320} height={180} fit="cover" tooltip="封面" />
```

## 选项

`key`、`src`、`image`（内存图片 `zant::image::Image`，给出时代替 `src`）、`width`、`height`、
`fit`（`"cover"` / `"contain"` / `"fill"`）、`radius`（默认 8）、`tooltip`。
手写 Rust：`ui::image(ui::ImageOptions { .. })`。

`src` 支持 PNG / JPEG / GIF / BMP / ICO；路径相对于进程工作目录。加载失败时显示占位背景。
TSX 里也可以用资源导入的字节：`<Image image={assetImage(iconBytes)} />`（`assetImage` 从 `"zant"` 导入）。
