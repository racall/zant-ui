# 图标 `Icon` / `IconButton`

```tsx
<Icon name="search" size={16} />
<IconButton icon="search" tooltip="查找" />
```

| 属性 | 默认 / 说明 |
|---|---|
| `Icon.name` | 必填，Zant 图标名 |
| `Icon.size`、`color`、`stroke` | 18、主题文字色、随尺寸缩放（18px 时线宽 1.5px） |
| `IconButton.icon` | 必填，Zant 图标名 |
| `IconButton.size`、`box` | 图标 18、按钮边长 32 |
| `IconButton.color`、`activeBackground` | 默认主题色；`active` 默认 false |
| `IconButton.disabled`、`focusable` | false、false；工具栏按钮默认不抢输入框焦点 |
| `IconButton.tooltip`、`onClick` | 提示文字、点击回调；也可用优先的 `indexAction` + `index`（默认 0） |
| `key` | 两个组件均支持 |

24×24 网格的矢量线性图标；名字可用 `chevron_down` / `chevron-down` / `chevronDown`。
完整集合见 `crates/zant/src/core/icons.rs` 的 `Icon` 枚举（`None` 表示无图标）。
常用：`search`、`close`、`plus`、`check`、`folder`、`file_text`、`save`、`copy`、`trash`、`settings`、`more`。
Windows 标题栏字形见 [标题栏按钮](window-controls.md)。

Rust：`ui::icon(ui::IconOptions { .. })` / `ui::icon_button(ui::IconButtonOptions { .. })`，图标用 `ui::Icon` 枚举。
