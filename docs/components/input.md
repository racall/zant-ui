# 输入框

```tsx
import { Input, useState } from "zant";

export default function App() {
  const [name, setName] = useState("");
  return <Input value={name} placeholder="项目名称" onChange={setName} />;
}
```

| 属性 | 默认 / 说明 |
|---|---|
| `value`、`onChange(value: string)` | 不传 value 为非受控；传值后由应用在回调中更新 |
| `placeholder` | 空字符串 |
| `disabled`、`readOnly`、`password`、`multiline` | 均为 false |
| `focusRing` | true，聚焦时显示激活边框 |
| `autoFocus` | false；true 时出现即获得键盘焦点（弹窗里的输入框、搜索框），同 React 的 autoFocus |
| `width`、`height` | 自动、32；多行需要给足高度 |
| `fontSize`、`lineHeight` | 14、0（字体自身行高） |
| `contentMaxWidth` | 可选：多行文字居中成列，滚动条仍在右侧 |
| `onSubmit` | 单行回车 |
| `onUndo`、`onRedo` | 撤销 / 重做回调，需应用提供历史记录 |
| `key` | 节点标识 |

```tsx
<Input multiline height={240} placeholder="正文" />
<Input password placeholder="密码" />
```

支持光标、选区、剪贴板和输入法。密码显示为 `*`；回调和剪贴板仍使用真实文字。
macOS 用 ⌘，Windows 用 Ctrl 执行全选、复制、剪切、粘贴、撤销；重做为 ⇧⌘Z / Ctrl+Shift+Z（Windows 另支持 Ctrl+Y）。

Rust：`ui::input(ui::InputOptions { .. })`；另支持 `selection: Option<[usize; 2]>`（anchor / cursor，UTF-8 字节）、
`on_selection`（`ui::range_action`）和 `on_key`（`ui::input_key_action`，返回 true 吞键），这些属性未暴露给 TSX。
输入事件排查可设 `ZANT_INPUT_TRACE=1`（见 [开发与测试](../development.md)）。
