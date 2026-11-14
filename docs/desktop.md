# 桌面集成 `zant::desktop`

常驻型应用（剪贴板工具、同步工具、启动器）需要的系统能力：多窗口与快捷面板、托盘 / 状态栏、
全局快捷键、剪贴板监听与图片、单实例、深色模式。macOS 与 Windows 行为一致，平台差异在框架内处理。
完整示例：ClipHop（局域网剪贴板同步）。

TSX 应用直接用 [`docs/tsx.md`](tsx.md) 里的应用 hooks（`useTray` / `useHotkey` / `useClipboardChange` /
`useReopen` / `useWake` / `useAppExit` / `createWindow` 与从 `"zant"` 导入的 `clipboard` / `app` /
`mainWindow`）；本文件的手写 Rust 接口是它们的底层实现，写原生模块或手写 Rust 应用时使用。

## 应用钩子（`App` trait，`zant::ui::run` 调用）

| 钩子 | 时机 |
|---|---|
| `on_start(&mut self)` | 主窗口创建后、第一帧前：打开面板、设置托盘、注册快捷键、启动后台线程 |
| `on_app_event(&mut self, &AppEvent) -> bool` | 托盘点击 / 菜单项、全局快捷键、程序坞点击、剪贴板变化；返回 true 重建所有窗口 |
| `on_wake(&mut self)` | 后台线程 `ui::post_wake()` 之后，在 UI 线程取结果 |
| `on_exit(&mut self)` | 应用马上结束（退出、主窗口关闭）：停后台线程、保存；返回后进程退出 |
| `on_close_request(&mut self) -> bool` | 关闭主窗口；返回 false 保持打开（例如改为隐藏到托盘） |
| `on_quit_request(&mut self) -> bool` | ⌘Q / 程序坞“退出”；未实现时用 `on_close_request` |
| `theme(&self) -> Option<Theme>` | 跟随系统：`if zant::desktop::system_dark_mode() { ant_dark() } else { ant_light() }`，系统切换外观时自动重建 |

`AppEvent`（`zant::desktop::AppEvent`）：`TrayClick`（Windows 左键托盘）、`TrayMenu(id)`、
`Hotkey(id)`、`Reopen`（macOS 点击程序坞 / 再次启动）、`Clipboard`。

TSX 对应：`useCloseRequest`、`useQuitRequest`、`useKeyDown`、`useWindowFocus`、`useReopen`、
`useWake`、`useAppExit`、`useTray`、`useHotkey`、`useClipboardChange`。

## 多窗口与快捷面板

```rust
use zant::desktop;

fn on_start(&mut self) {
    self.panel = desktop::open_window(
        desktop::WindowHooks::new(|| render_panel()),
        desktop::OpenOptions {
            title: String::from("History"),
            window: desktop::WindowOptions {
                style: desktop::WindowStyle::Panel,
                width: 380.0,
                height: 480.0,
                hidden: true,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .ok();
    if let Some(panel) = &self.panel {
        panel.place(desktop::Place::TrayCorner, 12.0); // macOS 右上角（菜单栏下）/ Windows 托盘旁
        panel.activate();
        panel.focus("panel-search");
    }
}
```

- 所有窗口渲染同一份应用状态；任何窗口里的动作改变了状态，所有窗口都会重建。
- `.panel` 窗口样式：无标题栏、系统圆角与阴影、置顶、不出现在程序坞 / 任务栏；macOS 上获得键盘焦点时
  不会把整个应用切到前台（用户复制后可以直接粘贴回原来的应用）。
- `show()` 只显示、不抢键盘焦点（提示、确认这类小窗口：用户可以继续在原来的应用里输入，第一次点击
  按钮就有效）；`activate()` 显示并获得键盘焦点（快捷面板）。
- `Window`：`show` / `hide` / `activate` / `is_visible` / `is_active` / `is_minimized` / `place` /
  `focus` / `close`；主窗口用 `desktop::main_window()`。
- `WindowOptions`：`hidden = true` 创建时不显示；`hidden_at_login = true` 只在系统登录时自动打开应用时不显示
  （托盘应用在后台启动，见下文“开机启动”）。
