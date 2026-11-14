# 进度条

```tsx
<Progress percent={62} width={200} height={8} />
```

## 选项

`key`、`percent`（0–100）、`width`、`height`、`color`。
手写 Rust：`ui::progress(ui::ProgressOptions { .. })`。

只显示进度轨道；没有内置百分比文字和不确定状态。
