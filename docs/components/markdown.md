# Markdown 编辑器/预览

UI 库内置原生 Markdown 解析与预览，不依赖 HTML/WebView。

```zig
ui.input(.{
    .value = self.markdown,
    .multiline = true,
    .height = 430,
    .on_change = ui.textAction(self, State.onMarkdown),
})

// 只读预览
zant.markdown.render(allocator, self.markdown)
```

## 已实现

- 标题 `#` 到 `######`
- 段落、软换行
- 粗体、斜体、删除线
- 行内代码、代码围栏
- 链接、图片（提取显示文字）
- 无序列表、有序列表、任务列表
- 引用、分割线
- GFM 表格
- 行内/块级数学标记 `$...$`、`$$...$$`
- 转义字符

## 说明

- `$$...$$` 显示公式使用原生数学布局：分式、根号、上下标、希腊字母和常见运算符。
- 行内 `$...$` 目前保留公式源码并高亮；完整行内公式排版仍是后续工作。
- 富文本预览使用多 run Paragraph 布局，粗体/斜体/代码/链接可以混合并自动换行。
- 该实现参考 CommonMark/GFM 的块和行内结构，但不是逐条通过 CommonMark 官方
  spec 的完整实现。

## 建议

需要正式数学排版时，应接入 SwiftMath/KaTeX 等数学引擎，并把公式渲染结果缓存；
不要把公式源码伪装成已渲染结果。
