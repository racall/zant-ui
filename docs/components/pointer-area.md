# 指针区域与后台唤醒

TSX 用容器悬停样式和内置组件组合交互：

```tsx
import { useTheme, Row, Text } from "zant";

export default function App() {
  const theme = useTheme();
  return <Row height={32} radius={6} hoverBackground={theme.colorFillSecondary}>
    <Text>悬停显示背景</Text>
  </Row>;
}
```

`hoverBackground` / `hoverBorderColor` 带过渡，只重绘；指针在子控件上也算悬停。`focusScope` 将 Tab / Shift+Tab 限制在最上层范围内（Modal 自带）。`onResize` 在首次及尺寸变化后调用，并在同一帧重建；Rust 回调可用 `ui::key_bounds(key)` 读取新尺寸。

## 原始指针（仅 Rust）

TSX 没有 `PointerArea` 标签或 `onPointer` 属性。Rust 用 `ui::pointer_area`：

```rust
ui::pointer_area(ui::PointerAreaOptions {
    r#box: ui::BoxOptions {
        key: Some("canvas".into()),
        width: Some(320.0),
        height: Some(240.0),
        ..Default::default()
    },
    cursor: Some(ui::Cursor::Crosshair),
    on_pointer: ui::PointerAction::new(|cx, info| {
        let state = cx.state::<State>();
        if info.phase != ui::PointerPhase::Down { return false; }
        state.point = (info.x, info.y);
        true
    }),
    ..Default::default()
})
```

上例假设应用的 `State` 有 `point: (f32, f32)` 字段。窗口根节点不接收指针：`pointer_area` 放在普通根容器内。

| 选项 | 默认值 / 说明 |
|---|---|
| `r#box` | `BoxOptions::default()`，容器布局与子节点 |
| `on_pointer` | `PointerAction::new` 回调；返回 `true` 请求重建 |
| `capture_scroll` | `false`；开启后接收 `Scroll`，不滚动外层容器 |
| `clicks` | `true`；关闭后只处理悬停 / 滚轮，内部文字可选择 |
| `cursor` | `None`；可设 `Some(ui::Cursor::ResizeHorizontal)` 等 |
| `focusable`、`on_key` | `false`、`None`；开启焦点后用 `ui::KeyAction::new` 处理按键 |

`PointerInfo`：`phase` 为 `Down` / `Drag` / `Up` / `Hover` / `Leave` / `Scroll`；`x/y` 是区域内坐标，`window_x/window_y` 是窗口坐标，另有 `width/height`、`button`、`modifiers`、`click_count`、滚动量 `dx/dy`。按下后捕获指针，移出区域仍收到拖动，释放或失焦时收到 `Up`。

## 内存图片与后台任务（Rust）

- `zant::image::Image::from_encoded(bytes)` / `from_data_uri(uri)` 解码图片；`from_pixels(pixels, width, height, stride)` 接收预乘 32 位像素，字节序见 `zant::image::NATIVE_ORDER`。
- `ui::ImageOptions.image` 接收 `Option<Rc<Image>>`，例如 `image: Some(Rc::new(image))`；节点持有图片引用。
- 图片可在后台线程创建；发布结果后调用线程安全的 `ui::post_wake()`，通知 UI 线程重建，多次通知会合并。UI 状态仍只在 UI 线程修改。
- `ui::display_scale()` 返回当前显示器像素密度，供画布选择渲染分辨率。
