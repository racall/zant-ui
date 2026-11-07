# 指针区域、尺寸变化、内存图片、后台唤醒

自定义交互（画布、分隔条、拖动手柄、文档阅读器）用到的底层能力。PDF 阅读器就建立在它们之上。

## `ui.pointerArea`

一个接收原始指针输入的容器：

```zig
fn dragSplit(self: *State, info: ui.PointerInfo) bool {
    switch (info.phase) {
        .down => self.drag_start = .{ info.window_x, self.split },
        .drag => self.split = std.math.clamp(self.drag_start[1] + info.window_x - self.drag_start[0], 200, 600),
        else => return false,
    }
    return true; // 状态变了：重建界面
}

ui.pointerArea(.{
    .box = .{ .key = "splitter", .width = 6, .height_percent = 100 },
    .cursor = .resize_horizontal,
    .on_pointer = ui.pointerAction(self, State.dragSplit),
})
```

- `on_pointer` 收到 `ui.PointerInfo`：`phase`（`down` / `drag` / `up` / `hover` / `leave` /
  `scroll`）、相对区域左上角的 `x` / `y`、窗口坐标 `window_x` / `window_y`、区域尺寸、
  `button`、`modifiers`、`click_count`（双击 2、三击 3）、滚动量 `dx` / `dy`。
  返回 `true` 表示状态变了、需要重建。
- 按下后区域**捕获指针**：移出区域、移出窗口都继续收到 `drag`，松开收到 `up`；窗口失焦时
  也会补一个 `up`。拖动距离超过 4px 的按下不计入多击。
- `capture_scroll = true`：滚轮 / 触控板滚动交给 `on_pointer`（`.scroll`），不再滚动外层
  滚动容器。⌘/Ctrl + 滚轮与触控板捏合都以 Control + 滚轮送达（捏合 `dy = 250 × 放大量`），
  可缩放视图同样处理。
- `cursor`：区域上的指针形状（`arrow`、`pointing_hand`、`ibeam`、`crosshair`、
  `resize_horizontal`、`resize_vertical`、`grab`、`grabbing`、`not_allowed`）。
- `focusable = true`：点击获得键盘焦点（Tab 也能到达），按键先交给 `on_key`
  （`ui.keyAction`，返回 `true` 表示已处理）。
- 区域里的按钮、输入框等交互控件照常接收自己的点击。

## 悬停样式 `hover_background` / `hover_border_color`

```zig
ui.row(.{ .height = 32, .radius = 6, .hover_background = theme.color_fill_secondary, .children = … })
```

指针在容器上时显示的背景 / 边框，带 antd 的 200 ms 过渡，只重绘、不重建界面，应用不必保存悬停
状态。指针在容器里的按钮等控件上时，容器也算悬停（CSS `:hover`）；模态框等遮挡层盖住时不显示。

## 焦点范围 `BoxOptions.focus_scope`

Tab / Shift+Tab 只在最上层的焦点范围内循环。`ui.modal` 自带：打开对话框后 Tab 不会跳到背后的控件；
焦点在范围外时，Tab 进入范围的第一个控件。

## `BoxOptions.on_resize`

容器尺寸变化（包括首次布局）后调用，然后**同一帧内**重建并重新布局，所以新尺寸不会先闪一帧
旧布局。回调里可用 `ui.keyBounds(key)` 读新尺寸。适合“适合宽度”的阅读器、响应式列数等。

## 内存图片 `zant.image.Image`

```zig
const img = zant.image.Image.fromEncoded(png_bytes) orelse return; // PNG / JPEG / GIF / BMP / ICO
defer img.deinit();
ui.image(.{ .image = img, .width = 240, .fit = .contain })
```

- `fromPixels`（预乘 32 位像素，平台原生字节序 `zant.image.native_order`）、`fromEncoded`、
  `zant.image.fromDataUri(allocator, "data:image/png;base64,...")`。
- 创建是线程安全的：在后台线程解码 / 下载，交给 UI 线程后调用 `ui.postWake()`。
- 节点持有自己的引用：图片在显示期间 `deinit` 也安全。

## `ui.postWake()`

线程安全：请 UI 线程尽快重建一次（连续多次合并为一次）。后台线程发布结果后调用，下一次
`render` 读取结果。Zant 的状态本身仍只在 UI 线程修改。

## 其他

- `ui.displayScale()`：当前显示器的像素密度（Retina 为 2，Windows 150% 为 1.5），用于选择
  渲染图片的分辨率。
- `zant.sync.Mutex` / `Condition`（`src/core/sync.zig`）：工作线程用的互斥锁和条件变量
  （macOS pthread，Windows SRW 锁）。
