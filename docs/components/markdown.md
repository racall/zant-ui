# Markdown 编辑器 / 预览

`zant::markdown` 是 Vercel Streamdown 的原生 Rust 移植（remend → parse-blocks → 块级缓存 →
原生渲染），不用 WebView。数学公式由 `zant::math`（KaTeX 排版算法 + STIX Two Math）排版。
完整说明见 [Markdown 与数学](../markdown.md)；渲染效果见仓库 README 的 stillmd 截图和
[`docs/showcase.md`](../showcase.md)。

```tsx
// 只读预览（流式/静态）：只重新解析变化的块
<Markdown source={text} streaming={busy} />

// 编辑器：编辑 / 分栏 / 预览、格式工具栏、快捷键（mod+B/I/K/E、mod+Shift+X：macOS ⌘，Windows Ctrl）、列表续写
<Markdown
  key="doc"
  source={text}
  mode="split"
  onModeChange={setMode}
  contentWidth={720}
  codeCopy
  tableCopy
/>
```

预览的常用属性：`source`（或用字符串子节点 `<Markdown>{text}</Markdown>`）、`streaming`、
`caret`（`"none"` / `"block"` / `"circle"`）、`parseIncomplete`、`singleDollarMath`、`latexDelimiters`、
`fontSize`、`contentWidth`、`lineNumbers`、`codeMaxHeight`、`codeCopy` / `codeDownload`、
`tableCopy` / `tableDownload`、`mermaidCopy` / `mermaidDownload`、`onLink`、`baseDir`、`selectable`。
深色主题自动跟随 `useTheme()`（手写 Rust 用 `markdown::theme::dark()`）。

手写 Rust：`zant::markdown::streamdown(&mut cache, source, props)` 直接渲染，
或用 `ui::markdown(id, ui::MarkdownOptions { .. })` 让框架按实例持有缓存（TSX 的 `Markdown` 就是它；
实现位于 `zant::components::widgets`）。编辑器的其余 Rust 选项：
`on_mode`（`markdown::editor::Mode`）、`on_undo` / `on_redo`、`preview`（`markdown::ViewProps`）、
`placeholder`、`font_size`（16）、`line_height`（1.8）、`toolbar`、`mode_switch`、`shortcuts`、
`preview_width`、`labels`（`markdown::Labels::zh()`）。`html::to_html` 导出独立 HTML 页面（KaTeX 渲染公式）。

## 支持

- CommonMark + GFM：标题、段落、强调、删除线、高亮 `==…==`、上下标、行内代码、链接、
  自动链接、图片、列表（嵌套、任务列表）、引用（嵌套）、分割线、表格（对齐）、脚注、
  提示块 `> [!NOTE]` 等、HTML 块（安全转义）。
- 数学：`$…$`（pandoc 规则，金额不误判）、`$$…$$` 行内与块级；分式、根式、上下标、
  大型运算符、矩阵 / cases / aligned、`\left…\right`、字体命令、颜色等。
- 代码块语法高亮（行号、复制、下载），表格复制 Markdown / CSV、下载，Mermaid
  flowchart / sequence / pie（宽图自动缩放到内容列）。
- 外链安全确认（`markdown::link_safety_modal`）、流式光标、未闭合语法修复（remend）。
- 跳转：`markdown::heading_key` / `markdown::block_key_at_offset` + `ui::scroll_into_view`（大纲、查找）。
