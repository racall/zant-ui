# 虚拟列表 `List`

只构建、布局视口附近的条目：上万行的列表、带长消息的聊天，打开和滚动的开销都只有一屏。
设计参照 GPUI 的 `list` / `uniform_list`。

```tsx
<List
  key="chat"
  data={messages}
  renderItem={(message, i) => <MessageBubble key={message.id} message={message} />}
  keyExtractor={(message) => message.id}
  alignment="bottom"      // 内容少时贴底
  followTail              // 流式输出时跟随末尾
  gap={16}
  padding={{ horizontal: 24, vertical: 16 }}
  selectable
/>
```

列表默认填满父容器（`grow = 1`），也可以给 `width` / `height`。

## 选项

| 选项 | 说明 |
|---|---|
| `key` | 视口节点的 key |
| `data`、`renderItem` | 条目数组与构建函数 `(item, index) => 元素` |
| `keyExtractor` | 稳定 id（`(item) => string \| number`）：在中间插入 / 删除条目时用；只在末尾追加时不需要 |
| `alignment` | `"top"`（默认）/ `"bottom"`（短列表贴底，聊天） |
| `followTail` | 内容增长时保持末尾可见；用户向上滚动后停止跟随，滚回末尾自动恢复 |
| `overdraw` | 视口上下额外构建的像素（默认 600），滚动不露白 |
| `itemHeight` | 所有条目等高时给出（不再测量，最快） |
| `estimatedItemHeight` | 尚未测量的条目的估计高度（测过一些后改用它们的平均值） |
| `gap`、`padding`、`background` | 间距、内边距、背景 |
| `selectable` | 条目里的文字可拖选复制 |
| `focusable` | 点击后获得键盘焦点：↑↓、Page Up / Down、Home / End、空格 |
| `scrollbar` | 可拖动的滚动条（默认开） |
| `scrollToIndex` / `scrollAlign` | 声明式滚动：值变化时把第 i 条滚到 `"start"` / `"center"` / `"end"` |
| `layoutKey` | 行变了（新搜索、删除、上限变化）时换一个数字，重新测量并再滚到 `scrollToIndex` |

手写 Rust 用 `ui::list(&state.list, ui::ListOptions { count, item, … })`（`state.list` 是 `Rc<RefCell<ui::ListState>>`）：
`ListOptions` 的 `count` / `item`（`ui::ListItemBuilder`）/ `item_id` 对应上面的 `data` / `renderItem` /
`keyExtractor`，其余字段同名 snake_case（`estimated_height`、`follow_tail`、`item_height`、
`on_scroll` …）。

## `ListState`

| 方法 | 说明 |
|---|---|
| `ListState::new()` / `init()` | 创建 |
| `scroll_to_item(i, ui::ListScrollPosition::{Start,Center,End})` | 下一帧把第 i 条滚到视口开头 / 中间 / 末尾 |
| `scroll_to_end()` | 滚到末尾并恢复跟随 |
| `scroll_by(dy)`、`scroll_top()`、`is_at_end()` | 滚动与位置 |
| `rendered_range()` | 上一帧构建的下标范围 `[first, end)` |
| `splice(at, removed, inserted)` | 中间插入 / 删除了条目（高度记录随之移动） |
| `invalidate(i)` / `invalidate_all()` | 条目内容变了、需要重新测量（列表宽度变化时自动全部失效） |
| `follow(layout_key, index, position)` | `index` 和上次不同时滚动；`layout_key` 变了先 `invalidate_all()` 再滚一次 |

## 原理

- 条目在布局后测得高度并记住；未测量的用估计值，前缀和用 Fenwick 树（滚动条、跳转 O(log n)）。
- 滚动位置是逻辑的（第几条 + 条内偏移）：上方条目的估计被修正时，视口内容不跳动。
- 每次最多新建 3 个未测量的条目，从锚点向外扩展；同一帧最多三轮“布局 → 测量 → 补齐”，
  余下的在下一帧补上（GPUI 也是逐个测量）。
- 列表只接收滚轮（`PointerArea` 的 `clicks = false`）：点击照常到达条目里的按钮和文字。

## 限制

- 文字选择只覆盖已构建的条目（屏幕外的条目没有节点，与浏览器中的虚拟列表相同）。
- 屏幕外条目的状态（输入框内容、展开状态）要由应用保存在自己的数据里。

示例：`cargo run -p zant-examples --bin zant-components`（交互区的一万行列表）、
`zant-md-stream`（聊天）。
