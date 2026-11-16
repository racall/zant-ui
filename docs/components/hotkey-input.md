# 快捷键录入 `HotkeyInput`

```tsx
import { HotkeyInput, useState } from "zant";

export default function App() {
  const [value, setValue] = useState("");
  const [recording, setRecording] = useState(false);
  return <HotkeyInput value={value} recording={recording}
    onStart={() => setRecording(true)}
    onRecord={(shortcut) => { setValue(shortcut); setRecording(false); }}
    onCancel={() => setRecording(false)}
    onClear={() => { setValue(""); setRecording(false); }} />;
}
```

| 属性 | 默认 / 说明 |
|---|---|
| `value`、`recording` | 无快捷键（null 或空字符串）、false；录入状态由应用维护 |
| `width`、`height` | 按内容、32 |
| `placeholder`、`recordingText` | `"Click to set a shortcut"`、`"Press a shortcut…"` |
| `onStart`、`onRecord` | 点击或 Enter / Space 开始；记录回调收到快捷键字符串 |
| `onCancel`、`onClear` | 录入时无修饰键的 Esc 取消，Backspace / Delete 清除；清除按钮也触发 onClear |
| `key` | `"hotkey-input"`，同屏多个时分别指定 |

组合键需带 Ctrl / Alt / Meta（⌘ / Win），仅 Shift 不可。
若已注册全局快捷键，录入期间先注销，结束后重新注册，见 [桌面集成](../desktop.md)。
Rust：`ui::hotkey_input(ui::HotkeyInputOptions { .. })` + `ui::hotkey_action`，value 为 `Option<Shortcut>`。
