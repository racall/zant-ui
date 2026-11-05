# 全局提示 Message

antd `message`：窗口顶部居中的小药丸，success / info / warning / err 四种类型，
move-up 动效进出；不拦截点击。显示时长由应用决定：在截止时间前渲染它，并用
`ui.wakeAfter` 在到期时重建。

```zig
const now = ui.nowMs();
if (now < state.toast_until_ms) {
    children.append(ui.message(.{
        .text = state.toastText(),
        .kind = .success,            // .info / .warning / .err
        .top = 60,
    }));
    ui.wakeAfter(@intCast(state.toast_until_ms - now));
}
```

设置提示：`state.toast_until_ms = zant.ui.nowMs() + 1800`（在动作回调里调用）。
