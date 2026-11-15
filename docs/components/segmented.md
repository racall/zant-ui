# 分段控制器 Segmented

```tsx
import { useState, Segmented } from "zant";

export default function App() {
  const [mode, setMode] = useState(0);
  return <Segmented key="mode" options={[
    { label: "编辑", icon: "pencil" }, { label: "预览", icon: "book_open" },
  ]} selected={mode} onChange={setMode} />;
}
```

| 属性 | 默认值 / 说明 |
|---|---|
| `options` | TSX 必填；每项 `label` 默认空、`icon` 默认 `"none"`、`tooltip` 可选 |
| `selected`、`onChange` | 默认索引 0；回调收到从 0 开始的索引 |
| `height`、`fontSize` | 28、13 |
| `itemWidth` | 默认随内容；可固定每项宽度 |
| `block` | `false`；设为 `true` 撑满父容器宽度 |
| `key` | `"segmented"`；多个实例使用不同 key |

选中滑块平滑移动。纯图标项建议提供 `tooltip`，核心视图切换优先用图标加文字。
Rust：`ui::segmented(ui::SegmentedOptions { .. })`，选项类型为 `ui::SegmentedOption`。
