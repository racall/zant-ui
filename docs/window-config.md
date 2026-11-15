# 窗口配置

推荐用 TSX 编写应用，窗口默认值来自项目根目录的 `app.config.json`。
`native/build.rs` 的 `zant_build::tsx_app()` 在构建时读取配置，将 `crate::app_config`
生成到 `$OUT_DIR/zant_app.rs`；`native/main.rs` 只需 `zant::app!();`。
`zant run` / `zant compile` / `zant package` / `zant build` / `zant sync` 还会同步平台文件
（macOS 上包括 `Info.plist`），不要手改生成文件。
通用设置由 `zant create` 写入；模板包含 `macos.category` 默认值；其余平台专属设置在该平台首次运行 / 打包时补充。

## app.config.json

```json
{
  "title": "My App",
  "bundle_id": "com.acme.myapp",
  "version": "0.1.0",
  "description": "My App 桌面应用",
  "company": "acme",
  "copyright": "© 2026 acme",
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
    "center": true,
    "remember": true
  },
  "macos": {
    "category": "public.app-category.productivity",
    "minimum_system_version": "13.0",
    "sandbox": true,
    "signing_identity": "Developer ID Application: Example (TEAMID)"
  }
}
```

| 字段 | 说明 |
|---|---|
| `title` | 窗口标题、应用显示名 |
| `bundle_id` | 应用标识（如 `com.xx.test`）：macOS 的 Bundle ID，Windows 上 `zant::core::paths::app_data_dir` 的目录名；须符合苹果与微软规范（见 `docs/scaffold.md`），否则 `zant` 报错 |
| `version` | 版本号（macOS 写入 `CFBundleShortVersionString`） |
| `description` | 应用简介：Windows 写入 VERSIONINFO 的 `FileDescription`，显示在文件属性 → 详细信息中；留空时用 `title`。macOS 不写入 Info.plist |
| `company` | 公司 / 发布者：Windows 写入 VERSIONINFO 的 `CompanyName`，显示在文件属性 → 详细信息中；macOS 不写入 Info.plist |
| `copyright` | 版权声明：macOS 写入 `NSHumanReadableCopyright`，供标准“关于”面板显示；Windows 写入 VERSIONINFO 的 `LegalCopyright`，显示在文件属性 → 详细信息中 |
| `macos.category` | macOS 应用分类，写入 Info.plist 的 `LSApplicationCategoryType`（如效率类 `public.app-category.productivity`），用于系统分类和 App Store；Windows 忽略 |
| `macos.minimum_system_version` | macOS 最低系统版本，写入 Info.plist 的 `LSMinimumSystemVersion`，默认 `13.0`；Windows 忽略 |
| `single_instance` | 只运行一个实例：再次打开时把已运行的应用带到前面并退出（生成的入口自动传给 `zant::tsx::app::Config`；手写 Rust 应用可调 `zant::desktop::single_instance`）。默认 false |
| `macos.sandbox` | 是否写入 App Sandbox entitlement 并用沙盒方式运行（仅 macOS） |
| `macos.signing_identity` | `zant package` 和沙盒应用 `zant run --no-hot` 用的 `codesign` 身份；`none` 表示 ad-hoc（仅 macOS）。发布构建的证书在 `build.config.json`，见 [发布构建](build.md) |
| `macos.entitlements` | 应用需要的其他权限，如 `{"com.apple.security.network.client": true}`；值是 true / false、字符串或字符串列表。签名时（`zant package` / `zant build`）一起写入 |
| `macos.info_plist` | Info.plist 额外的字符串键，如 `{"NSLocalNetworkUsageDescription": "在局域网内发现其他设备"}`；不能覆盖生成器自己写的键 |
| `window.style` | `native` / `hidden-titlebar` / `borderless` / `panel`（快捷面板，见 [桌面集成](desktop.md)） |
| `window.width` / `height` | 初始窗口尺寸（逻辑像素，Windows 按显示器缩放） |
| `window.min_width` / `min_height` | 最小尺寸 |
| `window.background` | `#RRGGBB` 或 `#AARRGGBB` |
| `window.resizable` | 是否可缩放 |
| `window.always_on_top` | 是否悬浮在其他窗口之上 |
| `window.movable_by_background` | 是否允许拖拽空白背景移动窗口 |
| `window.center` | 首次显示是否居中；`remember` 有已保存的位置时优先恢复 |
| `window.remember` | 记住并恢复主窗口及 `createWindow` 副窗口的尺寸和位置。新项目模板为 true；旧配置省略时为 false。主窗口以 `bundle_id` 为键，副窗口以 `<bundle_id>.<createWindow 变量名>` 为键（手写 Rust 的 `open_window` 默认用窗口标题）。macOS 保存在用户默认设置，Windows 保存在 `HKCU\Software\Zant\<键>` 的 `WindowPlacement`；保存位置不在当前显示器上时 Windows 使用默认位置 |
| `window.titlebar_height` | `hidden-titlebar` 时应用自绘标题栏的高度（系统窗口按钮所在区域）；0 为系统默认。`macos.titlebar_height` / `windows.titlebar_height` 可按平台覆盖（比如 macOS 52、Windows 32） |
| `window.hidden_at_login` | 系统在登录时启动应用时先不显示窗口（托盘 / 菜单栏应用） |

