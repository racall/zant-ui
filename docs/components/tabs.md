# 标签页

```tsx
import { useState, Tabs, Text } from "zant";

export default function App() {
  const [tab, setTab] = useState(0);
  return <Tabs key="settings" active={tab} onChange={setTab} items={[
    { label: "通用", content: <Text>通用设置</Text> },
    { label: "成员", content: <Text>成员列表</Text> },
  ]} />;
}
```

| 属性 | 默认值 / 说明 |
|---|---|
| `items` | `[]`；每项 `label`、`content` 必填 |
| `active`、`onChange` | 默认索引 0；回调收到从 0 开始的索引；越界索引显示最后一项 |
| `key` | 可选；同级多个标签页应分别设置 |

只有当前内容进入节点树。需跨页保留的状态放在父组件或 store（`createStore` / `useStore`）。
Rust：`ui::tabs(ui::TabsOptions { items: vec![ui::TabItem { .. }], .. })`。
