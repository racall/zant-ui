# CLAUDE.md

请先阅读同目录的 [AGENTS.md](AGENTS.md)，那是本项目完整的开发规范。

## 快速上下文

- 项目：`__APP_TITLE__`（Zig + Skia + Metal + AppKit）。
- Bundle ID：`__BUNDLE_ID__`。
- 入口：`src/main.zig`。
- 状态：`src/state.zig` 中的 `AppState`，由 main 持有。
- 组件：`src/components/`，父组件通过 props 传值，子组件通过回调向上发事件。
- 配置：`app.config.json`；`src/app_config.zig`、`Info.plist`、`app.entitlements`
  由 `zant` 生成，不要手改。
- 运行：`zant run`。
- 打包：`zant package`。
- 测试：`zig build test`。
- 格式化：`zig fmt src build.zig`。

## 改动原则

1. 先读 `AGENTS.md` 和 `src/main.zig`。
2. 保持状态单一来源；不要把业务状态放进 UI 组件内部。
3. 新组件写成纯函数 `fn (props) ui.Node`。
4. 不引入平台相关代码到业务组件。
5. 修改后运行格式化、测试，并用 `zant run` 验证窗口。
