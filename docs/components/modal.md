# 对话框 Modal

```tsx
import { useState, View, Button, Text, Modal } from "zant";

export default function App() {
  const [open, setOpen] = useState(false);
  return <View>
    <Button text="帮助" onClick={() => setOpen(true)} />
    {open && <Modal key="help" title="帮助" onClose={() => setOpen(false)}
      footer={<Button text="知道了" onClick={() => setOpen(false)} />}>
      <Text>按 Esc 或点击遮罩关闭。</Text>
    </Modal>}
  </View>;
}
```

| 属性 | 默认值 / 说明 |
|---|---|
| `content` / 唯一子元素 | 必填其一，内容节点 |
| `title`、`footer` | 空标题、无底栏；`footer` 可为 `null` |
| `width` | 520；TSX 传数字，Rust `None` 可按内容宽度 |
| `onClose` | 关闭按钮、可关闭的遮罩或 Esc 触发；应用负责移除弹窗 |
| `closable`、`maskClosable` | 均为 `true` |
| `keyboard` | `true`：Esc 关闭（同 antd）；多个弹窗叠在一起时只关最上层 |
| `mask` | `0x73000000`，遮罩颜色 |
| `key` | `"modal"`；每个弹窗使用稳定且不同的 key |

作为窗口根容器的最后一个子节点渲染，覆盖父容器。内置遮罩、进出动效、Tab 焦点范围和 Esc 关闭（下拉框展开或输入法正在组字时，Esc 先交给它们）。
打开时页面上的控件失去键盘焦点（内容里有 `autoFocus` 的输入框则聚焦它，否则按 Tab 从弹窗第一个控件开始）；
关闭后焦点回到打开前的控件。多个弹窗叠在一起时逐层恢复。
Rust：`ui::modal(ui::ModalOptions { .. })`。
