# 进度条

```tsx
<Progress percent={62} width={200} height={8} />
```

| 属性 | 默认值 / 说明 |
|---|---|
| `percent` | 0，限制在 0–100 |
| `width`、`height` | 200、8 |
| `color` | 进度填充色，默认主题 `colorPrimary` |
| `key` | 可选节点标识 |

只显示轨道和进度，没有内置百分比文字或不确定状态。
Rust：`ui::progress(ui::ProgressOptions { .. })`。
