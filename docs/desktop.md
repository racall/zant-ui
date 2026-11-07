# 桌面集成 `zant.desktop`

常驻型应用（剪贴板工具、同步工具、启动器）需要的系统能力：多窗口与快捷面板、托盘 / 状态栏、
全局快捷键、剪贴板监听与图片、单实例、深色模式。macOS 与 Windows 行为一致，平台差异在框架内处理。
完整示例：ClipHop（局域网剪贴板同步）。

## 应用钩子（`zant.launch.run` 自动识别）

| 钩子 | 时机 |
|---|---|
| `onStart(self)` | 主窗口创建后、第一帧前：打开面板、设置托盘、注册快捷键、启动后台线程 |
| `onAppEvent(self, AppEvent) bool` | 托盘点击 / 菜单项、全局快捷键、程序坞点击、剪贴板变化；返回 true 重建所有窗口 |
| `onWake(self)` | 后台线程 `ui.postWake()` 之后，在 UI 线程取结果 |
| `onCloseRequest(self) bool` | 关闭主窗口；返回 false 保持打开（例如改为隐藏到托盘） |
| `onQuitRequest(self) bool` | ⌘Q / 程序坞“退出”；未定义时用 `onCloseRequest` |
| `theme(self)` | 跟随系统：`if (zant.desktop.systemDarkMode()) antDark() else antLight()`，系统切换外观时自动重建 |

`AppEvent`：`.tray_click`（Windows 左键托盘）、`.tray_menu = id`、`.hotkey = id`、`.reopen`
（macOS 点击程序坞 / 再次启动）、`.clipboard`。

## 多窗口与快捷面板

```zig
self.panel = try zant.desktop.openWindow(gpa, self, .{
    .render = State.renderPanel,
    .key = State.panelKey,     // 可选：Esc 隐藏等
    .focus = State.panelFocus, // 可选：失去焦点时隐藏
}, .{
    .title = "History",
    .window = .{ .style = .panel, .width = 380, .height = 480, .hidden = true },
});
self.panel.?.place(.tray_corner, 12); // macOS 右上角（菜单栏下）/ Windows 托盘旁（跟随任务栏位置）
self.panel.?.activate();
self.panel.?.focus("panel-search");
```

- 所有窗口渲染同一份应用状态；任何窗口里的动作改变了状态，所有窗口都会重建。每个窗口有独立的帧内存。
- `.panel` 窗口样式：无标题栏、系统圆角与阴影、置顶、不出现在程序坞 / 任务栏；macOS 上获得键盘焦点时
  不会把整个应用切到前台（用户复制后可以直接粘贴回原来的应用）。
- `show()` 只显示、不抢键盘焦点（提示、确认这类小窗口：用户可以继续在原来的应用里输入，第一次点击
  按钮就有效）；`activate()` 显示并获得键盘焦点（快捷面板）。
- `Window`：`show` / `hide` / `activate` / `isVisible` / `isActive` / `place` / `focus` / `close`；
  主窗口用 `zant.desktop.mainWindow()`。
- `WindowOptions.hidden = true` 创建时不显示。

## 托盘 / 状态栏

```zig
try zant.desktop.setTray(.{
    .icon = if (zant.keys.is_macos) @embedFile("icon_statusbar") else @embedFile("icon_tray_dark"),
    .icon_light = if (zant.keys.is_macos) null else @embedFile("icon_tray_light"),
    .template = zant.keys.is_macos,
    .tooltip = "My App",
    .items = &.{ .{ .id = 1, .title = "打开" }, .{ .separator = true }, .{ .id = 2, .title = "退出" } },
});
```

- 图标为 PNG，框架按系统尺寸缩放（macOS 18 pt 模板图；Windows 小图标尺寸，按 DPI）。
  Windows 托盘图标随任务栏深浅色自动切换 `icon` / `icon_light`。脚手架自带符合两端规范的默认图标。
- macOS 点击状态栏图标弹出菜单；Windows 左键发送 `.tray_click`，右键弹出菜单。
- 状态（例如“同步中”）：`zant.desktop.setTrayIcon(.{ .icon, .icon_light, .template, .tooltip })` 只换图标和
  提示文字，不重建菜单（菜单正打开时也不受影响），开销很小。状态图标和普通图标一样放进 `assets/icons/`
  （每个 PNG 都可以 `@embedFile("icon_<名字>")`，见 [scaffold.md](scaffold.md)「图标」）。
- 隐藏到托盘：`mainWindow().?.hide()` 加 `setDockVisible(false)`（macOS 隐藏程序坞图标）。
- `zant.desktop.quit()` 直接退出（不再询问 `onCloseRequest`）。

## 全局快捷键

```zig
_ = zant.desktop.registerHotkey(1, .{ .key = .{ .character = 'v' }, .modifiers = .{ .control = true, .meta = true } });
```

- 其他应用在前台时也有效（macOS Carbon `RegisterEventHotKey`，无需辅助功能权限；Windows `RegisterHotKey`）。
  被其他应用占用时返回 false。
- `zant.desktop.Shortcut`：`label`（"⌃⌘V" / "Ctrl+Alt+V"）、`format` / `parse`（存设置用的文本
  "ctrl+meta+v"）、`fromKeyEvent`。
- 录入控件 `ui.hotkeyInput`（见组件文档）；录入期间先 `unregisterHotkey`，已注册的组合键不会传到窗口。

## 剪贴板

| 接口 | 说明 |
|---|---|
| `watchClipboard(true)` | 每次剪贴板变化（包括本应用写入）发送 `.clipboard` 事件；Windows 用系统剪贴板监听，macOS 在 UI 线程每 0.3 秒检查变化计数 |
| `clipboardChangeCount()` | 变化计数；写入后记下它，可以忽略自己引起的那次变化 |
| `clipboardText` / `setClipboardText` | 纯文本 |
| `clipboardImage` / `setClipboardImage` | PNG：macOS 读 PNG / TIFF，写 PNG + TIFF；Windows 读 "PNG" 或位图（CF_DIBV5 / CF_DIB），写 "PNG" + CF_DIBV5 |

复制文字时很多应用也会附带一张渲染图，所以先读文字，没有文字再读图片。
`clipboardImage` 可以在后台线程调用，大图应当这样做：剪贴板里是位图时（Windows 截图和很多应用）要在这里
编码成 PNG，1200 万像素的照片约需 1 秒（ClipHop 的 `state.zig`「Capture」是参考写法）。

## 其他

- `singleInstance(bundle_id)`：在 `main` 开头调用；再次启动时把已运行的实例调到前台并返回 false。
- `deviceName(buf)`：用户看到的电脑名（macOS“Brian 的 MacBook Pro”，Windows 计算机名）。
- `systemDarkMode()`：系统深色模式。
- `invalidateAll()`：在动作之外改了状态时重建所有窗口。
- 图片：`zant.image.Image.thumbnail(png, max_px)`（缩小解码，线程安全，列表缩略图用）、`encodePng`。

## 平台说明

- Zig 0.17 的 `std.Io` 线程实现里，带超时的 TCP 连接在 macOS / Windows 上未实现（会 panic），
  Windows 上带超时的接收需要并发支持（返回 `ConcurrencyUnavailable`）。网络代码请用阻塞调用放在
  独立线程里（ClipHop 的 `sync.zig` 是参考写法）。
- Windows 首次监听网络端口时会弹出防火墙确认；macOS 向 255.255.255.255 广播会被拒绝
  （EADDRNOTAVAIL），请向各网卡的广播地址发送。
