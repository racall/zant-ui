# Markdown 与数学公式

```tsx
import { View, Markdown } from "zant";

export default function App() {
  return <View padding={24} scroll>
    <Markdown source="# 你好\n\n**原生渲染**与公式 $E=mc^2$" />
  </View>;
}
```

`zant::markdown` 参考 Streamdown，`zant::math` 参考 KaTeX 排版算法；使用 Skia 原生绘制，无 WebView / JS。
效果示例见 [showcase.md](showcase.md)。

## 流式回复

```tsx
import { View, Markdown } from "zant";

interface Props { reply: string; busy: boolean; }
export function Reply({ reply, busy }: Props) {
  return <View scroll>
    <Markdown key="reply" source={reply} streaming={busy} caret="block" />
  </View>;
}
```

`Markdown` 按实例持有块缓存，更新时只重解析变化的块；稳定 key 可保留实例。
流式未闭合语法由 remend 修复，未完成的代码块禁用复制 / 下载。

| TSX 属性 | 默认 / 用途 |
|---|---|
| `source` | 文本；也可传字符串子节点 |
| `streaming` / `parseIncomplete` | false / true；流式模式与未闭合语法修复 |
| `caret` | none；另有 block / circle，流式时显示 |
| `fontSize` / `contentWidth` | 16 / 0；正文大小；公式、图片、图表的最大宽度（0 跟随实际宽度，正数也不超过实际宽度） |
| `lineNumbers` / `codeMaxHeight` | true / 400；代码限高，0 为不限高 |
| `codeCopy` / `codeDownload` | 均为 true；表格、Mermaid 有同名 table / mermaid 前缀选项 |
| `singleDollarMath` / `latexDelimiters` | true / false |
| `baseDir` | 相对图片路径的基准目录，默认空 |
| `selectable` | true；公式复制为 LaTeX，行号和按钮不进入正文复制 |
| `onLink` | 未设置时系统浏览器打开；设置后由应用处理 URL |

主题自动跟随当前 UI。`<Markdown>` 是预览组件，没有 `mode` / `onChange` 编辑器属性。

## 语法与限制

| 内容 | 示例 / 范围 |
|---|---|
| Markdown | 标题、列表、引用、代码、链接、图片、表格、任务、脚注、提示块 |
| 扩展行内格式 | 删除线 `~~x~~`、高亮 `==x==`、上下标 |
| 行内公式 | `$x$` 或 `$$x$$`；单美元按 pandoc 规则避免把 `$20 and $10` 当公式 |
| 块级公式 | 独立成行的 `$$` 围栏；单行 `$$x$$` 仍是行内公式 |
| 可选公式分隔符 | `latexDelimiters` 开启 `\(…\)` 与独立成行的 `\[…\]` |
| 数学 | 分式、根式、上下标、运算符、矩阵、cases/aligned、伸缩括号、字体、颜色等 |
| 化学式 / 单位 | `\ce{N2 + 3H2 <=> 2NH3}`、`\pu{123 kJ mol-1}` |
| Mermaid | flowchart/graph、sequenceDiagram、pie；不支持的图表回退代码 |

公式不是完整 TeX 引擎；排版失败会显示源码。远程图片默认显示占位卡片，应用可通过 Rust
`image_loader` 提供已解码图片；本地图片按 `baseDir` 解析，data 图片受安全策略控制。

## Rust 扩展与编辑器

```rust
use zant::{markdown, ui};

fn preview(cache: &mut markdown::Cache, source: &str) -> ui::Node {
    markdown::streamdown(cache, source, markdown::ViewProps {
        mode: markdown::CacheMode::Streaming,
        render: markdown::RenderOptions {
            is_animating: true,
            caret: markdown::Caret::Block,
            ..Default::default()
        },
        ..Default::default()
    })
}
```

缓存放在应用状态中复用；不要每帧创建。只需预览时也可用 `ui::markdown(id, ui::MarkdownOptions { .. })`。

| Rust API / 选项 | 用途 |
|---|---|
| `RenderOptions.image_loader` | 快速查图片缓存；后台加载解码后 `ui::post_wake()` |
| `RenderOptions.block_renderers` | 按围栏语言自定义块；`BlockInfo` 提供 lang/meta/code/complete/key，返回 None 回退代码 |
| `RenderOptions.policy` / `labels` | URL 安全策略 / 文案（默认跟随应用语言；`markdown::Labels::for_locale("zh-CN")`） |
| `markdown::link_safety_modal` | 应用接管链接后显示确认框；不会自动弹出 |
| `heading_key` / `block_key_at_offset` | 缓存中的标题 / 源码位置映射到 key，配合 `ui::scroll_into_view` |
| `markdown::html::to_html` | HTML 导出；独立页面公式使用 KaTeX auto-render，原始 HTML 块转义 |
| `markdown::editor(EditorProps { .. })` | Edit / Split / Preview；由应用持有 value、selection、mode、cache 并接收回调 |

`image_loader` 与自定义块回调的 `Rc` 应长期保存并按帧 clone；每帧新建会使块缓存失效。
编辑器支持 mod+B/I/K/E、mod+Shift+X、列表回车续写与 Tab 缩进（mod 为 ⌘ / Ctrl）。

## 实现与检查

管线：`remend/` → `blocks.rs` → `cache.rs` → `parser/` → `render/`；均位于 `crates/zant/src/markdown/`。
缓存按公共前后缀匹配，变化区间按位置继承块 id；memo 子树跳过未变化内容。数学位于 `math/`。
remend 夹具见 `tests/fixtures/remend_cases.json`；mhchem 夹具见 `crates/zant/src/math/mhchem_cases.json`。

运行示例用 `./scripts/cargo.sh run -p zant-examples --bin zant-markdown`；另有 `zant-math`、
`zant-md-stream`，`ZANT_DARK=1` 切深色。性能检查见 [开发与测试](development.md)。
