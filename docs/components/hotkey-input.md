# 快捷键录入 `ui.hotkeyInput`

设置页里录入全局快捷键的控件：显示当前组合键（每个键一个键帽），点击后按下新组合即完成。
录入状态由应用保存：

```zig
ui.hotkeyInput(.{
    .key = "panel-hotkey",
    .value = self.hotkey,                 // ?zant.desktop.Shortcut
    .recording = self.recording,
    .width = 180,
    .on_start = ui.action(self, State.startRecording),   // 录入期间先注销已注册的快捷键
    .on_record = ui.hotkeyAction(self, State.setHotkey),
    .on_cancel = ui.action(self, State.cancelRecording), // Esc
    .on_clear = ui.action(self, State.clearHotkey),      // ✕ 按钮、Backspace / Delete
})
```

录入时：带 Ctrl / Alt（⌥）/ ⌘（Win）的组合键会被记录；Esc 取消；Backspace / Delete 清除。
`placeholder`、`recording_text` 可改文案；`width`（默认按内容）、`height`（默认 32）、`key`（默认 `"hotkey-input"`，同屏多个时各给一个）。全局快捷键的注册见 [桌面集成](../desktop.md)。
