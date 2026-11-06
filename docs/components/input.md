# 输入框

```zig
ui.input(.{
    .value = self.name(),
    .placeholder = "项目名称",
    .on_change = ui.textAction(self, State.onName),
})
```

## 选项

`key`、`value`、`placeholder`、`disabled`、`read_only`、`password`、`width`、
`height`、`font_size`、`on_change`、`on_submit`。

## 密码输入

```zig
ui.input(.{
    .value = self.password,
    .password = true,
    .placeholder = "密码",
    .on_change = ui.textAction(self, State.onPassword),
})
```

密码模式显示为 `*`，但内部、回调和剪贴板仍使用真实文本；光标、选区和
输入法偏移按原始字节对齐。

`value != null` 时是受控输入；支持光标、选区、剪贴板和
输入法组合事件（macOS `NSTextInputClient`，Windows IMM32）。Windows 微软拼音的组字、候选窗定位
和上屏由 `scripts/win/smoke-stillmd.ps1` 端到端验证；macOS 拼音候选流程尚未自动化验证。
