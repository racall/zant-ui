# 架构

## 分层

```text
src/main.zig / 生成项目 main.zig
        │
        ├── ui.zig              声明式 Node builder、ui.styled、命令式辅助
        ├── launch.zig          最小入口（theme / onKey / onCloseRequest 钩子）
        ├── ui_runtime.zig      Runtime：协调、事件、焦点、IME、动画、帧调度
        │       ├── core/tree.zig      保留树、命中测试、绘制、FLIP
        │       ├── core/layout.zig    Flex 布局
        │       ├── motion/            缓动、Animated、presence
        │       └── renderer/skia      Skia C ABI（可移植核心；Metal / Direct3D 12 / 光栅）
        ├── markdown/, math/    Markdown 与公式（基于 ui.* 与矢量绘制）
        └── platform/platform.h        平台 C ABI，两份实现：
                ├── macos/window.m     AppKit 窗口、事件、输入法、显示链接
                └── windows/window.c   Win32 窗口、事件、IMM32、自绘标题栏、对话框
```

## 窗口

macOS 由 AppKit `NSWindow` / `NSView`、Windows 由 Win32 `HWND` 负责原生窗口、窗口样式、
事件、输入法、剪贴板、光标和 DPI（Windows 为每显示器 DPI v2，逻辑坐标 = 像素 / 缩放）。
`WindowOptions` 传入尺寸、最小尺寸、背景色、缩放、置顶、拖拽、居中，以及隐藏标题栏时的
自定义标题栏高度（交通灯垂直居中）。关窗 / 退出前会询问应用（`onCloseRequest`）；
文档未保存状态通过 `ui.documentEdited` 显示（macOS 关闭按钮上的圆点，Windows 标题前的 `*`）。
窗口底色跟随根节点背景，Windows 还据此切换深 / 浅色系统标题栏（DWM）。

Windows 的隐藏标题栏：`WM_NCCALCSIZE` 去掉系统标题栏但保留缩放边框、阴影和贴靠；
`WM_NCHITTEST` 询问 Runtime（`ZantHost.titlebar_hit`）——交互控件可点击，其余区域拖动窗口，
运行时自动叠加的最大化按钮返回 `HTMAXBUTTON`，以便 Windows 11 显示贴靠布局浮层。

## 渲染

默认 GPU：macOS 每帧从 `CAMetalLayer` 取 drawable；Windows 使用 Direct3D 12 flip-model
交换链（帧延迟 1，`Present(1)` 垂直同步）。二者都包装为 Skia render target 后绘制。
GPU 不可用或 `ZANT_RENDERER=raster` 时回退 CPU 光栅，分别通过 `CGImage` / `SetDIBitsToDevice` 显示。
滚动容器只绘制可见区域附近的子节点。

Skia 桥接层 `skia_bridge.cpp` 是可移植 C++；字体管理器、字体族名和 GPU 交换链在
`skia_bridge_mac.mm`（CoreText、Metal）/ `skia_bridge_win.cpp`（DirectWrite、D3D12）中实现
（接口见 `skia_bridge_internal.h`）。Windows 上预编译 Skia 是 MSVC 的 STL，因此桥接层、
Skia、ICU 数据和 STIX Two Math 字体一起编成 `zant_skia.dll`（`scripts/win/build_bridge.ps1`），
Zig 侧只通过 `skia_bridge.h` 的 C ABI 链接它（导入库由 `zant_skia.def` 生成）。

## 保留树与协调

`ui.*` 每帧在 frame arena 中构造轻量 Node；Runtime 把它协调到保留树：

- 有 key 的节点只匹配同 key 节点；无 key 的按顺序匹配；`ui.memo` 的 hash 不变时整棵子树跳过。
- 根节点先归一化再比较，无变化的重建不会触发布局。
- 被移除的节点释放内存、槽位进入空闲表复用；运行时保存的悬停 / 按下 / 焦点 / 提示等引用
  在 `forgetNode` 中清理。带 presence 的节点先播放退场动画（不参与布局和命中测试），
  结束后再释放；同 key 节点重新出现时复活原节点。
- 保留树保存焦点、光标、选区、悬停和输入法组合状态，并深拷贝所有字符串与绘制数据。

## 布局

Zig 实现的 CSS 风格 Flex：方向、换行、justify / align、grow / shrink / basis（多轮冻结）、
百分比尺寸、order、align-self、绝对定位（含 left + right 拉伸）、`anchor`（绝对定位子节点
跟随第 n 个流式兄弟节点，用于分段控制器滑块、标签页下划线）、纵向滚动容器。

## 文本与矢量

Skia Paragraph 负责 shaping、换行、字体回退、装饰线和单行省略号；布局结果按宽度缓存。
富文本支持多段样式、链接与内嵌绘制（行内公式与正文基线对齐）。公式（`math/`）与
Mermaid 图表输出保留式矢量绘制（字形、矩形、线、路径），可整体缩放适配内容列。

默认字体（正文 / 中文 / 等宽 / 衬线中文）：macOS SF Pro Text / PingFang SC / Menlo / Songti SC；
Windows Segoe UI / Microsoft YaHei UI / Cascadia Mono（回退 Consolas）/ SimSun。公式用 STIX Two Math（macOS 系统自带，Windows 嵌在 `zant_skia.dll` 中）。

## 动效与帧调度

组件以布尔状态为唯一来源，在 `animate(data, now, theme)` 中用基于时间戳的
`motion.Animated` 缓动出呈现值；状态变化处 `tree.poke(id)` 登记动画节点，Runtime 每帧只推进
`tree.animating` 列表。进场 / 退场与 FLIP 变换在绘制时应用（必要时 `saveLayerAlpha`）。
动画帧只重绘、不重建。有动画时按屏幕刷新出帧（macOS `CADisplayLink`；Windows 的帧都从
`WM_PAINT` 绘制，由交换链或 `DwmFlush` 垂直同步节拍，拖动缩放的模态循环里也会继续）；
空闲时只在光标闪烁、提示延迟或 `ui.wakeAfter` 到期时用一次性定时器唤醒；鼠标移动只在界面确有变化时重绘。

## 平台范围

macOS 13+（Apple Silicon，预编译 Skia 为 arm64）与 Windows 10/11 x64。平台差异只在 `src/platform/` 与 Skia 桥接的平台文件中；
组件和应用通过 `zant.keys`（⌘ / Ctrl、按键文字）、`ui.titlebarInsets()`、`zant.paths`、
`zant.dialogs` 获得平台习惯，不写 `builtin.os.tag` 分支。
