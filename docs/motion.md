# 动效

```tsx
import { useState, View, Button, Text } from "zant";

export default function App() {
  const [open, setOpen] = useState(false);
  return <View padding={24} gap={12}>
    <Button onClick={() => setOpen(prev => !prev)}>切换内容</Button>
    {open && <View key="panel" motion="slideUp" motionOrigin="top">
      <Text>内容出现和消失时过渡</Text>
    </View>}
  </View>;
}
```

## 进出场（presence）

| `motion` | 效果 | 进入 / 退出时长 |
|---|---|---|
| `fade` | 透明度 | 200ms |
| `zoomBig` | 0.8→1 缩放与透明度 | 200ms |
| `zoomBigFast` | 同上，用于 Tooltip | 100ms |
| `slideUp` / `slideDown` | 从上 / 下边缘纵向缩放与透明度 | 200ms |
| `moveUp` | 下移 8px 入场与透明度 | 200ms |

`View` / `Column` / `Row` / `Clickable` 都有 `motion`、`motionOrigin`（默认 `center`）与
`layoutTransition`；Rust 是 `BoxOptions` / `ViewOptions` 的同名字段，或 `ui::Style`。
`motionOrigin` 支持 topLeft/top/topRight、left/center/right、bottomLeft/bottom/bottomRight。
节点需有稳定 key；退场时继续绘制，但已退出布局、命中测试与焦点；同 key 再次出现会复活节点。
DropdownMenu、Select 弹层与 Modal 等组件已内置过渡。

## 布局过渡

```tsx
<View key="thumb" layoutTransition width={wide ? 240 : 120} />
```

`layoutTransition` 用默认的 200ms 过渡；自定义时长或 `anchor` 用 Rust：

```rust
use zant::{motion::motion, ui};

let node = ui::styled(ui::column(ui::BoxOptions {
    key: Some("thumb".into()),
    ..Default::default()
}), ui::Style {
    absolute: Some(true),
    anchor: Some(0),
    layout_transition: Some(motion::Transition {
        duration_ms: 300,
        ..Default::default()
    }),
    ..Default::default()
});
```

叶容器布局位置或尺寸改变时，从旧盒子过渡到新盒子；窗口缩放不触发此过渡。
Segmented 滑块与 Tabs 下划线使用此机制。TSX 没有 `anchor` 属性。

## 组件实现与检查

- 状态是唯一来源；组件 `animate` 用 `Animated::retarget` 计算呈现值，状态变化处 `tree.poke(id)`。
  Runtime 只推进登记的动画节点；动画帧只重绘，不重建，不用应用状态逐帧驱动。
- `Transition` 默认 200ms、`EaseInOut`、无延迟；缓动与 presence 实现在 `crates/zant/src/motion/`。
- 节点 id 会复用，应用不要跨帧保存 `NodeId`。
- `ZANT_NO_MOTION=1`、截图模式或系统“减少动态效果”会关闭动效。
- 测试用 `runtime.clock` 注入假时钟，不 sleep；见 `crates/zant/tests/motion_test.rs`。
  Release 下用 `ZANT_FRAME_STATS=1` 检查耗时，见 [开发与测试](development.md)。
