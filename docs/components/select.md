# 下拉选择

```tsx
import { useState, Select } from "zant";

export default function App() {
  const [plan, setPlan] = useState(0);
  return <Select options={["基础版", "专业版", "企业版"]}
    selected={plan} onChange={setPlan} width={240} />;
}
```

| 属性 | 默认值 / 说明 |
|---|---|
| `options` | `[]`，字符串数组 |
| `selected`、`onChange` | 默认索引 0；回调收到从 0 开始的索引 |
| `width`、`height` | 未指定宽度时使用 220，高度 32 |
| `disabled`、`tooltip`、`key` | `false`、无提示、无 key |

弹出层自动上翻或下翻，点击外部关闭。Tab 聚焦后，Enter / 空格切换展开状态，↑↓循环选择，Esc 关闭；没有搜索或输入匹配。
Rust：`ui::select(ui::SelectOptions { .. })`。
