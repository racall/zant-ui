# Markdown 与数学公式

`zant::markdown` 是 [Vercel Streamdown](https://github.com/vercel/streamdown) 的原生 Rust 移植，
`zant::math` 是按 KaTeX 排版算法实现的 LaTeX 公式组件。不使用 WebView / JS。

## 处理管线（与 Streamdown 一一对应）

| 模块 | 对应 Streamdown | 作用 |
| --- | --- | --- |
| `markdown/remend/` | `packages/remend` | 流式输出时补全未闭合语法（`**粗`、`[链接](htt`、`` `code``、`$$`…） |
| `markdown/blocks.rs` | `lib/parse-blocks.tsx` | 切分顶层块；HTML/`$$` 合并规则；流式时复用稳定前缀块 |
| `markdown/cache.rs` | 块级 memo | 每块只在内容变化时重新解析 |
| `markdown/parser/` | remark-gfm + remark-math | CommonMark + GFM（表格、任务、删除线、自动链接、脚注、提示块）+ 数学 |
| `markdown/render/` | `lib/components.tsx` 等 | 代码块、表格、Mermaid、图片、链接安全、光标 |
| `markdown/editor.rs` | — | 编辑 / 分栏 / 预览编辑器，格式化工具栏与快捷键 |
| `math/` | KaTeX | TeX 解析 + KaTeX 字体参数与盒模型排版，STIX Two Math 绘制 |

remend 的移植用 Streamdown 原版测试集校验：`tests/fixtures/remend_cases.json`
（522 条，由 `packages/remend/__tests__` 抽取）全部通过。与原版的差异：公式（`$…$`、`$$…$$`、
`\(…\)`、`\[…\]`）里的方括号（如区间 `[m,n]`）不当作未完成的链接补全，原版会在公式后面露出
`streamdown:incomplete-link`。

## 用法

TSX 用 `<Markdown>` 组件（见 [Markdown 组件](components/markdown.md)）：

```tsx
<Markdown source={reply} streaming={busy} />
```

手写 Rust 有三种用法：

```rust
use zant::markdown;

// 1) 无状态：每帧从 AST 渲染（文档已解析好时）
let node = markdown::render_document(&doc, markdown::RenderOptions::default());

// 2) Streamdown：块级缓存（cache 放在应用 State 里，单一数据源）
let mut cache = markdown::cache::Cache::new();
let node = markdown::streamdown(&mut cache, source, markdown::ViewProps {
    mode: markdown::cache::Mode::Streaming,   // AI 流式：先 remend 再解析
    render: markdown::RenderOptions {
        is_animating: true,
        caret: markdown::Caret::Block,
        ..Default::default()
    },
    ..Default::default()
});

// 3) 编辑器
let node = markdown::editor(markdown::EditorProps {
    value: text,
    selection: Some([anchor, cursor]),        // 受控选区
    mode: markdown::editor::Mode::Split,      // Edit / Split / Preview
    on_change: Some(ui::text_action(|s: &mut State, value| s.on_change(value))),
    on_selection: Some(ui::range_action(|s: &mut State, range| s.on_selection(range))),
    on_mode: Some(ui::index_action(|s: &mut State, index| s.on_mode(index))),
    cache: Some(&mut cache),
    ..Default::default()
});
```

编辑器快捷键：⌘B 粗体、⌘I 斜体、⌘K 链接、⌘E 行内代码、⌘⇧X 删除线（Windows 上 ⌘ 换成 Ctrl）；
列表/引用中回车自动续写，空项回车结束列表；Tab / ⇧Tab 缩进列表。

## 自动管理缓存的 `Markdown` 组件

```rust
// 手写 Rust：id 由父实例 id + key 派生（和 TSX 的 child_id 一致）
ui::markdown(ui::child_id(parent_id, "reply"), zant::components::widgets::MarkdownOptions {
    source: reply.into(),
    streaming: busy,
    caret: markdown::Caret::Block,
    font_size: 15.0,
    latex_delimiters: true,
    on_link: Some(ui::text_action(|s: &mut State, url| s.on_link(url))),
    ..Default::default()
})
```

`MarkdownOptions` 把常用排版 / 代码 / 表格 / Mermaid 控件和数学选项放在同一层；
主题默认跟随当前 UI，亦可传 `theme` 覆盖。`policy`、`labels`、`image_loader` 和 `block_renderers`
是 Rust 类型。内部通过 `ui::local_state` / `local_slot` 保存一个 `Cache`，每个稳定 id 代表一个实例，
卸载自动释放缓存。多个实例使用不同 id，内部节点 key 自动隔离；外观尺寸可用 `ui::styled` 覆盖。
TSX 对应 `<Markdown source={reply} streaming={busy} />` 或 `<Markdown>{reply}</Markdown>`。

## 渲染选项（Streamdown props）

- `theme`：`markdown::theme::light()` / `.dark()`，代码配色为 github-light / github-dark。
- `controls`：代码块复制/下载、表格复制 Markdown/CSV 与下载、Mermaid 复制/下载。
- `line_numbers`（默认开）、`code_max_height`（默认 400，超出滚动）。
- `is_animating` + `caret`（`Caret::Block` ▋ / `Caret::Circle` ●）：流式光标；未闭合代码块禁用复制。
- `on_link`：链接安全。设置后点击链接把 URL 交给应用，应用渲染
  `markdown::link_safety_modal`；为 `None` 时直接用系统浏览器打开。
- `policy`：URL 白名单（`allowed_link_prefixes` 等），`javascript:` 等危险协议总是拦截。
- `labels`：界面文案，`markdown::Labels::zh()` 为中文。
- `selectable`（默认开）：可拖动选择文字（跨段落、列表、表格、代码块），双击选词、三击选整段，
  ⌘C / Ctrl+C 复制，⌘A / Ctrl+A 全选；行内公式复制为 `$…$`，块级公式为 `$$…$$`，
  列表符号、代码行号、代码块标题栏和按钮不进入复制内容。
- `image_loader`：远程图片（`http(s)://…` 或应用自定义的 URL）。应用返回已解码的
  `zant::image::Image`，尚未就绪时返回 `None`（显示占位卡片），在后台线程下载解码后调用
  `ui::post_wake()`，下一帧该图片所在的块自动重建。`data:image/…;base64` 图片总是直接解码显示。
- `block_renderers`：按代码围栏语言渲染自定义界面（类似 Streamdown 的自定义 components），
  例如 ` ```chart {"id":3} ` 渲染成应用自己的图表 / 卡片。回调收到 `lang`、`meta`、`code`、
  `complete`（流式中为 false）、`key` 和上下文；返回 `None` 则按普通代码块显示。块是记忆化的，
  回调上下文要活过这一帧（`Rc`）。

```rust
let renderers = vec![markdown::BlockRenderer::new("chart", |info: &markdown::BlockInfo| {
    // info.lang / meta / code / complete / key
    None // None = 按普通代码块显示
})];
let loader: markdown::ImageLoader = std::rc::Rc::new(|url: &str| load_image(url));
let node = markdown::streamdown(&mut cache, source, markdown::ViewProps {
    render: markdown::RenderOptions {
        block_renderers: renderers,
        image_loader: Some(loader),
        ..Default::default()
    },
    parse: markdown::parser::Options { latex_delimiters: true, ..Default::default() },
    ..Default::default()
});
```

## 数学语法（与 Streamdown/remark-math 一致）

- 行内：`$$…$$`，以及单个 `$…$`（`parser::Options.single_dollar_math`，默认开启）。单 `$` 采用
  pandoc 规则以避免把金额当公式：开头 `$` 后不能是空白；结束 `$` 前不能是空白、后面不能紧跟数字；
  只尝试第一个候选结束符（`$20 and $10` 是普通文本，`$x$`、`$U=[u_1\ \cdots\ u_m]$` 是公式）。
- 块级：独立成行的 `$$` 围栏；单行 `$$x$$` 属于段落中的行内公式。
- `\( \)` / `\[ \]`：Streamdown 默认不支持（`\[` 在 Markdown 里是转义的方括号）；
  语言模型常用这种写法，可用 `parser::Options.latex_delimiters = true` 开启（行内 `\(…\)`，
  独立成行的 `\[ … \]` 为块级公式，可跨多行）。
- 解析失败时与 KaTeX `throwOnError:false` 一样，以 `error_color` 显示源码；
  未知命令单独标色。排版入口 `zant::math::layout::layout(&str, Options) -> Drawing`，
  用 `ui::vector` 显示。

已支持：分式（`\frac \dfrac \tfrac \cfrac \binom`）、根式（含 `\sqrt[n]`）、上下标与 `'`、
大型运算符与上下限（`\sum \prod \int \oint …`，`\limits \nolimits`）、函数名（`\sin \lim \max …`、
`\operatorname`）、`\left \middle \right` 与 `\big…\Bigg`、矩阵与环境（`matrix pmatrix bmatrix
Bmatrix vmatrix Vmatrix smallmatrix cases dcases rcases aligned align gathered split array`）、
`\hline` 与列竖线、重音（`\hat \bar \vec \dot \ddot \tilde \widehat \widetilde …`）、
`\overline \underline \overbrace \underbrace \overrightarrow …`、字体（`\mathbb \mathcal \mathfrak
\mathbf \boldsymbol \mathrm \mathit \mathsf \mathtt`）、`\text…`、颜色、间距、`\boxed \cancel \not`、
`\overset \underset \stackrel`、`\phantom`、`\tag`、`\displaystyle` 等；
带文字的箭头（`\xrightarrow[下]{上} \xleftarrow \xRightarrow \xleftrightarrow \xrightleftharpoons
\xhookrightarrow \xmapsto \xlongequal …`，按 KaTeX 的最小宽度与留白）、`\substack`、
`\displaylines`、`\colorbox \fcolorbox`、`\tiny … \Huge`、`\smash`、`\mathrlap \mathllap \mathclap`、
`\mathchoice`、`\rule`、`\raisebox`、`\genfrac`、`\href`（显示内容）、`\unicode \char`、`\Set`。

化学式与单位：`\ce{…}`、`\pu{…}`（`crates/zant/src/math/mhchem.rs`，KaTeX contrib/mhchem 的移植，
823 条用例与原版逐字节一致，全部可排版；入口 `zant::math::mhchem::to_tex`）。
例：`\ce{N2 + 3H2 <=>[催化剂][高温高压] 2NH3}`、`\ce{SO4^2- + Ba^2+ -> BaSO4 v}`、
`\ce{^{227}_{90}Th}`、`\pu{123 kJ mol-1}`。

## 流式渲染与性能

流式回复（SSE 逐段追加）每次更新只重新解析变化的块；变化区间里的块按位置继承旧块的身份
（UI key，同 Streamdown 按下标的 key），正在生长的段落 / 代码块 / 表格复用已有节点，未变化的
行和单元格保留排好的文字。聊天视图跟随最新内容用 `ui::scroll_to_end("chat")`（滚动容器的 key）。
还没输出完的公式：结束符只到了一半（`\]` 只到 `\`、`$$` 只到 `$`）时不显示这半个符号；
暂时无法排版的公式显示为一行灰色原文（换行当作空格，同浏览器里的 KaTeX）。
基准与参考数据见 `docs/development.md`「流式 Markdown 基准」（`zant-md-stream`）。

## 框架能力（为本组件新增）

- `ui::memo`：hash 不变时整棵子树跳过构建与协调。
- `ui::vector`：保留式矢量绘制（字形、矩形、线、路径），公式和图表使用。
- `RichSpan::embed`：富文本内嵌绘制（行内公式与正文基线对齐）；`RichSpan::link` + `rich_text.on_link`。
- `ui::column(.. .scroll = true ..)`：纵向滚动容器；`ui::scroll_to_end(key)` 滚到末尾（聊天跟随）。
- `BoxOptions.selectable`：容器内文字可选择复制；`unselectable` 排除行号、标题栏等；
  `RichSpan::copy` / `ui::vector(.. .copy ..)` 指定复制文本。
- `InputOptions.selection / on_selection / on_key`：受控选区与按键钩子；`ButtonOptions.focusable`。
- 运行时主题切换（`App::theme` / TSX `useTheme`）与应用快捷键（`App::on_key`）；`theme::tokens::ant_dark()`。
- `zant::platform::dialog` 的 `confirm` / `save_file_as` / `open_url` / `write_file`。
- `ZANT_SNAPSHOT=out.png ZANT_SNAPSHOT_EXIT=1`：截图后退出，便于视觉回归。
- antd 风格组件：`Icon` / `IconButton`（Zant 线性图标集）、`DropdownMenu`、
  `Segmented`、`DismissLayer`；`ui::*` 默认颜色跟随运行时主题（`ui::current_theme()`）。
- 多行输入框 `content_max_width`：输入框全宽、文字列居中，滚动条贴右边缘。
- 块缓存按“公共前缀 + 公共后缀”匹配：在文档中间插入/删除一块只重新解析变化的块；
  每个缓存条目有稳定 `id`，预览块的 key 为 `"{key_prefix}-e{id}"`。
  `markdown::block_key_at_offset(&cache, key_prefix, offset)` 返回源码偏移所在块的 key，
  配合 `ui::scroll_into_view(key)` 实现大纲/查找跳转到预览位置；`heading_key` 给出第 n 个标题的 key。
- `markdown::html::to_html(&doc, &Options { title, standalone, dark })`：导出 HTML（GFM 标记，
  独立页面内置排版样式并用 KaTeX auto-render 渲染 `\(..\)` / `\[..\]`；原始 HTML 块被转义）。
- 编辑器分栏模式且 `mode_switch = false` 时，格式工具栏位于编辑窗格上方。

示例：`cargo run -p zant-examples --bin zant-markdown`（`ZANT_DARK=1` 深色）、
`zant-math`、`zant-md-stream`。覆盖全部 Markdown 与公式渲染的示例文档：
[`showcase.md`](showcase.md)（README 截图即 still. 打开它的效果）。
