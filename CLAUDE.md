# CLAUDE.md

开始工作前先阅读同目录的 [AGENTS.md](AGENTS.md)，那里是完整规范。

## 快速上下文

- 项目：Zant UI，Zig + Skia + Metal + AppKit 的原生 macOS UI 框架。
- 入口/示例：`src/main.zig`、`src/gallery.zig`、`examples/declarative.zig`。
- 声明式 API：`src/ui.zig`；协调器：`src/ui_runtime.zig`。
- 组件：`src/components/`；样式统一入口：`ui.styled`。
- 脚手架：`tools/zant/main.zig`；模板：`scaffold/template/`。
- 测试：`zig build test`（当前 40/40）。
- 全局 CLI：`zant`；框架更新后执行 `zant update`。
- 热更新：`zant run --watch`（重启式，非沙盒）。
- PDF：`ui.pdfPreview`，原生 PDFKit。

## 改动原则

1. 先读 `AGENTS.md`，再改代码。
2. 保持状态单一来源：props 向下，事件向上。
3. 新组件必须是 `fn (props) ui.Node`，不依赖平台细节。
4. 新增外观属性要真正被 paint 使用；不要只加字段不渲染。
5. 改布局/渲染后补测试，并运行 `zig build test`。
6. 不提交密钥、证书、token、真实签名身份或本机绝对路径。
7. 提交前运行 `zig fmt`、检查 `git diff`，不要提交 `.zig-cache`、
   `zig-out`、`third_party` 或测试项目。
