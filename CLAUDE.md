# CLAUDE.md

开始工作前先阅读同目录的 [AGENTS.md](AGENTS.md)，那里是完整规范。

## ★ 最高优先级：设计规范 [DESIGN.md](DESIGN.md)

所有界面与交互的设计、实现和评审都**必须**遵循同目录的 [DESIGN.md](DESIGN.md)（UI/UX 设计指导原则）。
它的优先级高于本文件中的其他所有条目；与其他约定（包括 antd 风格规范）冲突时以 DESIGN.md 为准。
改动界面前先读它；完成后按其第 6 条检查不同窗口尺寸、浅/深色主题和各交互状态。

## 快速上下文

- 项目：Zant UI，Zig + Skia + Metal + AppKit 的原生 macOS UI 框架。
- 入口/示例：`src/main.zig`、`src/gallery.zig`、`examples/declarative.zig`。
- 声明式 API：`src/ui.zig`；协调器：`src/ui_runtime.zig`。
- 组件：`src/components/`；样式统一入口：`ui.styled`。
- 脚手架：`tools/zant/main.zig`；模板：`scaffold/template/`。
- 测试：`zig build test`（当前 123/123，含 remend 522 条上游用例）。
- 动效：`src/motion/`，规范见 `docs/motion.md` 与 `AGENTS.md`「动效规范」；
  `ZANT_NO_MOTION=1` 关闭动效。
- 全局 CLI：`zant`；框架更新后执行 `zant update`。
- 热更新：`zant run --watch`（重启式，非沙盒，监视 `src/` 下所有 .zig）。
- `zant run` 会隐藏 macOS 输入法框架的无害日志（IMKCFRunLoopWakeUpReliable 等），
  需要原始输出时设 `ZANT_RAW_LOGS=1`。
- PDF：`ui.pdfPreview`，原生 PDFKit。
- Markdown：`src/markdown/`（Streamdown 移植，模块化），数学：`src/math/`（KaTeX 算法）。
  详见 `docs/markdown.md`；示例 `zig build run-markdown` / `run-math`。
- 视觉检查：`ZANT_SNAPSHOT=out.png ZANT_SNAPSHOT_EXIT=1 ./zig-out/bin/<app>`。

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
6. 不提交密钥、证书、token、真实签名身份或本机绝对路径。
7. 提交前运行 `zig fmt`、检查 `git diff`，不要提交 `.zig-cache`、
   `zig-out`、`third_party` 或测试项目。
