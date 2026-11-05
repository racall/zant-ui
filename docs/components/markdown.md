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

- 数学公式目前会解析并保留公式内容，使用等宽/高亮样式显示，**尚未做 TeX 数学排版**。
- 富文本行内样式使用同一行的文本片段布局；复杂跨行富文本仍需要多 run 文本布局。
- 该实现参考 CommonMark/GFM 的块和行内结构，但不是逐条通过 CommonMark 官方
  spec 的完整实现。

## 建议

需要正式数学排版时，应接入 SwiftMath/KaTeX 等数学引擎，并把公式渲染结果缓存；
不要把公式源码伪装成已渲染结果。
