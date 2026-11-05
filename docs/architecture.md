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
        │       └── renderer/skia      Skia C ABI（Metal / 光栅）
        ├── markdown/, math/    Markdown 与公式（基于 ui.* 与矢量绘制）
        └── platform/macos/window.m    AppKit 窗口、事件、输入法、显示链接
```

## 窗口

AppKit `NSWindow` / `NSView` 负责原生窗口、窗口样式、事件、输入法、剪贴板、光标和 DPI。
`WindowOptions` 传入尺寸、最小尺寸、背景色、缩放、置顶、拖拽、居中，以及隐藏标题栏时的
自定义标题栏高度（交通灯垂直居中）。关窗 / 退出前会询问应用（`onCloseRequest`）；
文档未保存状态通过 `ui.documentEdited` 显示在关闭按钮上。窗口底色跟随根节点背景。

## 渲染

默认 Skia Ganesh + Metal：每帧从 `CAMetalLayer` 取 drawable，包装为 Skia render target
后绘制。`ZANT_RENDERER=raster` 回退 CPU 光栅并通过 `CGImage` 显示。滚动容器只绘制可见区域
附近的子节点。

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

## 动效与帧调度

组件以布尔状态为唯一来源，在 `animate(data, now, theme)` 中用基于时间戳的
`motion.Animated` 缓动出呈现值；状态变化处 `tree.poke(id)` 登记动画节点，Runtime 每帧只推进
`tree.animating` 列表。进场 / 退场与 FLIP 变换在绘制时应用（必要时 `saveLayerAlpha`）。
动画帧只重绘、不重建。有动画时由 `CADisplayLink` 按屏幕刷新出帧，空闲时只在光标闪烁、
提示延迟或 `ui.wakeAfter` 到期时用一次性定时器唤醒；鼠标移动只在界面确有变化时重绘。

## 平台范围

一期只实现 macOS。`src/platform/windows/` 目前只有二期说明，未实现。
