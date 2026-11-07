# 图片

```zig
ui.image(.{
    .src = "assets/cover.png",
    .width = 320,
    .height = 180,
    .fit = .cover,
    .tooltip = "封面",
})
```

## 选项

`key`、`src`、`image`（内存图片 `zant.image.Image`，给出时代替 `src`）、`width`、`height`、
`fit`（`.cover` / `.contain` / `.fill`）、`radius`（默认 8）、`tooltip`。

`src` 支持 PNG / JPEG / GIF / BMP / ICO；路径相对于进程工作目录。加载失败时显示占位背景。
