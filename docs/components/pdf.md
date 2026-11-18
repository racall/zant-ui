# PDF 阅读器

```tsx
import { PdfViewer } from "zant";

export default function App() {
  return <PdfViewer src="document.pdf" height={600} annotationTools
    emptyText="请选择 PDF" loadingText="正在打开…" errorText="无法打开 PDF" />;
}
```

## PdfViewer

| 属性 | 默认值 / 说明 |
|---|---|
| `src`、`password` | 路径必填，密码可选；路径或密码改变时重新打开 |
| `width`、`height`、`grow` | 宽高未设；`grow` 默认 1，指定 `height` 后默认 0 |
| `toolbar` | `true`，翻页、页码输入、缩放、搜索 |
| `annotationTools`、`selectionMenu` | `false`、`true`；批注工具栏、文字选区菜单 |
| `pageGap`、`padding` | 12、16 |
| `background` | 主题 `colorBgLayout` |
| `onLink` | 收到外部 URL；未设时用系统浏览器打开 |
| `onChange` | 页码、缩放、选区或搜索结果变化时调用，无参数 |
| `onAnnotation` | 批注列表或选中批注变化时调用，无参数 |
| `emptyText` | `"Choose a PDF to read"`，空路径时显示 |
| `loadingText` | `"Opening PDF…"` |
| `errorText` | `"This PDF can't be opened"` |
| `labels` | 工具栏、选区菜单与错误提示文案，对象、各项可选：`search`、`noResults`、`previousPage`、`nextPage`、`zoomIn`、`zoomOut`、`fitWidth`、`fitPage`、`previousMatch`、`nextMatch`、`copy`、`cannotOpen`、`password`、`highlight`、`underline`、`strikeout`、`pen`、`note`、`select` |
| `key` | 可选，设置外层节点；内部实例自动隔离 key |

不接受子节点。相同输入保留阅读状态，失败路径不会每帧重试；卸载会释放文档和渲染线程。表中的默认文案与 `labels` 未给的项跟随应用语言（框架内置英文 / 简体中文，见 [i18n](../i18n.md#框架内置文案)），
显式传入的优先。Rust：`pdf::Labels::default()` 是当前语言，`pdf::Labels::for_locale("zh-CN")` 取指定语言
（旧的 `pdf::Labels::ZH` 已弃用）。

## 交互

- 滚轮 / 触控板连续滚动；方向键、Home / End、空格 / Shift+空格滚动。
- 工具栏缩放、适合宽度 / 页面；⌘/Ctrl + 滚轮、捏合缩放；⌘/Ctrl + `+` / `-` / `0`。
- 拖动跨页选文字，双击选词、三击选行、Shift+点击扩展；⌘/Ctrl+C 复制、⌘/Ctrl+A 全选、Esc 取消。
- ⌘/Ctrl+F 聚焦搜索，Enter / Shift+Enter 下一个 / 上一个；内部链接跳转，外部链接触发 `onLink`。
- 批注工具栏提供选择、高亮、下划线、画笔、便签；选区菜单还可添加删除线，波浪线用 Rust `Tool::Squiggly` 设置。选中后 Delete / Backspace 删除，⌘/Ctrl+Z 撤销，Shift+⌘/Ctrl+Z 或 ⌘/Ctrl+Y 重做。

macOS 使用 PDFKit，Windows 使用 PDFium（需 `pdfium.dll`）。页面在后台分块渲染；扫描件没有文本层时无法选择或搜索。

## Rust 控制与导出

`ui::pdf_viewer_keyed(key, options)` 按字符串 key 管理阅读状态。列表中每个实例须使用不同且稳定的 key；未设置 `options.key` 时，根节点也使用该 key。现有调用可继续使用数字 ID 版本 `ui::pdf_viewer`。

需要跳页、保存批注时，由应用持有 `pdf::Viewer`，用 `pdf::view(&viewer, pdf::ViewOptions { .. })` 渲染；打开文件放在事件或 effect 中。

```rust
use zant::pdf;

let viewer = pdf::Viewer::new();
viewer.open("document.pdf")?;
viewer.go_to_page(2, 0.0); // 页码从 0 开始
viewer.set_tool(pdf::Tool::Highlight);
viewer.save_annotated("annotated.pdf")?;
```

`save_annotated` 输出带标准 PDF 批注的副本，不修改原文件。`Viewer` 另有 `set_zoom` / `set_fit`、`selected_text`、`search`、`annotations` / `annotations_mut` 等接口，见 `crates/zant/src/pdf/viewer/`。

## 单页预览 PdfPreview

```tsx
import { useState, PdfPreview } from "zant";

export default function App() {
  const [page, setPage] = useState(0);
  return <PdfPreview src="document.pdf" page={page} onPageChange={setPage} />;
}
```

`src` 必填；`page=0`、`width=320`、`height=420`、`showToolbar=true`（翻页、复制本页文本）；`background` / `key` 默认未设。应用通过 `onPageChange(index)` 更新页码。“复制文本”按钮与无法预览时的提示跟随应用语言。宽高指页面图像，工具栏另占空间。Rust：`ui::pdf_preview(ui::PdfOptions { .. })`。