- `is_minimized()`：最小化到程序坞 / 任务栏（仍算打开着，和隐藏不同）。
- TSX 里用 `createWindow(Component, options)` 声明窗口，返回带 `show/hide/toggle/activate/visible/active/
  minimized/place/focus` 的句柄。

## 托盘 / 状态栏

```rust
fn on_start(&mut self) {
    let _ = desktop::set_tray(desktop::TrayOptions {
        icon: include_bytes!("../assets/icons/tray-dark.png"),
        icon_light: if desktop::IS_MACOS { None } else { Some(include_bytes!("../assets/icons/tray-light.png")) },
        template: desktop::IS_MACOS,
        tooltip: "My App",
        items: &[
            desktop::TrayItem { id: 1, title: "打开", ..Default::default() },
            desktop::TrayItem { separator: true, ..Default::default() },
            desktop::TrayItem { id: 2, title: "退出", ..Default::default() },
        ],
    });
}
```

- 图标为 PNG，框架按系统尺寸缩放（macOS 18 pt 模板图；Windows 小图标尺寸，按 DPI）。
  Windows 托盘图标随任务栏深浅色自动切换 `icon` / `icon_light`。脚手架自带符合两端规范的默认图标。
- macOS 点击状态栏图标弹出菜单；Windows 左键发送 `AppEvent::TrayClick`，右键弹出菜单。
- 状态（例如“同步中”）：`desktop::set_tray_icon(desktop::TrayIconOptions { .. })` 只换图标和
  提示文字，不重建菜单（菜单正打开时也不受影响），开销很小。状态图标和普通图标一样放进 `assets/icons/`
  （Rust 用 `include_bytes!`，TSX 用 `import`，见 [scaffold.md](scaffold.md)「图标」）。
- 隐藏到托盘：`main_window().unwrap().hide()` 即可。调用过 `set_tray` 的应用和其他托盘 / 菜单栏应用一样：macOS 上
  只有普通窗口开着（或最小化）时才显示程序坞图标和菜单栏（系统标准做法：在 regular / accessory 两种
  activation policy 之间切换，并修正切回时菜单栏不更新的已知问题）；Windows 上隐藏的窗口本来就没有
  任务栏按钮。应用不需要自己处理。“是否在托盘里”用 `!w.is_visible() && !w.is_minimized()` 判断。
- `desktop::remove_tray()` 去掉托盘 / 状态栏图标。
- `desktop::quit()` 直接退出（不再询问 `on_close_request`）。

## 开机启动

```rust
let item = desktop::launch_at_login(cfg.bundle_id);          // LoginItem::Off / On / NeedsApproval
desktop::set_launch_at_login(cfg.bundle_id, true)?;          // LaunchAtLoginError::Unsupported / Failed
if desktop::launched_at_login() { /* 这次是登录时自动打开的 */ }
```

- 用系统自己的机制：macOS 13+ 的 `SMAppService`（出现在“系统设置 › 通用 › 登录项”，系统会提示
  “已添加登录项”；只有打包后的 .app 可以（`zant package` / `zant build`），`zant run` 运行时返回
  `Unsupported`）；Windows 是当前用户的 `Run` 注册表项（名字为应用标识，启动参数 `--zant-login`），
  `zant build` 生成的安装包卸载时会删除它。
- 状态每次从系统读取（用户也可以在系统设置 / 任务管理器里关掉）；默认关闭，由用户在设置里打开。
- `NeedsApproval`：macOS 需要用户在“登录项”里允许。
- 登录时打开：主窗口设 `.hidden_at_login = true`，应用只出现在托盘 / 菜单栏。macOS 通过启动时的
  Apple 事件（`keyAELaunchedAsLogInItem`）判断，事件循环开始后才知道，所以带 `.hidden_at_login` 的窗口
  在启动完成时才显示。
- TSX：`app.launchAtLogin()` / `app.setLaunchAtLogin(on)` / `app.launchedAtLogin()`。

## 全局快捷键

