# 快捷键录入 `HotkeyInput`

设置页里录入全局快捷键的控件：显示当前组合键（每个键一个键帽），点击后按下新组合即完成。
录入状态由应用保存：

```tsx
const [hotkey, setHotkey] = useState("");     // "" 表示没有
const [recording, setRecording] = useState(false);
<HotkeyInput
  key="panel-hotkey"
  value={hotkey}
  recording={recording}
  width={180}
  onStart={() => { unregister(); setRecording(true); }}   // 录入期间先注销已注册的快捷键
  onRecord={(shortcut: string) => { setHotkey(shortcut); setRecording(false); }}
  onCancel={() => setRecording(false)}                    // Esc
  onClear={() => setHotkey("")}                           // ✕ 按钮、Backspace / Delete
/>
```

录入时：带 Ctrl / Alt（⌥）/ ⌘（Win）的组合键会被记录；Esc 取消；Backspace / Delete 清除。
`placeholder`、`recordingText` 可改文案；`width`（默认按内容）、`height`（默认 32）、`key`
（默认 `"hotkey-input"`，同屏多个时各给一个）。
手写 Rust：`ui::hotkey_input(ui::HotkeyInputOptions { .. })` + `ui::hotkey_action`。
全局快捷键的注册见 [桌面集成](../desktop.md)。
