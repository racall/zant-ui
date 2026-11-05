# 开发

## 依赖

- Zig 0.17.1
- macOS 13+
- Xcode 命令行工具
- Skia 预编译包（首次执行 `./scripts/fetch_skia.sh`）

## 常用命令

```bash
./scripts/fetch_skia.sh
zig build
zig build test
zig build -Doptimize=ReleaseFast
zig build run              # 声明式示例
zig build run-gallery      # 命令式画廊
zig build run-markdown     # Markdown 渲染画廊（ZANT_DARK=1 深色）
zig build run-math         # 公式画廊
```

## 环境变量

- `ZANT_RENDERER=metal`（默认）：Skia Ganesh Metal。
- `ZANT_RENDERER=raster`：CPU 光栅回退。
- `ZANT_SNAPSHOT=/path/out.png`：绘制后保存 PNG，用于视觉检查（截图模式关闭动效）。
- `ZANT_SNAPSHOT_EXIT=1`：截图后退出。
- `ZANT_NO_MOTION=1`：关闭动效（系统“减少动态效果”打开时也会关闭）。
- `ZANT_RAW_LOGS=1`：`zant run` 显示原始日志（默认隐藏输入法框架的无害日志）。

视觉检查的示例文档：[`docs/showcase.md`](showcase.md) 覆盖全部 Markdown 与公式渲染。

## 测试

`zig build test`（123 项）覆盖 Flex 布局、组件、声明式 UI 与协调、节点生命周期、动效
（假时钟）、文本、Store / SQLite、Markdown（含 remend 522 条上游用例）与数学排版。
测试不依赖真实窗口；时间相关测试注入假时钟（`runtime.clock`），不要 sleep。

## 脚手架

`tools/zant/` 编译为 `zig-out/bin/zant`：

```bash
zig build
./zig-out/bin/zant create my-app
```

生成项目的 `build.zig` 通过 `zant_ui` 依赖导入框架；模板位于
`scaffold/template/`，由 CLI 写入目标目录。

## 目录

- `src/ui.zig`：声明式 builders。
- `src/ui_runtime.zig`：协调器、事件、焦点、IME、帧调度。
- `src/core/`：节点、树、布局、几何、Unicode、图标、矢量绘制、结构化哈希。
- `src/motion/`：缓动曲线、Animated、presence 预设。
- `src/markdown/`、`src/math/`：Markdown（Streamdown 移植）与 LaTeX 公式。
- `src/components/`：antd 风格组件（菜单、Segmented、Modal、Message …）。
- `src/renderer/skia/`：C ABI 与 Skia/Metal 桥。
- `src/platform/macos/window.m`：AppKit 窗口和输入事件。
- `tools/zant/`：编译后的脚手架 CLI。
