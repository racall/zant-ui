# 窗口配置

应用的窗口默认值来自项目根目录的 `app.config.json`。`zant run` / `zant build` /
`zant package` / `zant sync` 会读取它，并生成 `src/app_config.zig`（macOS 上还有 `Info.plist`）。
通用设置由 `zant create` 写入；平台专属的段（目前只有 `macos`）在该平台首次运行 / 打包时补充。

## app.config.json

```json
{
  "title": "My App",
  "bundle_id": "com.acme.myapp",
  "version": "0.1.0",
  "window": {
    "style": "hidden-titlebar",
    "width": 560,
    "height": 420,
    "min_width": 480,
    "min_height": 320,
    "background": "#f5f5f5",
    "resizable": true,
    "always_on_top": false,
    "movable_by_background": false,
    "center": true
  },
  "macos": {
    "sandbox": true,
    "signing_identity": "Developer ID Application: Example (TEAMID)"
  }
}
```

| 字段 | 说明 |
|---|---|
| `title` | 窗口标题、应用显示名 |
| `bundle_id` | 应用标识（如 `com.xx.test`）：macOS 的 Bundle ID，Windows 上 `zant.paths.appDataDir` 的目录名；须符合苹果与微软规范（见 `docs/scaffold.md`），否则 `zant` 报错 |
| `version` | 版本号（macOS 写入 `CFBundleShortVersionString`） |
| `macos.sandbox` | 是否写入 App Sandbox entitlement 并用沙盒方式运行（仅 macOS） |
| `macos.signing_identity` | `codesign` 身份；`none` 表示 ad-hoc（仅 macOS） |
| `window.style` | `native` / `hidden-titlebar` / `borderless` |
| `window.width` / `height` | 初始窗口尺寸（逻辑像素，Windows 按显示器缩放） |
| `window.min_width` / `min_height` | 最小尺寸 |
| `window.background` | `#RRGGBB` 或 `#AARRGGBB` |
| `window.resizable` | 是否可缩放 |
| `window.always_on_top` | 是否悬浮在其他窗口之上 |
| `window.movable_by_background` | 是否允许拖拽空白背景移动窗口 |

省略的字段取默认值：`native`、560 × 420、最小 480 × 320、`#f5f5f5`、可缩放、居中、沙盒关、`none`。
旧项目写在顶层的 `sandbox` / `signing_identity` 仍然有效；补充 `macos` 段时会移到段里。
| `window.center` | 首次显示是否居中 |

## 代码优先

配置文件只生成默认值。`src/main.zig` 中传给 `zant.launch.run` 的
`WindowOptions` 是最终值，因此代码可以覆盖配置：

```zig
var window = cfg.window;
window.always_on_top = true;      // 代码覆盖 JSON
window.background = 0xff1f1f1f;   // 深色窗口背景

try zant.launch.run(init.gpa, State.init(), State.render, .{
    .title = cfg.title,
    .window = window,
    .theme = zant.tokens.antLight(),
});
```

修改 `app.config.json` 后直接执行 `zant run`，CLI 会重新生成
`src/app_config.zig` 并编译。

## 窗口风格

- `native`：原生标题栏（macOS 红黄绿按钮；Windows 系统标题栏，颜色随窗口底色深浅）。
- `hidden-titlebar`：内容延伸到标题栏区域。应用自绘标题栏时，在代码里设
  `window.titlebar_height = 52`（与自绘标题栏同高；未设时 macOS 用系统默认，Windows 按钮高 32）：
  - macOS：交通灯垂直居中其中，窗口缩放和退出全屏后自动保持；
  - Windows：框架在右上角自动绘制 Windows 11 风格的最小化 / 最大化 / 关闭按钮（高度填满标题栏），
    标题栏空白处可拖动、双击最大化，最大化按钮支持 Windows 11 贴靠布局；保留缩放边框和阴影。
  - 应用用 `ui.titlebarInsets()` 给系统控件留位置（`left`：macOS 交通灯；`right`：Windows 按钮）。
- `borderless`：无标题栏和系统按钮；配合 `movable_by_background` 拖拽背景移动，⌘Q / Alt+F4 退出。

## 文档类应用

- 关窗 / ⌘W / ⌘Q（Windows：关闭按钮 / Ctrl+W / Alt+F4）前确认未保存修改：状态类型声明
  `pub fn onCloseRequest(self: *State) bool`（返回 false 保持打开）。macOS 应用菜单自动包含
  “Quit <标题>”和“Close Window ⌘W”。
- 未保存标记：在 `render` 里调用 `ui.documentEdited(state.dirty)`，macOS 关闭按钮显示圆点，
  Windows 窗口标题（标题栏 / 任务栏）前加 `*`；标题可显示“文件名 — 已编辑”。
- 窗口底色跟随根节点背景（深色模式下缩放窗口不闪白）；Windows 的系统标题栏也随之切换深 / 浅色。

## 沙盒（仅 macOS）

`"sandbox": true` 时，打包会写入：

```xml
<key>com.apple.security.app-sandbox</key><true/>
```

并用 `--entitlements app.entitlements` 签名。沙盒应用必须通过
LaunchServices 启动，`zant run` 会先打包再用 `open -n -W` 启动（不支持 `--watch`）。
Windows 上该字段被忽略。
