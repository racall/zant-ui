# 点击外部关闭 `DismissLayer`

```tsx
import { Button, DismissLayer, View, useState } from "zant";

export default function App() {
  const [open, setOpen] = useState(false);
  return <View>
    <Button onClick={() => setOpen(true)}>打开面板</Button>
    {open && <DismissLayer onDismiss={() => setOpen(false)} />}
    {open && <View absolute top={40} right={0} blockPointer padding={12}>
      <Button onClick={() => setOpen(false)}>关闭</Button>
    </View>}
  </View>;
}
```

铺满父容器（含 padding），放在弹层之前；需要覆盖整个窗口时放在根容器。
弹层设 `blockPointer`，防止空白处点击穿透（`DropdownMenu` 已内置）。

- `onDismiss`：点击回调；`background` 默认不绘制，设置后可作遮罩。
- `escape`：默认 `false`；为 `true` 时 Esc 也触发 `onDismiss`（只触发最上层的一个），用于弹出菜单、浮层（同 antd 下拉菜单按 Esc 收起）。
- `key` 默认 `"dismiss-layer"`，同屏多个实例应分别指定。

Rust：`ui::dismiss_layer(ui::DismissLayerOptions { .. })`。
