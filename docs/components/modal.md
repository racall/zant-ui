# 对话框 Modal

```tsx
import { useState, useKeyDown, View, Button, Text, Modal } from "zant";

export default function App() {
  const [open, setOpen] = useState(false);
  useKeyDown(e => {
    if (open && e.key === "Escape") { setOpen(false); return true; }
    return false;
  });
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
| `onClose` | 关闭按钮或可关闭遮罩触发；应用负责移除弹窗 |
| `closable`、`maskClosable` | 均为 `true` |
| `mask` | `0x73000000`，遮罩颜色 |
| `key` | `"modal"`；每个弹窗使用稳定且不同的 key |

作为窗口根容器的最后一个子节点渲染，覆盖父容器。内置遮罩、进出动效和 Tab 焦点范围；Esc 需应用处理。
Rust：`ui::modal(ui::ModalOptions { .. })`。
