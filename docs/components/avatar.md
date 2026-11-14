# 头像

```tsx
<Avatar text="XL" size={40} />
<Avatar text="A" size={36} background={0xff1677ff} color={0xffffffff} />
```

## 选项

`key`、`text`、`size`、`background`、`color`。
手写 Rust：`ui::avatar(ui::AvatarOptions { .. })`。

显示文字头像；不加载图片。默认浅主色底 + 主色文字；只设 `background` 时文字默认白色。
