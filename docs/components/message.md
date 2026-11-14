# 全局提示 Message

antd `message`：窗口顶部居中的小药丸，success / info / warning / error 四种类型，
move-up 动效进出；不拦截点击。显示时长由应用决定：在截止时间前渲染它，并用
`wakeAfter`（手写 Rust 用 `ui::wake_after`）在到期时重建。

```tsx
const now = performance.now();
{now < toastUntil && (
  <Message text={toastText} kind="success" top={60} />
)}
{now < toastUntil && wakeAfter(toastUntil - now)}
```

选项：`key`（默认 `"message"`）、`text`、`kind`（默认 `"success"`；`"info"` / `"warning"` /
`"error"`）、`top`（距窗口顶部，默认 16）、`iconColor`。手写 Rust：`ui::message(ui::MessageOptions { .. })`。

设置提示：在动作回调里 `setToastUntil(performance.now() + 1800)`（手写 Rust 用 `ui::now_ms()`）。
