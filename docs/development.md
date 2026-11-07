# 开发

## 依赖

- Zig 0.17.1
- macOS 13+（Apple Silicon）、Xcode 命令行工具，Skia 预编译包（首次执行 `./scripts/fetch_skia.sh`）
- 或 Windows 10/11 x64、Visual Studio 2022+ / Build Tools（「使用 C++ 的桌面开发」），
  依赖（Skia、头文件、PDFium、STIX Two Math）首次执行
  `powershell -ExecutionPolicy Bypass -File scripts/fetch_skia_windows.ps1`。
  `zant_skia.dll` 由 `zig build` 调用 MSVC 构建（`scripts/win/build_bridge.ps1`，有缓存）。

在 macOS 上 `zig build -Dtarget=x86_64-windows` 可检查 Windows 代码能否编译（DLL 只能在 Windows 上构建）。
Windows 上 Debug 构建是控制台程序（双击运行会带控制台窗口，便于看 stderr），Release 构建为 GUI 子系统。

## 常用命令

```bash
./scripts/fetch_skia.sh    # macOS 首次；Windows 用 scripts/fetch_skia_windows.ps1
zig build
zig build test
zig build -Doptimize=ReleaseFast
zig build run              # 声明式示例
zig build run-gallery      # 命令式画廊
zig build run-markdown     # Markdown 渲染画廊（ZANT_DARK=1 深色）
zig build run-math         # 公式画廊
zig build run-components   # 全部组件总览（ZANT_DARK=1 深色）
zig build run-pdf -- a.pdf # PDF 预览（ZANT_PDF_PAGE=页码，ZANT_PDF_PROBE=1 只输出页数与文字长度）
```

## 环境变量

- 默认 GPU：macOS Skia Ganesh Metal，Windows Skia Ganesh Direct3D 12。
- `ZANT_RENDERER=raster`：CPU 光栅回退。
- `ZANT_SNAPSHOT=/path/out.png`：绘制后保存 PNG，用于视觉检查（截图模式关闭动效）。
- `ZANT_SNAPSHOT_EXIT=1`：截图后退出。
- `ZANT_NO_MOTION=1`：关闭动效（系统“减少动态效果”打开时也会关闭）。
- `ZANT_RAW_LOGS=1`：`zant run` 显示原始日志（macOS 默认隐藏输入法框架的无害日志；Windows 不过滤）。
- `ZANT_LAYOUT_FILE=/path/layout.txt`：每帧写出带 key 节点的位置（`key x y 宽 高`，逻辑像素，
  Tab 分隔），供 UI 自动化按 key 点击。
- `ZANT_FRAME_STATS=1`：每秒在 stderr 输出 `[frames]`：帧数、最慢一帧及其 rebuild / layout / paint
  耗时（Zig 与 Skia 录制，不含 GPU）、最长帧间隔、缩放时画出的新尺寸帧数与「尺寸变化 → 出帧」最长延迟；
  超过 8 ms 的帧另打一行 `[slow-frame]`（含当帧新建的文字排版数与耗时）。
- `ZANT_AUTOPLAY=1`（`zant-components`）：示例自己循环菜单、对话框、提示、主题等动画，
  用来无人值守地测帧率；`=quiet` 不打印 `[event]`。

性能要看 Release 构建（`zig build -Doptimize=ReleaseFast`）：Debug 构建的 rebuild 会慢 10 倍以上。
Windows 上写 stderr 很贵（每行约 10 ms），不要在每帧都会执行的路径里 `std.debug.print`。

视觉检查的示例文档：[`docs/showcase.md`](showcase.md) 覆盖全部 Markdown 与公式渲染。

## 测试

`zig build test`（154 项，macOS 与 Windows 都要通过）覆盖 Flex 布局、组件、声明式 UI 与协调、
节点生命周期、动效（假时钟）、文本、Store / SQLite、指针区域、文字选择、PDF 阅读器（选择 / 搜索 /
批注 / 导出）、Markdown（含 remend 522 条上游用例）与数学排版（含 mhchem 823 条用例全部可排版）。

## 帧基准 `zant-bench`

无窗口（光栅画布）驱动运行时，按场景统计整帧耗时（rebuild + layout + paint）：`open`（打开 30 条
约 5 KB 的长对话的首帧）、`stream`（流式回复）、`typing`（长文档上方输入）、`caret`（光标闪烁帧）、
`hover`（指针扫过长列表）、`resize`（窗口宽度变化）、`scroll`（滚轮）。`ZANT_BENCH_LIST=1` 用虚拟列表，
`ZANT_BENCH_MESSAGES` 改对话长度。结果确定、可复现，适合改动前后、两个平台对比：

```bash
git stash; zig build -Doptimize=ReleaseFast --prefix zig-out-baseline; git stash pop   # 基线
zig build -Doptimize=ReleaseFast --prefix zig-out-release
./zig-out-baseline/bin/zant-bench && ZANT_BENCH_LIST=1 ./zig-out-release/bin/zant-bench
```

## 流式 Markdown 基准

`zant-md-stream`（`examples/markdown_stream.zig`）模拟聊天：若干历史消息 + 一条流式回复，
工作线程追加文本后 `ui.postWake()`。用 ReleaseFast 构建测：

```bash
zig build -Doptimize=ReleaseFast --prefix zig-out-release
ZANT_FRAME_STATS=1 ZANT_STREAM_EXIT=1 ./zig-out-release/bin/zant-md-stream            # 内置回复
ZANT_STREAM_HISTORY=30 ZANT_STREAM_CHARS=12 ZANT_STREAM_MS=8 ./zig-out-release/bin/zant-md-stream long.md
curl -N … | ./zig-out-release/bin/zant-md-stream --sse                                  # 真实 SSE（text_delta）
./zig-out-release/bin/zant-md-stream --sse-replay recorded.sse                          # 回放录制的 SSE
```

