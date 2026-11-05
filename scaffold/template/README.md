# __APP_TITLE__

由 Zant UI 生成的 macOS 原生桌面应用。

- Bundle ID：`__BUNDLE_ID__`
- 沙盒：`__SANDBOX__`
- 打包账号：`__SIGN_IDENTITY__`

## 运行

在项目目录执行：

```bash
zant run
# 或开发时自动重编译、重启：
zant run --watch
```

它会读取 `app.config.json`，自动生成配置代码、编译并启动应用。

## 构建

```bash
zant build
zant build --release
```

## 打包 .app

```bash
zant package
```

生成并签名 `zig-out/__APP_TITLE__.app`。

## 配置

编辑 `app.config.json`：窗口尺寸、最小尺寸、标题栏风格、背景色、是否沙盒、
签名账号等。代码中的 `WindowOptions` 优先级高于配置文件。

详见 Zant UI 的 `docs/window-config.md`。

## 示例结构

- `src/state.zig`：共享状态 `AppState` 和动作。
- `src/components/stat_card.zig`：自定义展示组件，演示父传子 props。
- `src/components/counter_card.zig`：自定义子组件，演示 props 和事件回调。
- `src/components/dashboard.zig`：自定义父组件，组合上述子组件。
- `src/main.zig`：状态入口，并把 `AppState` 传给 dashboard。

事件路径：子组件按钮 → 父组件回调 → `AppState` → Runtime 重绘。

## AI Coding Agent

项目包含：

- `AGENTS.md`：Codex 等 agent 的项目规范。
- `CLAUDE.md`：Claude Code 的入口说明，引用 AGENTS.md。

修改前建议先读这两个文件。
