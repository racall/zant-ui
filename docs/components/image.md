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

`key`、`src`、`width`、`height`、`fit`（`.cover` / `.contain` / `.fill`）、
`radius`、`tooltip`。

支持 PNG/JPEG/WebP；路径相对于进程工作目录。加载失败时显示占位背景。
