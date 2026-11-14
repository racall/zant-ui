# 对话框 Modal

antd `Modal`：半透明遮罩（`rgba(0,0,0,.45)`）淡入，卡片以 zoom 动效弹出；标题、关闭按钮、
内容与可选的底部按钮。点击遮罩（`maskClosable`）或 ✕ 调用 `onClose`；Esc 由应用的按键
处理调用同一个关闭动作。打开时作为窗口根的最后一个子节点渲染（`Modal` 自己用 portal）。

```tsx
{helpOpen && (
  <Modal
    key="help"
    title="键盘快捷键"
    width={null}                     // null = 按内容；默认 520
    onClose={() => setHelpOpen(false)}
    content={<ShortcutList groups={shortcutGroups} />}
    footer={<Button text="知道了" variant="primary" onClick={() => setHelpOpen(false)} />}
  />
)}
```

选项：`key`、`title`、`content`、`footer`（null 无底栏）、`width`（默认 520，null 按内容）、
`onClose`、`closable`（右上角关闭按钮）、`maskClosable`、`mask`（遮罩 ARGB，默认 `0x73000000`）。
手写 Rust：`ui::modal(ui::ModalOptions { .. })`。
