# PDF 阅读器

`zant.pdf.view` 是完整的 PDF 阅读器：连续滚动、缩放、文字选择与复制、搜索、链接、
页码跳转，以及批注（高亮 / 下划线 / 删除线 / 波浪线 / 画笔 / 便签）和带批注导出。
macOS 用 **PDFKit**，Windows 用 **PDFium**（`pdfium.dll`）；界面是原生 Skia 节点，
不嵌入 WebView / JS / WASM。交互与算法参照 [EmbedPDF](https://github.com/embedpdf/embed-pdf-viewer)
的各插件（viewport、scroll、zoom、render + tiling、selection、search、annotation、history）。

```zig
const State = struct {
    viewer: *zant.pdf.Viewer, // 应用持有（像 markdown.Cache 一样）

    fn render(self: *State) ui.Node {
        return ui.column(.{ .grow = 1, .align_items = .stretch, .children = &.{
            zant.pdf.view(self.viewer, .{ .key = "pdf", .labels = .zh }),
        } });
    }
};

var viewer = zant.pdf.Viewer.init(gpa);
defer viewer.deinit();
try viewer.open("manual.pdf"); // error.PasswordRequired 时用 openWithPassword
```

阅读器填满父容器：放进有尺寸的容器，或在 column 里给 `grow`。

## 选项（`ViewOptions`）

| 选项 | 说明 |
|---|---|
| `key` | 节点 key 前缀（同一窗口多个阅读器要不同），默认 `"pdf"` |
| `toolbar` | 显示工具栏（翻页、页码输入、缩放、搜索），默认开 |
| `annotation_tools` | 工具栏里的批注工具（选择 / 高亮 / 下划线 / 画笔 / 便签 + 撤销重做），默认关 |
| `selection_menu` | 选中文字后浮出的菜单：复制、高亮、下划线、删除线，默认开 |
| `page_gap`、`padding` | 页间距、四周留白（px） |
| `background` | 视口背景，默认主题 `color_bg_layout` |
| `labels` | 界面文字，`Labels.zh` 为中文 |
| `on_link` | 外部链接点击（收到 URL）；不设则用系统浏览器打开 |
| `on_change` | 页码、缩放、选区、搜索结果变化 |
| `on_annotation` | 批注列表或选中的批注变化（保存列表、显示批注编辑器） |

## 交互

- **滚动**：滚轮 / 触控板；拖动滚动条；↑↓←→、Home / End、空格（Shift+空格向上）。
- **缩放**：工具栏 − / + 和百分比按钮（在“适合宽度 / 适合页面”间切换）；⌘/Ctrl + 滚轮、
  触控板捏合，以指针处为中心；⌘/Ctrl + `=` / `-` / `0`。窗口改变大小时保持适合模式。
- **选择文字**：拖动选择（可跨页，拖出视口自动滚动）；双击选词、三击选行；Shift + 点击扩展；
  ⌘/Ctrl + C 复制，⌘/Ctrl + A 全选；Esc 取消。
- **搜索**：工具栏输入框（⌘/Ctrl + F 聚焦），Enter / Shift+Enter 下一个 / 上一个；
  大小写不敏感、换行当作空格；逐帧增量搜索，大文档不卡界面。
- **链接**：文档内链接跳到目标页和位置；外部链接走 `on_link`。
- **批注**：见下文。

## 程序接口（`Viewer`）

| 方法 | 说明 |
|---|---|
| `open` / `openWithPassword` / `close` | 打开 / 关闭文档 |
| `pageCount`、`current_page` | 页数、当前页（0 起） |
| `goToPage(page, y)`、`nextPage`、`previousPage` | 跳页（`y` 为页内点数偏移） |
| `setZoom`、`zoomIn`、`zoomOut`、`setFit(.width / .page / .none)` | 缩放 |
| `selection`、`setSelection`、`selectAll`、`clearSelection` | 选区（字形下标） |
| `selectedText(allocator)`、`copySelection` | 选中文字 |
| `search(query)`、`searchResults`、`nextMatch`、`previousMatch` | 搜索 |
| `document()` | `zant.pdf.Document`：页面尺寸、`pageText`（字形几何）、`pageLinks`、`outline`（目录） |

## 批注

```zig
// 工具：.select / .highlight / .underline / .strikeout / .squiggly / .ink / .note
viewer.setTool(.highlight);                       // 拖过文字即高亮
_ = viewer.annotateSelection(.underline, null);   // 把当前选区变成批注
viewer.setAnnotationContents(id, "批注内容");
try viewer.saveAnnotated("out.pdf");              // 写入标准 PDF 批注的副本
```

- 选择工具下点击批注即选中（带描边），Delete / Backspace 删除，⌘/Ctrl + Z 撤销、
  ⇧⌘Z / Ctrl+Y 重做。
- `viewer.annotations.all()` 是批注列表（`page`、`kind`、`color`、`rects`、`strokes`、
  `contents`、`text`），应用自行持久化，下次用 `annotations.load(list)` 恢复。
- 选中的批注：`selectedAnnotation()`；它的位置 `ui.keyBounds(Viewer.annotationKey(&buf, "pdf", id))`，
  用来在旁边放评论编辑框（弹层放在根容器，参见下拉菜单文档）。
- `saveAnnotated` 写出 Highlight / Underline / StrikeOut / Squiggly / Ink / Text（便签）
  标准批注，Acrobat、Preview、浏览器都能显示和编辑；原文件不变。

## 实现

- 原生桥（`src/platform/pdf.h`）：文档句柄、页面尺寸、分块渲染到内存像素、字形几何、
  链接、目录、带批注导出。所有调用线程安全。
- 渲染：后台线程按“每页一张低清底图 + 当前缩放下可见区域的清晰分块（1024 设备像素）”渲染，
  内存缓存 384 MB（LRU）；缩放过程中先显示最近的已渲染图，清晰块完成后替换。
- 文字几何与选区算法移植自 EmbedPDF（其本身移植自 PDFium `GetIndexAtPos` 与 Chromium
  PDF 选区合并），见 `src/pdf/text.zig`。macOS 上字形框用单字 `PDFSelection` 取得，
  与 `PDFPage.string` 下标一致（`characterBoundsAtIndex:` 在很多文档上会错位）。
- 扫描版 PDF 没有文本层时无法选择 / 搜索。

## 单页预览 `ui.pdfPreview`

保留的简单组件：单页图片 + 翻页 + “复制本页文本”，选项 `key`、`src`、`page`、`width`、
`height`、`show_toolbar`、`background`、`on_page_change`。

示例：`zig build run-pdf -- 文件.pdf`（`examples/pdf.zig`；`ZANT_PDF_DEMO=1` 添加示例批注，
⌘/Ctrl + S 导出带批注副本）。Windows 桌面交互测试：`scripts/win/pdf-check.ps1`。
