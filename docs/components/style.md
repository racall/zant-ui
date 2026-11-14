# 样式覆盖

所有组件都可以先用自身属性配置；需要统一覆盖时，TSX 用各容器 / 组件都支持的 `style` 对象
（与组件属性同名、camelCase），手写 Rust 用 `ui::styled`：

```tsx
<Button text="保存" variant="primary" style={{ background: 0xff111111, borderColor: 0xff111111, radius: 4, height: 44 }} />

<Column style={{ padding: { horizontal: 16, vertical: 12 }, gap: 8, background: theme.colorBgContainer }}>
  …
</Column>
```

```rust
ui::styled(
    ui::button(ui::ButtonOptions { text: "保存".into(), variant: ui::ButtonType::Primary, ..Default::default() }),
    ui::Style {
        background: Some(0xff111111),
        border_color: Some(0xff111111),
        radius: Some(4.0),
        height: Some(44.0),
        ..Default::default()
    },
)
```

## 可覆盖字段

- 外观：`background`、`borderColor`、`borderWidth`、`radius`（Rust 别名 `border_radius`）、
  `shadowColor`、`shadowBlur`、`shadowOffsetY`、`clip`、`blockPointer`。
- 布局：`padding`、`margin`、`width`、`height`、`widthPercent`、`heightPercent`、
  `minWidth`、`minHeight`、`maxWidth`、`maxHeight`、`grow`、`shrink`、`basis`、
  `order`、`alignSelf`、`absolute`、`top`、`right`、`bottom`、`left`、`anchor`。
- 文本：`textColor`（Rust 别名 `color`）、`fontSize`、`fontWeight`、`textAlign`、
  `lineHeight`、`wrap`。
- 动效：`motion`、`motionOrigin`、`layoutTransition`（见 [动效](../motion.md)）。

规则：

- `style` / `ui::styled` 返回新的节点，不修改原组件。
- 布局属性会覆盖组件的默认尺寸，因此可以统一按钮/卡片/输入框风格。
- 按钮背景和边框支持 `background` / `borderColor` 覆盖。
- 文本颜色/字号/字重会写入组件的文本样式。
- 更细的交互状态（hover、variant）仍由组件属性控制。

这样既保留每个组件的默认语义，又允许项目建立自己的设计系统。

## 组件覆盖矩阵

所有组件都可以套 `style` / `ui::styled`：

| 组件 | background | border | radius | 布局/文本 |
|---|---|---|---|---|
| Row/Column/View/Card/Space | ✅ | ✅ | ✅ | ✅ |
| Text/Heading/Paragraph | ✅ 背景 | ✅ | ✅ | ✅ |
| Button/Link | ✅ | ✅ | ✅ | ✅ |
| Input | ✅ | ✅ | ✅ | ✅ |
| Checkbox/Switch/Radio | ✅ 外层 | ✅ 外层 | ✅ 外层 | ✅ |
| Select | ✅ | ✅ | ✅ | ✅ |
| Image | ✅ 占位/背景 | ✅ | ✅ | ✅ |
| Tabs/Tag/Alert/Avatar | ✅ | ✅ | ✅ | ✅ |
| Progress | ✅ | ✅ | ✅ | ✅ |
| PdfPreview | ✅ 外框 | ✅ | ✅ | ✅ |

内部状态色（如 checkbox 勾选色、switch 轨道色）仍由主题和组件属性控制；
需要完全自定义时可以继续扩展该组件的 paint 逻辑。