`zant create` 默认将 `description` 设为“<应用标题> 桌面应用”，`company` 取反向域名应用标识的第二段并将首字母大写
（如 `com.acme.myapp` → `Acme`），`copyright` 为“© <创建时的 UTC 年份> <公司>”。
这些值写入配置后可自行修改，年份不会在后续构建时自动更新。

新项目模板为 `native`、680 × 700、最小 600 × 620、`remember: true`。
配置中最小尺寸大于初始尺寸时，会将最小尺寸收敛到初始尺寸。
配置解析器对省略的字段取默认值：`native`、560 × 420、最小 480 × 320、`#f5f5f5`、可缩放、居中、沙盒关、`none`。
旧项目写在顶层的 `sandbox` / `signing_identity` 仍然有效；补充 `macos` 段时会移到段里。

## 配置与代码

TSX 主窗口直接修改 JSON；副窗口用 `createWindow(Component, options)` 配置，字段用 camelCase
（如 `minWidth`、`titlebarHeight`），见 [桌面集成](desktop.md)。
手写 Rust 启动时，传给 `zant::tsx::app::run` 的 `Config.window` 或 `zant::ui::run` 的
`RunOptions.window` 才是最终值；`app_config::window` 是可复制的默认值。
窗口位置记忆由 `Config.remember`（手写 Runtime 为 `RunOptions.autosave_name`）控制，
不是 `WindowOptions` 的字段。

修改 `app.config.json` 后，正在运行的 `zant run` 会重新启动自身并重新编译；
没有运行时执行 `zant run` / `zant compile` / `zant package` 即可。
应用的 `src/` 只放 `.tsx` / `.ts`，Rust 放在 `native/`，无需创建 `src/app_config.rs`。

## 窗口风格

- `native`：原生标题栏（macOS 红黄绿按钮；Windows 系统标题栏，颜色随窗口底色深浅）。
- `hidden-titlebar`：内容延伸到标题栏区域。应用自绘标题栏时，在 JSON 里设
  `"window": { "titlebar_height": 52 }`（与自绘标题栏同高；未设时 macOS 用系统默认，Windows 按钮高 32）：
  - macOS：交通灯垂直居中其中，窗口缩放和退出全屏后自动保持；
  - Windows：框架在右上角自动绘制 Windows 11 风格的最小化 / 最大化 / 关闭按钮（高度填满标题栏），
    标题栏空白处可拖动、双击最大化，最大化按钮支持 Windows 11 贴靠布局；保留缩放边框和阴影。
  - 应用用 `titlebarInsets()`（手写 Rust `ui::titlebar_insets()`）给系统控件留位置（`left`：macOS 交通灯；`right`：Windows 按钮）。
- `borderless`：无标题栏和系统按钮；配合 `movable_by_background` 拖拽背景移动，⌘Q / Alt+F4 退出。

## 文档类应用

- 关窗 / ⌘W / ⌘Q（Windows：关闭按钮 / Ctrl+W / Alt+F4）前确认未保存修改：TSX 用
  `useCloseRequest(() => …)`（返回 false 保持打开），手写 Rust 实现 `App::on_close_request`。macOS 菜单栏由框架安装，具体菜单见 [桌面集成](desktop.md)。
  退出应用用 `useQuitRequest`；未注册时沿用关闭请求。
- 未保存标记：在 `render` 里调用 `ui::document_edited(dirty)`，macOS 关闭按钮显示圆点，
  Windows 窗口标题（标题栏 / 任务栏）前加 `*`；标题可显示“文件名 — 已编辑”。
- 窗口底色跟随根节点背景（深色模式下缩放窗口不闪白）；Windows 的系统标题栏也随之切换深 / 浅色。

## 沙盒（仅 macOS）

`"sandbox": true` 时，打包会写入：

```xml
<key>com.apple.security.app-sandbox</key><true/>
```

并用 `--entitlements app.entitlements` 签名。沙盒应用必须通过
LaunchServices 启动；用 `zant run --no-hot` 验证沙盒，CLI 会先打包再用 `open -n -W` 启动。
默认 `zant run` 使用开发宿主热刷新，不用于验证 App Sandbox；旧的 `--watch` 模式不支持沙盒。
Windows 上该字段被忽略。
