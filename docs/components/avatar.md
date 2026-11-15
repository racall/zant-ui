# 头像

```tsx
<Avatar text="XL" size={40} />
```

| 属性 | 默认 / 说明 |
|---|---|
| `text`、`size` | `"?"`、32；圆形文字头像，不加载图片 |
| `background`、`color` | 主题 `colorPrimaryBg` / `colorPrimary`；只设背景时文字默认白色 |
| `key` | 节点标识 |

Rust：`ui::avatar(ui::AvatarOptions { .. })`。
