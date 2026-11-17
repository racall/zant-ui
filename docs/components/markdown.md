# Markdown 预览 / 编辑

```tsx
<Markdown source="# 标题\n\n正文 **加粗**" />
```

流式预览（`text`、`busy` 为应用状态）：

```tsx
<Markdown source={text} streaming={busy} caret="block" />
```

| 属性 | 默认 / 说明 |
|---|---|
| `source` / 字符串子节点 | Markdown 源码，二者选一；`key` 为实例标识 |
| `streaming`、`parseIncomplete` | false、true；流式模式缓存块并修复未闭合语法 |
| `singleDollarMath`、`latexDelimiters` | true、false；后者启用 `\(…\)` / `\[…\]` |
| `fontSize` | 16 |
| `contentWidth` | 0：公式、图片、Mermaid 图跟随 Markdown 实际宽度（与正文一致）；正数为它们的最大宽度，但不超过实际宽度，窄窗口不溢出 |
| `caret` | `"none"`；可选 `"block"` / `"circle"` |
| `lineNumbers`、`codeMaxHeight` | true、400 |
| `codeCopy` / `codeDownload` | 均为 true |
| `tableCopy` / `tableDownload`、`mermaidCopy` / `mermaidDownload` | 均为 true |
| `onLink`、`baseDir` | 链接回调、相对图片的基础目录（默认空） |
| `selectable` | true；主题自动跟随当前 UI 主题 |
| `labels` | 按钮与提示文案，对象、各项可选：`copy`、`copyMarkdown`、`copyCsv`、`download`、`footnotes`、`image`、`loadingImage`、`openLink`、`copyLink`、`cancel`、`linkTitle`、`linkMessage`、`unsupportedDiagram`；未给的项跟随应用语言（框架内置英文 / 简体中文） |

```tsx
<Markdown source={text} labels={{ copy: t("doc.copy"), download: t("doc.download") }} />
```

支持 GFM、代码高亮、表格、Mermaid 和数学，详见 [Markdown 与数学](../markdown.md)。
`Markdown` 标签是预览，不提供 `mode`、`onModeChange` 或编辑回调。

Rust：`ui::markdown(id, ui::MarkdownOptions { .. })` 按实例管理缓存；手动缓存用
`markdown::streamdown(&mut cache, source, props)`。Rust 还可设置 `theme`、`policy`、`labels`（`ui::MarkdownLabels`，`Labels::default()` 是当前语言，
`Labels::for_locale("zh-CN")` 取指定语言）、`image_loader`、`block_renderers`。
编辑器用 `markdown::editor::editor(Props { value, on_change, mode, .. })`，支持 Edit / Split / Preview；
`on_mode` 是模式下标回调（`IndexAction`），分栏 / 预览需提供 `cache`。
