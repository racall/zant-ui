# CLAUDE.md

开始工作前先阅读同目录的 [AGENTS.md](AGENTS.md)，那里是完整规范。

## ★ 最高优先级：设计规范 [DESIGN.md](DESIGN.md)

所有界面与交互的设计、实现和评审都**必须**遵循同目录的 [DESIGN.md](DESIGN.md)（UI/UX 设计指导原则）。
它的优先级高于本文件中的其他所有条目；与其他约定（包括 antd 风格规范）冲突时以 DESIGN.md 为准。
改动界面前先读它；完成后按其第 6 条检查不同窗口尺寸、浅/深色主题和各交互状态。

## 快速上下文

- 项目：Zant UI，Zig + Skia 的原生桌面 UI 框架（macOS：Metal + AppKit；Windows：Direct3D 12 + Win32）。
- 入口/示例：`src/main.zig`、`src/gallery.zig`、`examples/declarative.zig`、
  `examples/components.zig`（全部组件，`zig build run-components`）、`examples/pdf.zig`（`run-pdf`）。
- 声明式 API：`src/ui.zig`；协调器：`src/ui_runtime.zig`。
- 组件：`src/components/`；样式统一入口：`ui.styled`。
- 脚手架：`tools/zant/main.zig`；模板：`scaffold/template/`。
- 测试：`zig build test`（当前 164/164，含 remend 522 条、mhchem 823 条上游用例；macOS 与 Windows 都要过）。
- Windows：测试机见 `AGENTS.md`「跨平台规范」；`scripts/win/` 同步、远程构建、桌面冒烟测试。
- 动效：`src/motion/`，规范见 `docs/motion.md` 与 `AGENTS.md`「动效规范」；
  `ZANT_NO_MOTION=1` 关闭动效。
- 全局 CLI：`zant`；框架更新后执行 `zant update`。一键安装 `install.sh` / `install.ps1`（发布包，
  `scripts/release/`，推送 `v*` 标签由 `.github/workflows/release.yml` 发布，见 `docs/development.md`「发布」）。
- 发布构建：`zant build [配置文件]`（`build.config.json`，`docs/build.md`）：zant 读配置、提问、编译签名，
  平台步骤在 `tools/zant/package/` 脚本（macOS DMG / 公证 / App Store，Windows Inno Setup 安装包 / MSIX）。
- 热更新：`zant run --watch`（重启式，非沙盒，监视 `src/` 下所有 .zig）。
- macOS 上 `zant run` 会隐藏输入法框架的无害日志（IMKCFRunLoopWakeUpReliable 等），
  需要原始输出时设 `ZANT_RAW_LOGS=1`。
- PDF：阅读器 `zant.pdf.view`（`src/pdf/`：连续滚动、缩放、选择复制、搜索、批注与导出；
  算法参照 EmbedPDF），单页预览 `ui.pdfPreview`；macOS PDFKit，Windows PDFium。
- 原始指针输入：`ui.pointerArea`（拖动捕获、多击、滚轮、按键）；尺寸变化 `BoxOptions.on_resize`；
  内存图片 `zant.image.Image` + `ui.image(.{ .image = .. })`；后台线程唤醒 UI `ui.postWake()`。
- Markdown：`src/markdown/`（Streamdown 移植，模块化），数学：`src/math/`（KaTeX 算法，
  `mhchem.zig` 为 KaTeX contrib/mhchem 的逐字节对齐移植）。详见 `docs/markdown.md`；
  示例 `zig build run-markdown` / `run-math` / `run-md-stream`（流式帧率基准，可读真实 SSE）。
- 性能基准：`zig build bench`（`zant-bench`：打开长对话、流式、打字、光标、悬停、缩放、滚动；
  `ZANT_BENCH_LIST=1` 用虚拟列表）。优化前先建基线（ReleaseFast，另装到 `zig-out-baseline`），两边平台对比。
- 桌面集成：`zant.desktop`（`docs/desktop.md`）：多窗口 / `.panel` 快捷面板、托盘与状态栏、全局快捷键、
  剪贴板监听与图片、单实例、深色模式；钩子 `onStart` / `onAppEvent` / `onWake` / `onQuitRequest`。
  组件 `ui.clickable`、`ui.hotkeyInput`、`BoxOptions.show_on_hover`。示例项目 ClipHop。
- 虚拟列表：`ui.list` + `ui.ListState`（`docs/components/list.md`）；悬停样式 `BoxOptions.hover_background`；
  焦点范围 `BoxOptions.focus_scope`（`ui.modal` 自带）。
- 流式性能：改 Markdown / 布局 / 文本后，用 ReleaseFast 的 `zant-md-stream` 配 `ZANT_FRAME_STATS=1`
  复测（长回复 + 多条历史），单帧应远低于 16.7 ms，`[slow-frame]` 的 text_layouts 不应随内容增长。
- 视觉检查：`ZANT_SNAPSHOT=out.png ZANT_SNAPSHOT_EXIT=1 ./zig-out/bin/<app>`；Windows 的窗口要在
  已登录桌面里看（`scripts/win/run-desktop.ps1`），SSH 会话里不可见。
- 自动化与性能：`ZANT_LAYOUT_FILE`（按 key 定位控件）、`ZANT_FRAME_STATS=1`（每秒帧统计，
  性能看 Release 构建）、`zant-components` 的 `ZANT_AUTOPLAY=quiet`；Windows 桌面测试脚本见
  `docs/development.md`「Windows 测试机」。
- 平台差异不写进组件和应用：快捷键用 `zant.keys`，自绘标题栏用 `ui.titlebarInsets()`，
  浮层跟随控件用 `ui.keyBounds()`，数据目录用 `zant.paths`（见 `AGENTS.md`「跨平台规范」）。

## 改动原则

0. **所有组件风格参考 Ant Design (antd 5)**：尺寸、圆角、间距、颜色 token、阴影和
   悬停/按下/禁用状态都按 antd 规范实现，浅色/深色都要正确；图标用 `ui.icon` 图标集，
   弹出菜单用 `ui.dropdownMenu` + `ui.dismissLayer`。详见 `AGENTS.md`「组件与样式规范」。
   每个组件的状态变化都要有 antd 风格的细腻过渡（`docs/motion.md`）；动画帧只重绘不重建。
   通用组件（模态框、快捷键列表、菜单…）一律放进 Zant UI（`src/components/` + `ui.*` 导出），
   应用里只写业务数据；应用缺通用组件时先补框架。

1. 先读 `AGENTS.md`，再改代码。
2. 保持状态单一来源：props 向下，事件向上。
3. 新组件必须是 `fn (props) ui.Node`，不依赖平台细节。
4. 新增外观属性要真正被 paint 使用；不要只加字段不渲染。
5. 改布局/渲染后补测试，并运行 `zig build test`。
6. 不提交密钥、证书、token、真实签名身份或本机绝对路径；Windows 测试机的地址只写在
   不提交的 `scripts/win/.env` 里。
7. 提交前运行 `zig fmt`、检查 `git diff`，不要提交 `.zig-cache`、
   `zig-out`、`third_party` 或测试项目；改了 `skia_bridge.h` / `pdf.h` 的函数要运行
   `scripts/win/gen_def.sh`，并确认 macOS 与 Windows 都通过 `zig build test`。
