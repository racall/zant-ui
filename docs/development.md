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
```

## 环境变量

- `ZANT_RENDERER=metal`（默认）：Skia Ganesh Metal。
- `ZANT_RENDERER=raster`：CPU 光栅回退。
- `ZANT_SNAPSHOT=/path/out.png`：绘制后保存 PNG，用于视觉检查。

## 测试

`zig build test` 覆盖 Flex 布局、组件、声明式 UI、文本缓存和 Store。
测试不依赖真实窗口。

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
- `src/core/`：节点、树、布局、几何、Unicode。
- `src/renderer/skia/`：C ABI 与 Skia/Metal 桥。
- `src/platform/macos/window.m`：AppKit 窗口和输入事件。
- `tools/zant/`：编译后的脚手架 CLI。
