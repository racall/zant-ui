# 输入框

```zig
ui.input(.{
    .value = self.name(),
    .placeholder = "项目名称",
    .on_change = ui.textAction(self, State.onName),
})
```

## 选项

`key`、`value`、`placeholder`、`disabled`、`read_only`、`password`、`multiline`、
`focus_ring`、`width`、`height`、`font_size`、`line_height`、`on_change`、
`on_submit`（单行回车）、`on_undo` / `on_redo`、`selection`（受控 `[2]usize{ anchor, cursor }`，
UTF-8 字节）+ `on_selection`（`ui.rangeAction`）、`on_key`（`ui.inputKeyAction`，返回
`true` 吞掉按键，用于列表续写、自定义快捷键）、`content_max_width`（多行：文字居中成列，
滚动条仍贴右边）。

## 键盘

| 操作 | macOS | Windows |
|---|---|---|
| 全选 / 复制 / 剪切 / 粘贴 | ⌘A ⌘C ⌘X ⌘V | Ctrl+A/C/X/V |
| 撤销 / 重做 | ⌘Z / ⇧⌘Z | Ctrl+Z / Ctrl+Shift+Z、Ctrl+Y |
| 按词移动 | ⌥← / ⌥→ | Ctrl+← / Ctrl+→ |
| 行首 / 行尾 | ⌘← / ⌘→ | Home / End |
| 文档首 / 尾（多行） | ⌘↑ / ⌘↓ | Ctrl+Home / Ctrl+End |
| 删除一个词 | — | Ctrl+Backspace / Ctrl+Delete |

加 Shift 扩展选区。撤销/重做只在提供 `on_undo` / `on_redo` 时生效。

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
