# CLAUDE.md

请先阅读同目录的 [AGENTS.md](AGENTS.md)，那是本项目完整的开发规范。

## ★ 最高优先级：设计规范 [DESIGN.md](DESIGN.md)

所有界面与交互的设计、实现和评审都**必须**遵循同目录的 [DESIGN.md](DESIGN.md)（UI/UX 设计指导原则）。
它的优先级高于本文件中的其他所有条目；与其他约定（包括 antd 风格规范）冲突时以 DESIGN.md 为准。
改动界面前先读它；完成后按其第 6 条检查不同窗口尺寸、浅/深色主题和各交互状态。

## 快速上下文

- 项目：`__APP_TITLE__`（Zig + Skia；macOS Metal + AppKit，Windows Direct3D 12 + Win32）。
- 应用标识（bundle_id）：`__BUNDLE_ID__`。
- 入口：`src/main.zig`。
- 状态：`src/state.zig` 中的 `AppState`，由 main 持有。
- 组件：`src/components/`，父组件通过 props 传值，子组件通过回调向上发事件。
- 配置：`app.config.json`（两端通用设置 + `macos` 专属段）；`src/app_config.zig`、
  `Info.plist`、`app.entitlements`（后两者仅 macOS）由 `zant` 生成，不要手改。
- 运行：`zant run`（`--watch` 改动后自动重编译、重启）。
- 打包：`zant package`。
- 测试：`zig build test`。
- 格式化：`zig fmt src build.zig`。

## 改动原则

1. 先读 `AGENTS.md` 和 `src/main.zig`。
2. 保持状态单一来源；不要把业务状态放进 UI 组件内部。
3. 新组件写成纯函数 `fn (props) ui.Node`。
4. 不引入平台相关代码到业务组件；通用组件（模态框、菜单、快捷键列表…）用框架的 `ui.*`，
   缺的话补进 Zant UI，而不是写在应用里。
5. 修改后运行格式化、测试，并用 `zant run` 验证窗口。
