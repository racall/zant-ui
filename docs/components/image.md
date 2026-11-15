# 图片

```tsx
<Image src="assets/cover.png" width={320} height={180} fit="cover" />
```

| 属性 | 默认 / 说明 |
|---|---|
| `src` | 文件路径，默认空；相对进程工作目录，支持 PNG / JPEG / GIF / BMP / ICO |
| `image` | 内存图片；有值时优先于 src |
| `width`、`height` | 默认使用图片自然尺寸 |
| `fit` | `"cover"` 裁切填满；`"contain"` 完整显示；`"fill"` 拉伸 |
| `radius`、`tooltip`、`key` | 圆角默认 8；可选提示文字和节点标识 |

推荐导入打包资源（示例路径需存在）：

```tsx
import { Image, assetImage } from "zant";
import cover from "../assets/cover.png";

export default function App() {
  return <Image image={assetImage(cover)} width={320} height={180} />;
}
```

加载失败显示主题占位背景。Rust：`ui::image(ui::ImageOptions { .. })`，`image` 为 `Option<Rc<zant::image::Image>>`。
