# 单选组

```tsx
import { useState, RadioGroup } from "zant";

export default function App() {
  const [plan, setPlan] = useState(0);
  return <RadioGroup options={[
    { label: "基础版" }, { label: "专业版" }, { label: "企业版", disabled: true },
  ]} selected={plan} onChange={setPlan} />;
}
```

| 属性 | 默认值 / 说明 |
|---|---|
| `options` | `[]`；每项 `label` 必填，`disabled` 默认 `false` |
| `selected`、`onChange` | 默认索引 0；回调收到从 0 开始的索引 |
| `direction`、`gap` | `"column"`、8；横排用 `direction="row"` |
| `key` | 可选；同级多个单选组应分别设置 |

应用保存选中索引。Rust：`ui::radio_group(ui::RadioGroupOptions { .. })`，回调用 `ui::index_action`。