参考（Apple Silicon / i7-13700KF，4K 150%）：真实对话接口回放最慢一帧约 7 ms；30 条长历史 +
29 KB 回复以约 1200 字节/秒输出时稳定 60 帧。示例用虚拟列表渲染聊天，打开长对话只排版可见的消息。
测试不依赖真实窗口；时间相关测试注入假时钟（`runtime.clock`），不要 sleep。

## Windows 测试机

在 Mac 上开发、到 Windows 测试机构建和验证。主机与目录由 `ZANT_WIN_HOST`（`user@host`）和
`ZANT_WIN_DIR`（父目录，如 `D:/dev`）指定，可写在不提交的 `scripts/win/.env` 里；测试机上
需要 Zig 在 PATH 上（或设 `ZANT_WIN_ZIG`）。

```bash
scripts/win/sync.sh ../stillmd                     # 同步框架和应用（保持同级目录）
scripts/win/remote.sh XyUI 'zig build test'        # 远程执行（UTF-8 输出，Zig 在 PATH 上）
scripts/win/remote.sh XyUI 'powershell -File scripts/win/run-desktop.ps1 -Exe zig-out\bin\zant-demo.exe -Env "ZANT_SNAPSHOT=$env:TEMP\a.png;ZANT_SNAPSHOT_EXIT=1" -Wait 30'
```

SSH 会话里的窗口不可见、也拿不到真实缩放，所以视觉检查用 `run-desktop.ps1` 在已登录的
桌面会话里运行。桌面自动化测试（都输出 PASS / FAIL 报告和截图）：

| 脚本 | 内容 |
|---|---|
| `scripts/win/ui-auto.ps1` | 公共函数：启动应用（`ZANT_LAYOUT_FILE`）、按 key 点击 / 悬停、截图、核对 `[event]` 输出 |
| `scripts/win/components-check.ps1` | 点击 `zant-components` 的每个组件（按钮、输入、选择、菜单、对话框、提示、深色…） |
| `scripts/win/smoke-stillmd.ps1 -App <still. app.exe>` | 驱动 still.：输入、保存（CRLF）、剪贴板、对话框、拼音输入法、标题栏、拖动、关闭确认 |
| `scripts/win/docs-check.ps1 -App <still. app.exe>` | 用 still. 打开 `docs/` 下每个 md（预览 / 编辑）并截图 |
| `scripts/win/resize-check.ps1` | 真实鼠标 120 Hz 拖动窗口右下角缩放（含缩到最小），统计缩放帧率与延迟；先跑 `ZANT_AUTOPLAY` 测动画帧率；检查布局与最小尺寸 |
| `scripts/win/scaffold-check.ps1` | 在真实命令行窗口（代码页 936）运行 `zant create`，用 Unicode 按键输入中文标题，截图并检查生成的 `app.config.json` |
| `scripts/win/stream-resize-check.ps1 -Sse <录制的 SSE>` | `zant-md-stream` 回放真实回复的同时，真实鼠标拖动标题栏移动窗口、拖右下角放大 / 缩到最小 / 还原，每步截图，统计帧时间 |
| `scripts/win/pdf-check.ps1 -Folder <目录>` | 对目录里的 PDF 检查页数、文字、链接与目录、首页 / 中间页截图；真实鼠标拖选 + Ctrl+C、双击选词、缩放、滚轮、搜索、翻页 |

用 `run-desktop.ps1 -Exe powershell.exe -Arguments "-NoProfile -ExecutionPolicy Bypass -File <脚本>"`
在桌面会话运行；结果与截图在 `%TEMP%\zant-checks\` 下。含中文的 `.ps1` 要保存为带 BOM 的
UTF-8（Windows PowerShell 5.1）。

## 脚手架

`tools/zant/` 编译为 `zig-out/bin/zant`：

```bash
zig build
./zig-out/bin/zant create my-app
```

生成项目的 `build.zig` 通过 `zant_ui` 依赖导入框架，并调用 `configureApp`（Windows 上嵌入 manifest、
Release 用 GUI 子系统、把 `zant_skia.dll` / `pdfium.dll` 装到 exe 旁；macOS 上无操作）；模板位于
`scaffold/template/`，由 CLI 写入目标目录。框架更新后执行 `zant update` 重编译全局 CLI。详见 [脚手架](scaffold.md)。

## 目录

- `src/ui.zig`：声明式 builders。
- `src/ui_runtime.zig`：协调器、事件、焦点、IME、帧调度。
- `src/core/`：节点、树、布局、几何、Unicode、图标、矢量绘制、结构化哈希。
- `src/motion/`：缓动曲线、Animated、presence 预设。
- `src/markdown/`、`src/math/`：Markdown（Streamdown 移植）与 LaTeX 公式。
- `src/components/`：antd 风格组件（菜单、Segmented、Modal、Message …）。
- `src/renderer/skia/`：C ABI 与 Skia 桥（可移植核心 + mac / win 平台文件）。
- `src/platform/platform.h`：平台 C ABI；`macos/window.m`（AppKit）与 `windows/window.c`（Win32）实现。
- `scripts/win/`：Windows 桥接 DLL 构建、测试机同步、远程执行与桌面冒烟测试。
- `src/core/keys.zig`、`src/core/paths.zig`：平台快捷键（`zant.keys`）与应用数据目录（`zant.paths`）。
- `src/db/`：SQLite 封装（macOS 系统 libsqlite3，Windows 系统 `winsqlite3.dll`）。
- `tools/zant/`：编译后的脚手架 CLI。
