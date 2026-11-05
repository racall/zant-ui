# Markdown 编辑器 / 预览

`zant.markdown` 是 Vercel Streamdown 的原生移植（remend → parse-blocks → 块级缓存 →
原生渲染），不用 WebView。数学公式由 `zant.math`（KaTeX 排版算法 + STIX Two Math）排版。
完整说明见 [Markdown 与数学](../markdown.md)；渲染效果见仓库 README 的 stillmd 截图和
[`docs/showcase.md`](../showcase.md)。

```zig
const md = zant.markdown;

// 只读预览（流式/静态）：只重新解析变化的块
md.streamdown(&state.md_cache, state.text(), .{ .render = .{ .theme = md.theme.light() } })

// 编辑器：编辑 / 分栏 / 预览、格式工具栏、快捷键（⌘B ⌘I ⌘K ⌘E ⇧⌘X）、列表续写
md.editor(.{
    .key = "doc",
    .value = state.text(),
    .selection = state.selection,
    .mode = state.mode,
    .cache = &state.md_cache,
    .on_change = ui.textAction(state, State.onChange),
    .on_selection = ui.rangeAction(state, State.onSelection),
    .content_max_width = 720,
})

// 导出 HTML（独立页面，KaTeX 渲染公式）
const html = try md.html.toHtml(gpa, &doc, .{ .title = "Note" });
```

## 支持

- CommonMark + GFM：标题、段落、强调、删除线、高亮 `==…==`、上下标、行内代码、链接、
  自动链接、图片、列表（嵌套、任务列表）、引用（嵌套）、分割线、表格（对齐）、脚注、
  提示块 `> [!NOTE]` 等、HTML 块（安全转义）。
- 数学：`$…$`（pandoc 规则，金额不误判）、`$$…$$` 行内与块级；分式、根式、上下标、
  大型运算符、矩阵 / cases / aligned、`\left…\right`、字体命令、颜色等。
- 代码块语法高亮（行号、复制、下载），表格复制 Markdown / CSV、下载，Mermaid
  flowchart / sequence / pie（宽图自动缩放到内容列）。
- 外链安全确认（`md.linkSafetyModal`）、流式光标、未闭合语法修复（remend）。
- 跳转：`md.headingKey` / `md.blockKeyAtOffset` + `ui.scrollIntoView`（大纲、查找）。
