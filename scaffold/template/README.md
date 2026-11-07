# __APP_TITLE__

由 Zant UI 生成的原生桌面应用（macOS 与 Windows）。

- 应用标识（bundle_id）：`__BUNDLE_ID__`（macOS 的 Bundle ID，Windows 的数据目录名）
- macOS 专属设置（沙盒、签名账号）在 Mac 上首次 `zant run` / `zant package` 时自动补充到
  `app.config.json` 的 `macos` 段

## 运行

在项目目录执行：

```bash
zant run
# 或开发时自动重编译、重启：
zant run --watch
```

它会读取 `app.config.json`，自动生成配置代码、编译并启动应用。
Windows 上需要 Visual Studio 2022+ / Build Tools 的 C++ 工作负载（构建 `zant_skia.dll`）。

## 构建

```bash
zant build
zant build --release
```

Windows 上 Debug 构建是控制台程序，`--release` 为 GUI 子系统。

## 打包

```bash
zant package
```

- macOS：生成并签名 `zig-out/__APP_TITLE__.app`（沙盒开启时写入 entitlements）。
- Windows：生成 `zig-out/__APP_TITLE__/` 目录（exe、`zant_skia.dll`、`pdfium.dll` 与
  MSVC 运行库），压缩即可分发。

## 配置

编辑 `app.config.json`：窗口尺寸、最小尺寸、标题栏风格、背景色，以及 macOS 的沙盒、
签名账号等。代码中的 `WindowOptions` 优先级高于配置文件。

详见 Zant UI 的 `docs/window-config.md`。

## 图标

`assets/icons/` 下是默认图标，替换同名 PNG 即可（尺寸不变）：`app-macos.png`（1024，macOS 应用图标）、
`app-windows.png`（256，Windows 应用图标）、`tray.png`（Windows 托盘）、`statusbar.png`
（macOS 菜单栏模板图，黑色 + 透明）。代码中用 `@embedFile("icon_tray")` 等引用。
规范见 Zant UI 的 `docs/scaffold.md`「图标」。

## 示例结构

- `src/state.zig`：共享状态 `AppState` 和动作。
- `src/components/stat_card.zig`：自定义展示组件，演示父传子 props。
- `src/components/counter_card.zig`：自定义子组件，演示 props 和事件回调。
- `src/components/dashboard.zig`：自定义父组件，组合上述子组件。
- `src/main.zig`：状态入口，并把 `AppState` 传给 dashboard。

事件路径：子组件按钮 → 父组件回调 → `AppState` → Runtime 重绘。

## AI Coding Agent

项目包含：

- `DESIGN.md`：UI/UX 设计指导原则，优先级最高。
- `AGENTS.md`：Codex 等 agent 的项目规范。
- `CLAUDE.md`：Claude Code 的入口说明，引用 AGENTS.md。

修改前建议先读这三个文件。
