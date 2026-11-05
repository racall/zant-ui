# 架构

## 分层

```text
src/main.zig / 生成项目 main.zig
        │
        ├── ui.zig              声明式 Node builder
        ├── ui_runtime.zig      Runtime、协调、事件、焦点、IME、帧调度
        │       ├── core/tree.zig
        │       ├── core/layout.zig
        │       └── renderer/skia
        └── platform/macos/window.m
```

## 窗口

AppKit `NSWindow` / `NSView` 负责原生窗口、窗口样式、事件、输入法、
剪贴板、光标和 DPI。窗口配置通过 `WindowOptions` 传入，包括尺寸、最小尺寸、
背景色、是否缩放、是否置顶、是否可拖拽背景移动、是否居中。

## 渲染

默认使用 Skia Ganesh + Metal。每帧从 `CAMetalLayer` 取 drawable，包装为
`GrBackendRenderTarget` 后绘制。`ZANT_RENDERER=raster` 可回退 CPU 光栅并
通过 `CGImage` 显示。

## 保留树与协调

`ui.*` 每帧在 arena 中构造轻量 Node；Runtime 按 key（其次索引）协调到保留树，
保留焦点、光标、选区、Hover 和输入法组合状态。布局为 Zig 实现的单行 Flex。

## 文本

Skia Paragraph 负责 shaping、换行、字体回退；布局和字形结果带缓存。输入框
有独立的光标/选区逻辑。

## 事件与重绘

窗口事件先经过 Runtime：命中测试、焦点、键盘、文本、IME、指针、滚轮。
只有状态变化或明确的失效才请求重绘；静态窗口不会持续动画。

## 平台范围

一期只实现 macOS。`src/platform/windows/` 目前只有二期说明，未实现。
