# AGENTS.md

## ★ 最高优先级：设计规范 [DESIGN.md](DESIGN.md)

所有界面与交互的设计、实现和评审都**必须**遵循同目录的 [DESIGN.md](DESIGN.md)（UI/UX 设计指导原则）。
它的优先级高于本文件中的其他所有条目；与其他约定（包括 antd 风格规范）冲突时以 DESIGN.md 为准。
改动界面前先读它；完成后按其第 6 条检查不同窗口尺寸、浅/深色主题和各交互状态。

## 项目是什么

这是一个由 **Zant UI** 生成的原生桌面应用，运行在 macOS 与 Windows 上。

- 应用标题：`__APP_TITLE__`
- Bundle ID：`__BUNDLE_ID__`

- 语言：Zig 0.17.1
- UI：Zant UI 声明式 `ui.*` API
- 渲染：Skia + Metal（macOS）/ Direct3D 12（Windows）
- 平台：AppKit / NSWindow（macOS）、Win32（Windows），由框架封装
- 跨平台：快捷键用 `key.modifiers.primary()` 与 `zant.keys.label("mod+s")`（⌘S / Ctrl+S），
  自绘标题栏用 `ui.titlebarInsets()` 留出系统控件位置，数据目录用 `zant.paths.appDataDir`
- 配置：项目根目录 `app.config.json`
- 脚手架：编译后的 `zant` CLI，不使用 shell 脚本

## 常用命令

```bash
zant run              # 生成配置代码、编译并运行
zant run --watch      # 重启式热更新（macOS 沙盒应用不支持）
zant run --release
zant build
zant build --release
zant package           # macOS：生成并签名 .app；Windows：zig-out/<标题>/ 可分发目录
zig fmt src build.zig  # 格式化
zig build test         # 测试
```

不要手动编辑 `src/app_config.zig`、`Info.plist`、`app.entitlements`，它们由
`zant` 根据 `app.config.json` 生成（`zant sync` 只重新生成它们）。
Windows 上 Debug 构建是控制台程序，Release（`--release`、`zant package`）为 GUI 子系统。

## 目录结构

```text
src/main.zig                 界面入口、布局、窗口启动
src/state.zig                共享状态与动作
src/components/stat_card.zig  自定义展示组件（props）
src/components/counter_card.zig 自定义子组件（props + 事件回调）
src/components/dashboard.zig 自定义父组件（组合子组件）
src/app_config.zig           生成文件，不要手改
app.config.json              窗口、沙盒、签名配置
DESIGN.md                    UI/UX 设计规范（最高优先级）
AGENTS.md / CLAUDE.md        给 AI coding agent 的规范
```

## 状态与事件约定

- 状态由 `main.zig` 中的 `AppState` 持有，是单一数据源。
- 父组件通过 props 把只读值传给子组件。
- 子组件不直接修改状态，通过回调（事件）通知父组件。
- 事件传递路径：子组件按钮 → 父组件回调 → `AppState` 方法 → Runtime 重绘。
- 需要全局/异步状态时使用 `zant.store.Store(T)`，详见框架文档 `docs/state.md`。
- 回调类型必须是 `ui.action` / `ui.textAction` / `ui.boolAction` /
  `ui.indexAction` 包装后的类型；Zig 没有闭包。

## UI 规范

- 组件都是普通函数：`fn foo(props) ui.Node`，不要引入平台相关代码。
- 只组合 `ui.*` 基础组件，不要直接操作 NSWindow / HWND / Skia，也不要写 `builtin.os.tag` 分支。
- 布局使用 `ui.column` / `ui.row` / `ui.card`，避免手写绝对坐标。
- 文本使用 `ui.text`、`ui.heading`、`ui.paragraph`、`ui.textFmt`。
- 回调直接接收 `*AppState`，不要做 `*anyopaque` 转换。
- 新增组件放在 `src/components/`，并在父组件或 `main.zig` 中使用。
- 先用框架现成的通用组件（`ui.modal`、`ui.dropdownMenu`、`ui.segmented`、`ui.shortcutList`、
  `ui.iconButton`、`ui.dismissLayer`、`ui.pointerArea`、`zant.pdf.view`、`markdown.streamdown` …）；
  本应用的 `src/components/` 只放业务组件。可选择复制的文字用 `BoxOptions.selectable`。
  缺少通用组件时应补进 Zant UI 框架，而不是写在应用里。
- 保持组件无隐藏全局状态；需要共享时把状态提升到父级或 Store。

## 窗口配置

`app.config.json` 支持：

`title`、`bundle_id`、`version`、`sandbox`、`signing_identity`，
以及 `window` 下的 `style`、`width`、`height`、`min_width`、`min_height`、
`background`、`resizable`、`always_on_top`、`movable_by_background`、`center`。

代码中的 `WindowOptions` 优先级高于配置文件。修改 JSON 后重新执行
`zant run` / `zant build`。

## 沙盒与签名（仅 macOS）

- `sandbox: true` 时 `zant package` 写入 `com.apple.security.app-sandbox`。
- 签名使用 `signing_identity`；`none` 表示 ad-hoc。
- 沙盒应用运行必须通过 LaunchServices；`zant run` 已处理。
- Windows 上不签名、不沙盒，`zant package` 只生成可分发目录。
- 不要提交私钥、证书或密码。

## 代码质量

- 修改后运行 `zig fmt` 和 `zig build test`。
- 优先保持示例简短、可读，不要为了“架构”增加不必要的抽象。
- 注释解释“为什么”，不要重复代码表面含义。
- 不要新增 shell 脚本；脚手架功能放进 `zant` CLI。
