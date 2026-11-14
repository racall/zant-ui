# 指针区域、尺寸变化、内存图片、后台唤醒

自定义交互（画布、分隔条、拖动手柄、文档阅读器）用到的底层能力。PDF 阅读器就建立在它们之上。
这些是**手写 Rust**（或框架内部）的接口；`onPointer` 的回调类型不在 TSX 标签表里，
TSX 应用暂时用 `onResize` / 内置组件组合实现（迁移中，见 `docs/rust-port/status.md`）。

## `ui::pointer_area`

一个接收原始指针输入的容器：

```rust
fn drag_split(state: &mut State, info: ui::PointerInfo) -> bool {
    match info.phase {
        ui::PointerPhase::Down => state.drag_start = (info.window_x, state.split),
        ui::PointerPhase::Drag => {
            state.split = (state.drag_start.1 + info.window_x - state.drag_start.0).clamp(200.0, 600.0)
        }
        _ => return false,
    }
    true // 状态变了：重建界面
}

ui::pointer_area(ui::PointerAreaOptions {
    box: ui::BoxOptions {
        key: Some("splitter".into()),
        width: Some(6.0),
        height_percent: Some(100.0),
        ..Default::default()
    },
    cursor: ui::Cursor::ResizeHorizontal,
    on_pointer: ui::pointer_action(|s: &mut State, info| drag_split(s, info)),
    ..Default::default()
})
```

- `on_pointer` 收到 `ui::PointerInfo`：`phase`（`Down` / `Drag` / `Up` / `Hover` / `Leave` /
  `Scroll`）、相对区域左上角的 `x` / `y`、窗口坐标 `window_x` / `window_y`、区域尺寸、
  `button`、`modifiers`、`click_count`（双击 2、三击 3）、滚动量 `dx` / `dy`。
  返回 `true` 表示状态变了、需要重建。
- 按下后区域**捕获指针**：移出区域、移出窗口都继续收到 `Drag`，松开收到 `Up`；窗口失焦时
  也会补一个 `Up`。拖动距离超过 4px 的按下不计入多击。
- `capture_scroll = true`：滚轮 / 触控板滚动交给 `on_pointer`（`Scroll`），不再滚动外层
  滚动容器。⌘/Ctrl + 滚轮与触控板捏合都以 Control + 滚轮送达（捏合 `dy = 250 × 放大量`），
  可缩放视图同样处理。
- `cursor`：区域上的指针形状（`Arrow`、`PointingHand`、`IBeam`、`Crosshair`、
  `ResizeHorizontal`、`ResizeVertical`、`Grab`、`Grabbing`、`NotAllowed`）。
- `focusable = true`：点击获得键盘焦点（Tab 也能到达），按键先交给 `on_key`
  （`ui::key_action`，返回 `true` 表示已处理）。
- `clicks = false`：只接收悬停和滚轮，按下时像普通容器（里面的文字仍可选择）。
- 区域里的按钮、输入框等交互控件照常接收自己的点击。

## 悬停样式 `hoverBackground` / `hoverBorderColor`

```tsx
<Row height={32} radius={6} hoverBackground={theme.colorFillSecondary}>…</Row>
```

指针在容器上时显示的背景 / 边框，带 antd 的 200 ms 过渡，只重绘、不重建界面，应用不必保存悬停
状态。指针在容器里的按钮等控件上时，容器也算悬停（CSS `:hover`）；模态框等遮挡层盖住时不显示。

## 焦点范围 `focusScope`

Tab / Shift+Tab 只在最上层的焦点范围内循环。`Modal` 自带：打开对话框后 Tab 不会跳到背后的控件；
焦点在范围外时，Tab 进入范围的第一个控件。

## `onResize`

容器尺寸变化（包括首次布局）后调用，然后**同一帧内**重建并重新布局，所以新尺寸不会先闪一帧
旧布局。回调里可用 `ui::key_bounds(key)` 读新尺寸。适合“适合宽度”的阅读器、响应式列数等。

## 内存图片 `zant::image::Image`

```rust
let Some(img) = zant::image::Image::from_encoded(png_bytes) else { return }; // PNG / JPEG / GIF / BMP / ICO
// 显示：交给 Image 组件；Image 持有自己的引用，图片在显示期间 drop 也安全
ui::image(ui::ImageOptions { image: Some(img), width: Some(240.0), fit: zant::core::node::ImageFit::Contain, ..Default::default() })
```

- `from_pixels`（预乘 32 位像素，平台原生字节序）、`from_encoded`、
  `zant::image::Image::from_data_uri("data:image/png;base64,...")`。
- 创建是线程安全的：在后台线程解码 / 下载，交给 UI 线程后调用 `ui::post_wake()`。
- 节点持有自己的引用（`Image::retain` 是显式加引用计数）；图片在显示期间 drop 也安全。

## `ui::post_wake()`

线程安全：请 UI 线程尽快重建一次（连续多次合并为一次）。后台线程发布结果后调用，下一次
`render` 读取结果。Zant 的状态本身仍只在 UI 线程修改。

## 其他

- `ui::display_scale()`：当前显示器的像素密度（Retina 为 2，Windows 150% 为 1.5），用于选择
  渲染图片的分辨率。
- `std::sync::{Mutex, Condvar}`：工作线程用的互斥锁和条件变量。
