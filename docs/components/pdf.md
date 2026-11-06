# PDF 预览

```zig
ui.pdfPreview(.{
    .src = "assets/manual.pdf",
    .page = self.pdf_page,
    .width = 360,
    .height = 460,
    .on_page_change = ui.indexAction(self, State.setPdfPage),
})
```

## 选项

`key`、`src`、`page`、`width`、`height`、`show_toolbar`、`background`、
`on_page_change`。

`page` 从 0 开始；`on_page_change` 接收新页码。

## 基本功能

- 首页 / 上一页 / 下一页 / 末页
- 页码显示
- **复制本页文本**：提取当前页文字并写入系统剪贴板
- 页面保持比例缩放（`contain`）
- 工具栏和外框可自定义

复制使用 PDF 的文本层（PDFKit / PDFium），因此文字型 PDF 可以复制；扫描版 PDF 如果没有
文本层，则没有可复制内容。

示例：`zig build run-pdf -- 文件.pdf`（`examples/pdf.zig`）。

## 实现

macOS 使用 **PDFKit**、Windows 使用 **PDFium**（`pdfium.dll` 与 `zant_skia.dll` 同目录，
首次使用时加载；缺失时显示“无法预览 PDF”）将页面渲染为 PNG，再交给 Skia 图片管线绘制。
页面图片按「文件 + 页码 + 尺寸」缓存在临时目录，不引入 WebView、JavaScript
或 WASM。

交互思路参考 [EmbedPDF](https://github.com/embedpdf/embed-pdf-viewer)，
但这里是原生子集。当前还不支持拖拽选中文本、搜索、缩略图侧栏、连续滚动和
缩放控件；需要这些能力时应优先扩展原生 PDF 桥（`src/platform/pdf.h`），而不是嵌入 Web 查看器。
