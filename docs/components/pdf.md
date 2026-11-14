# PDF 阅读器

PDF 阅读器：连续滚动、缩放、文字选择与复制、搜索、链接、
页码跳转，以及批注（高亮 / 下划线 / 删除线 / 波浪线 / 画笔 / 便签）和带批注导出。
macOS 用 **PDFKit**，Windows 用 **PDFium**（`pdfium.dll`）；界面是原生 Skia 节点，
不嵌入 WebView / JS / WASM。交互与算法参照 [EmbedPDF](https://github.com/embedpdf/embed-pdf-viewer)
的各插件（viewport、scroll、zoom、render + tiling、selection、search、annotation、history）。

```tsx
// 自动管理文档（最简单）：路径或密码变化时重新打开，卸载时关闭文档、停止渲染线程
<PdfViewer src={path} toolbar annotationTools height={600} />
```

阅读器填满父容器：放进有尺寸的容器，或在 `Column` 里给 `grow`。

## 自动管理文档的 `PdfViewer`

`PdfViewer` 的属性是 `pdf::ViewOptions` 的全部字段（下同）加 `src` / `password` / `width` / `height` /
`grow`。实例由 `ui::local_state` 持有一个 `Viewer`，卸载自动关闭文档、停止渲染线程并释放状态。
路径或密码改变后在协调结束的 effect 中重新打开；相同输入保留阅读状态，失败路径不会反复打开。
空路径显示 `emptyText`，打开中显示 `loadingText`，失败沿用阅读器的主题化错误提示。
指定 height 时容器不伸长，其他情况下 grow 默认 1；支持 `ui::styled` 覆盖。
多个实例的内部 key 自动用 id 隔离，`key` 只设置最外层节点。不接受子节点。
需要程序控制 Viewer（如跳页、保存批注）时改由应用持有 `zant::pdf::Viewer` 并调用 `pdf::view`。

## 选项（`pdf::ViewOptions`）

| 选项 | 说明 |
|---|---|
| `key` | 节点 key 前缀（同一窗口多个阅读器要不同），默认 `"pdf"` |
| `toolbar` | 显示工具栏（翻页、页码输入、缩放、搜索），默认开 |
| `annotationTools` | 工具栏里的批注工具（选择 / 高亮 / 下划线 / 画笔 / 便签 + 撤销重做），默认关 |
| `selectionMenu` | 选中文字后浮出的菜单：复制、高亮、下划线、删除线，默认开 |
| `pageGap`、`padding` | 页间距、四周留白（px） |
| `background` | 视口背景，默认主题 `colorBgLayout` |
| `labels` | 界面文字，`pdf::Labels::ZH` 为中文 |
| `onLink` | 外部链接点击（收到 URL）；不设则用系统浏览器打开 |
| `onChange` | 页码、缩放、选区、搜索结果变化 |
| `onAnnotation` | 批注列表或选中的批注变化（保存列表、显示批注编辑器） |

## 交互

- **滚动**：滚轮 / 触控板；拖动滚动条；↑↓←→、Home / End、空格（Shift+空格向上）。
- **缩放**：工具栏 − / + 和百分比按钮（在“适合宽度 / 适合页面”间切换）；⌘/Ctrl + 滚轮、
  触控板捏合，以指针处为中心；⌘/Ctrl + `=` / `-` / `0`。窗口改变大小时保持适合模式。
- **选择文字**：拖动选择（可跨页，拖出视口自动滚动）；双击选词、三击选行；Shift + 点击扩展；
  ⌘/Ctrl + C 复制，⌘/Ctrl + A 全选；Esc 取消。
- **搜索**：工具栏输入框（⌘/Ctrl + F 聚焦），Enter / Shift+Enter 下一个 / 上一个；
  大小写不敏感、换行当作空格；逐帧增量搜索，大文档不卡界面。
- **链接**：文档内链接跳到目标页和位置；外部链接走 `onLink`。
- **批注**：见下文。

## 程序接口（`zant::pdf::Viewer`）

| 方法 | 说明 |
|---|---|
| `open` / `open_with_password` / `close` | 打开 / 关闭文档 |
| `page_count`、`current_page` | 页数、当前页（0 起） |
| `go_to_page(page, y)`、`next_page`、`previous_page` | 跳页（`y` 为页内点数偏移） |
| `set_zoom`、`zoom_in`、`zoom_out`、`set_fit(pdf::Fit::{Width,Page,None})` | 缩放 |
| `selection`、`set_selection`、`select_all`、`clear_selection` | 选区（字形下标） |
| `selected_text`、`copy_selection` | 选中文字 |
| `search(query)`、`search_results`、`next_match`、`previous_match` | 搜索 |
| `document()` | `zant::pdf::Document`：页面尺寸、`page_text`（字形几何）、`page_links`、`outline`（目录） |

## 批注

```rust
// 工具：Tool::Select / Highlight / Underline / Strikeout / Squiggly / Ink / Note
viewer.set_tool(pdf::Tool::Highlight);              // 拖过文字即高亮
viewer.annotate_selection(pdf::annotations::Kind::Underline, None); // 把当前选区变成批注
viewer.set_annotation_contents(id, "批注内容");
viewer.save_annotated("out.pdf")?;                  // 写入标准 PDF 批注的副本
```

- 选择工具下点击批注即选中（带描边），Delete / Backspace 删除，⌘/Ctrl + Z 撤销、
  ⇧⌘Z / Ctrl+Y 重做。
- `viewer.annotations().all()` 是批注列表（`page`、`kind`、`color`、`rects`、`strokes`、
  `contents`、`text`），应用自行持久化，下次用 `viewer.annotations_mut().load(&list)` 恢复。
- 选中的批注：`selected_annotation()`；它的位置 `ui::key_bounds(...)`，
  用来在旁边放评论编辑框（弹层放在根容器，参见下拉菜单文档）。
- `save_annotated` 写出 Highlight / Underline / StrikeOut / Squiggly / Ink / Text（便签）
  标准批注，Acrobat、Preview、浏览器都能显示和编辑；原文件不变。

## 实现

- 原生桥（`crates/zant-sys/native/platform/pdf.h`）：文档句柄、页面尺寸、分块渲染到内存像素、
  字形几何、链接、目录、带批注导出。所有调用线程安全。
- 渲染：后台线程按“每页一张低清底图 + 当前缩放下可见区域的清晰分块（1024 设备像素）”渲染，
  内存缓存 384 MB（LRU）；缩放过程中先显示最近的已渲染图，清晰块完成后替换。
- 文字几何与选区算法移植自 EmbedPDF（其本身移植自 PDFium `GetIndexAtPos` 与 Chromium
  PDF 选区合并），见 `crates/zant/src/pdf/text.rs`。macOS 上字形框用单字 `PDFSelection` 取得，
  与 `PDFPage.string` 下标一致（`characterBoundsAtIndex:` 在很多文档上会错位）。
- 扫描版 PDF 没有文本层时无法选择 / 搜索。

## 单页预览 `PdfPreview`

保留的简单组件：单页图片 + 翻页 + “复制本页文本”，选项 `key`、`src`、`page`、`width`、
`height`、`showToolbar`、`background`、`onPageChange`（手写 Rust：`ui::pdf_preview(ui::PdfOptions { .. })`）。

示例：`cargo run -p zant-examples --bin zant-pdf -- 文件.pdf`（`ZANT_PDF_DEMO=1` 添加示例批注，
⌘/Ctrl + S 导出带批注副本）。Windows 桌面交互测试：`scripts/win/pdf-check.ps1`。