```rust
let _ = desktop::register_hotkey(
    1,
    desktop::Shortcut::parse("ctrl+meta+v").unwrap(),
);
```

- 其他应用在前台时也有效（macOS Carbon `RegisterEventHotKey`，无需辅助功能权限；Windows `RegisterHotKey`）。
  被其他应用占用时返回 false。
- `zant::desktop::Shortcut`：`label()`（"⌃⌘V" / "Ctrl+Alt+V"）、`parse()` / 格式化（存设置用的文本
  "ctrl+meta+v"）、`from_key_event`、`eql`。
- 录入控件 `HotkeyInput`（见组件文档）；录入期间先 `unregister_hotkey`，已注册的组合键不会传到窗口。
- TSX：`useHotkey("ctrl+cmd+v", cb)`、`shortcutLabel(str)`、`keyLabel("mod+f")`。

## 剪贴板

| 接口（`zant::desktop`） | 说明 |
|---|---|
| `watch_clipboard(true)` | 每次剪贴板变化（包括本应用写入）发送 `AppEvent::Clipboard`；Windows 用系统剪贴板监听，macOS 在 UI 线程每 0.3 秒检查变化计数 |
| `clipboard_change_count()` | 变化计数；写入后记下它，可以忽略自己引起的那次变化 |
| `clipboard_text` / `set_clipboard_text` | 纯文本 |
| `clipboard_image` / `set_clipboard_image` | PNG：macOS 读 PNG / TIFF，写 PNG + TIFF；Windows 读 "PNG" 或位图（CF_DIBV5 / CF_DIB），写 "PNG" + CF_DIBV5 |
| `clipboard_files` / `set_clipboard_files` | 在访达 / 资源管理器里复制的文件和文件夹：绝对路径，每行一个；写入后可以在访达 / 资源管理器里粘贴（Windows 为 CF_HDROP，粘贴是复制而不是移动） |

读取顺序：先文件（访达还会把文件名作为文字放进剪贴板），再文字（复制文字时很多应用也会附带一张渲染图），
最后图片。
`clipboard_image()` 可以在后台线程调用，大图应当这样做：剪贴板里是位图时（Windows 截图和很多应用）要在这里
编码成 PNG，1200 万像素的照片约需 1 秒（ClipHop 的 `state` 是参考写法）。
TSX：`clipboard.text()/setText/image/setImage/files/setFiles/changeCount()`。

## 其他

- `single_instance(bundle_id)`：在 `main` 开头调用；再次启动时把已运行的实例调到前台并返回 false
  （TSX 应用由 `app.config.json` 的 `single_instance` 自动处理）。
- `device_name()`：用户看到的电脑名（macOS“Brian 的 MacBook Pro”，Windows 计算机名）。
- `downloads_dir()`：用户的“下载”文件夹（Windows 用系统记录的位置，用户移动过也正确）。
- `system_dark_mode()`：系统深色模式。
- `invalidate_all()`：在动作之外改了状态时重建所有窗口。
- 图片：`zant::image::Image::thumbnail(png, max_px)`（缩小解码，线程安全，列表缩略图用）、
  `Image::encode_png`。
- 对话框：`zant::platform::dialog::{open_file, open_file_as, save_file, save_file_as, confirm, open_url}`。
- 快捷键文字：`zant::core::keys::{label, pick, primary}`（`keys::label("mod+shift+s")` → ⇧⌘S / Ctrl+Shift+S；
  平台习惯不同用 `keys::pick(mac, win)`）。

## 平台说明

- 网络代码用阻塞调用放在独立线程里（`std::net` + `std::thread`），完成后用 `ui::post_wake()` 唤醒 UI；
  不要在 UI 线程里做网络 I/O。ClipHop 的 `sync` 模块是参考写法。
- Windows 首次监听网络端口时会弹出防火墙确认；macOS 向 255.255.255.255 广播会被拒绝
  （EADDRNOTAVAIL），请向各网卡的广播地址发送。
- macOS 的登录项 / 全局快捷键等能力只对打包后的 .app 完整可用；`zant run` 下部分接口返回
  `Unsupported`。
