# 按钮

```tsx
import { Button, useState } from "zant";

export default function App() {
  const [saved, setSaved] = useState(false);
  return <Button variant="primary" onClick={() => setSaved(true)}>
    {saved ? "已保存" : "保存"}
  </Button>;
}
```

| 属性 | 默认 / 说明 |
|---|---|
| `text` / 文字子节点 | 空字符串 |
| `variant` | `"default"`；常用 `"primary"` / `"text"` / `"link"`；另有 `"plain"` / `"caption"` / `"caption_close"`（标题栏用） |
| `size` | `"middle"`；`"small"` / `"middle"` / `"large"` 高 24 / 32 / 40 |
| `disabled`、`focusable` | false、true；关闭 focusable 后点击不抢焦点 |
| `danger` | false；危险按钮（antd）：primary 为红色填充，default 为红色边框和文字，text / link 为红色文字；悬停 / 按下用 `colorErrorHover` / `colorErrorActive`，禁用时与普通禁用相同 |
| `block` | false；宽度撑满父容器（antd `block`，即 `width: 100%`）；同时给了 `width` 时以 `width` 为准 |
| `loading` | false；加载中：标签前显示旋转的加载图标（有 `icon` 时替换它），整体 65% 不透明度，忽略点击与 Enter / Space |
| `width`、`fontSize`、`textColor` | 可选覆盖；`textAlign` 默认 `"center"`，`labelPadding` 默认 0 |
| `icon`、`iconSize` | `"none"`、18；见 [图标](icon.md) |
| `onClick` | 点击回调；或 `indexAction` + `index`（默认 0，优先于 onClick） |
| `key` | 节点标识 |

Tab 聚焦，Space / Enter 激活；移出按钮后释放不触发点击，禁用时忽略输入。
只有图标、没有文字的按钮是正方形（宽 = 高）。加载图标每秒转一圈，只重绘不重建；
`ZANT_NO_MOTION=1`、截图或系统“减少动态效果”时显示静止的图标。

```tsx
<Button variant="primary" danger onClick={remove}>删除</Button>
<Button variant="primary" loading={saving} block onClick={save}>保存</Button>
```

Rust：`ui::button(ui::ButtonOptions { .. })`，回调用 `ui::action` / `ui::index_action`。
