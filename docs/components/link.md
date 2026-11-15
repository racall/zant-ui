# 链接

```tsx
import { Link, Text, View, useState } from "zant";

export default function App() {
  const [opened, setOpened] = useState(false);
  return <View>
    <Link onClick={() => setOpened(true)}>查看详情</Link>
    {opened && <Text>详情内容</Text>}
  </View>;
}
```

`text` / 文字子节点默认空字符串；`onClick` 决定行为，不会自动打开浏览器；`key` 为节点标识。
Rust：`ui::link(ui::LinkOptions { .. })`，使用 link 样式的按钮。
