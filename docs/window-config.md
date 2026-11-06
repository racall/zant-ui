# 窗口配置

应用的窗口默认值来自项目根目录的 `app.config.json`。`zant run` / `zant build` /
`zant package` 会读取它，并生成 `src/app_config.zig`。

## app.config.json

```json
{
  "title": "My App",
  "bundle_id": "com.acme.myapp",
  "version": "0.1.0",
  "sandbox": true,
  "signing_identity": "Developer ID Application: Example (TEAMID)",
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
  }
}
```

| 字段 | 说明 |
|---|---|
| `title` | 窗口标题、应用显示名 |
| `bundle_id` | Apple Bundle ID |
| `version` | `CFBundleShortVersionString` |
| `sandbox` | 是否写入 App Sandbox entitlement 并用沙盒方式运行 |
| `signing_identity` | `codesign` 身份；`none` 表示 ad-hoc |
| `window.style` | `native` / `hidden-titlebar` / `borderless` |
| `window.width` / `height` | 初始窗口尺寸 |
| `window.min_width` / `min_height` | 最小尺寸 |
| `window.background` | `#RRGGBB` 或 `#AARRGGBB` |
| `window.resizable` | 是否可缩放 |
| `window.always_on_top` | 是否悬浮在其他窗口之上 |
| `window.movable_by_background` | 是否允许拖拽空白背景移动窗口 |
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
  `window.titlebar_height = 52`（与自绘标题栏同高）：
  - macOS：交通灯垂直居中其中，窗口缩放和退出全屏后自动保持；
  - Windows：框架在右上角自动绘制 Windows 11 风格的最小化 / 最大化 / 关闭按钮（高度填满标题栏），
    标题栏空白处可拖动、双击最大化，最大化按钮支持 Windows 11 贴靠布局；保留缩放边框和阴影。
  - 应用用 `ui.titlebarInsets()` 给系统控件留位置（`left`：macOS 交通灯；`right`：Windows 按钮）。
- `borderless`：无标题栏和交通灯，可拖拽背景移动，Cmd+Q / Alt+F4 退出。

## 文档类应用

- 关窗 / ⌘W / ⌘Q（Windows：关闭按钮 / Ctrl+W / Alt+F4）前确认未保存修改：状态类型声明
  `pub fn onCloseRequest(self: *State) bool`（返回 false 保持打开）。macOS 应用菜单自动包含
  “Quit <标题>”和“Close Window ⌘W”。
- 未保存标记：在 `render` 里调用 `ui.documentEdited(state.dirty)`，macOS 关闭按钮显示圆点，
  Windows 任务栏标题前加 `*`；标题可显示“文件名 — 已编辑”。
- 窗口底色跟随根节点背景（深色模式下缩放窗口不闪白）。

## 沙盒

`"sandbox": true` 时，打包会写入：

```xml
<key>com.apple.security.app-sandbox</key><true/>
```

并用 `--entitlements app.entitlements` 签名。沙盒应用必须通过
LaunchServices 启动，`zant run` 会自动使用 `open -W`。
