# 虚拟列表 `List`

```tsx
import { List, Text } from "zant";

export default function App() {
  return <List height={240} data={["第一行", "第二行", "第三行"]}
    itemHeight={32} renderItem={(text, i) => <Text>{text}</Text>} />;
}
```

| 属性 | 默认 / 说明 |
|---|---|
| `data`、`renderItem` | 必填：数组、`(item, index) => 元素` |
| `key`、`keyExtractor` | 视口标识、`(item) => string \| number`；中间插入 / 删除时提供稳定条目 id |
| `width`、`height`、`grow` | 自动、自动、1；需有确定视口高度 |
| `alignment`、`followTail` | `"top"`、false；bottom 让短列表贴底，followTail 跟随增长的末尾，向上滚动后暂停 |
| `overdraw`、`estimatedItemHeight`、`itemHeight` | 600、120、未指定；视口外预构建像素、未测量高度估值、固定行高（跳过测量） |
| `gap`、`padding`、`background` | 0、0、无 |
| `selectable`、`focusable`、`scrollbar` | false、true、true；文字选择、键盘滚动、滚动条 |
| `scrollToIndex`、`scrollAlign` | 未指定、`"start"`；下标变化时滚到该条，另可选 center / end |
| `layoutKey` | 数字；变化时重新测量并再次执行 scrollToIndex |

聊天示例（`messages` 是应用数组，每项有稳定 id 和正文）：

```tsx
<List data={messages} keyExtractor={(message) => message.id}
  renderItem={(message, i) => <Paragraph>{message.text}</Paragraph>}
  alignment="bottom" followTail selectable gap={12} />
```

只构建视口附近的条目；文字选择只覆盖已构建部分。输入值、展开状态等持久数据由应用保存。
固定行高必须与实际行高一致；不等高列表不要传 `itemHeight`。

Rust：`ui::list(&list_state, ui::ListOptions { count, item, .. })`，状态为 `Rc<RefCell<ui::ListState>>`；
`item` / `item_id` 对应 renderItem / keyExtractor，`estimated_height` 对应 estimatedItemHeight，另有 `on_scroll` 回调。
状态方法：`scroll_to_item(i, ui::ListScrollPosition::Start)`、`scroll_to_end()`、`scroll_by(dy)`、
`splice(at, removed, inserted)`、`invalidate(i)` / `invalidate_all()`、`rendered_range()`（`[first, end]`，end 不含）。
