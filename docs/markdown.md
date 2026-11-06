# Markdown 与数学公式

`zant.markdown` 是 [Vercel Streamdown](https://github.com/vercel/streamdown) 的原生 Zig 移植，
`zant.math` 是按 KaTeX 排版算法实现的 LaTeX 公式组件。不使用 WebView / JS。

## 处理管线（与 Streamdown 一一对应）

| 模块 | 对应 Streamdown | 作用 |
| --- | --- | --- |
| `markdown/remend/` | `packages/remend` | 流式输出时补全未闭合语法（`**粗`、`[链接](htt`、`` `code``、`$$`…） |
| `markdown/blocks.zig` | `lib/parse-blocks.tsx` | 切分顶层块；HTML/`$$` 合并规则；流式时复用稳定前缀块 |
| `markdown/cache.zig` | 块级 memo | 每块只在内容变化时重新解析 |
| `markdown/parser/` | remark-gfm + remark-math | CommonMark + GFM（表格、任务、删除线、自动链接、脚注、提示块）+ 数学 |
| `markdown/render/` | `lib/components.tsx` 等 | 代码块、表格、Mermaid、图片、链接安全、光标 |
| `markdown/editor.zig` | — | 编辑 / 分栏 / 预览编辑器，格式化工具栏与快捷键 |
| `math/` | KaTeX | TeX 解析 + KaTeX 字体参数与盒模型排版，STIX Two Math 绘制 |

remend 的移植用 Streamdown 原版测试集校验：`tests/fixtures/remend_cases.json`
（522 条，由 `packages/remend/__tests__` 抽取）全部通过。

## 用法

```zig
// 1) 无状态：每帧解析并渲染
const node = try zant.markdown.render(allocator, source);

// 2) Streamdown：块级缓存（cache 放在应用 State 里，单一数据源）
state.md_cache = zant.markdown.Cache.init(gpa);
zant.markdown.streamdown(&state.md_cache, source, .{
    .mode = .streaming,                       // AI 流式：先 remend 再解析
    .render = .{ .is_animating = true, .caret = .block, .labels = zant.markdown.Labels.zh },
});

// 3) 编辑器
zant.markdown.editor(.{
    .value = state.text(),
    .selection = state.selection,             // 受控选区
    .mode = state.mode,                       // .edit / .split / .preview
    .on_change = ui.textAction(state, State.onChange),
    .on_selection = ui.rangeAction(state, State.onSelection),
    .on_mode = ui.indexAction(state, State.onMode),
    .cache = &state.md_cache,
});
```

编辑器快捷键：⌘B 粗体、⌘I 斜体、⌘K 链接、⌘E 行内代码、⌘⇧X 删除线（Windows 上 ⌘ 换成 Ctrl）；
列表/引用中回车自动续写，空项回车结束列表；Tab / ⇧Tab 缩进列表。

## 渲染选项（Streamdown props）

- `theme`：`markdown.theme.light()` / `.dark()`，代码配色为 github-light / github-dark。
- `controls`：代码块复制/下载、表格复制 Markdown/CSV 与下载、Mermaid 复制/下载。
- `line_numbers`（默认开）、`code_max_height`（默认 400，超出滚动）。
- `is_animating` + `caret`（`.block` ▋ / `.circle` ●）：流式光标；未闭合代码块禁用复制。
- `on_link`：链接安全。设置后点击链接把 URL 交给应用，应用渲染
  `markdown.linkSafetyModal`；为 null 时直接用系统浏览器打开。
- `policy`：URL 白名单（`allowed_link_prefixes` 等），`javascript:` 等危险协议总是拦截。
- `labels`：界面文案，`Labels.zh` 为中文。

## 数学语法（与 Streamdown/remark-math 一致）

- 行内：`$$…$$`，以及单个 `$…$`（`ParseOptions.single_dollar_math`，默认开启）。单 `$` 采用
  pandoc 规则以避免把金额当公式：开头 `$` 后不能是空白；结束 `$` 前不能是空白、后面不能紧跟数字；
  只尝试第一个候选结束符（`$20 and $10` 是普通文本，`$x$`、`$U=[u_1\ \cdots\ u_m]$` 是公式）。
- 块级：独立成行的 `$$` 围栏；单行 `$$x$$` 属于段落中的行内公式。
- 不支持 `\[ \]` / `\( \)`（与 Streamdown 相同）。
- 解析失败时与 KaTeX `throwOnError:false` 一样，以 `error_color` 显示源码；
  未知命令单独标色。

已支持：分式（`\frac \dfrac \tfrac \cfrac \binom`）、根式（含 `\sqrt[n]`）、上下标与 `'`、
大型运算符与上下限（`\sum \prod \int \oint …`，`\limits \nolimits`）、函数名（`\sin \lim \max …`、
`\operatorname`）、`\left \middle \right` 与 `\big…\Bigg`、矩阵与环境（`matrix pmatrix bmatrix
Bmatrix vmatrix Vmatrix smallmatrix cases dcases rcases aligned align gathered split array`）、
`\hline` 与列竖线、重音（`\hat \bar \vec \dot \ddot \tilde \widehat \widetilde …`）、
`\overline \underline \overbrace \underbrace \overrightarrow …`、字体（`\mathbb \mathcal \mathfrak
\mathbf \boldsymbol \mathrm \mathit \mathsf \mathtt`）、`\text…`、颜色、间距、`\boxed \cancel \not`、
`\overset \underset \stackrel`、`\phantom`、`\tag`、`\displaystyle` 等。

## 框架能力（为本组件新增）

- `ui.memo(.{ .key, .hash }, ctx, build)`：hash 不变时整棵子树跳过构建与协调。
- `ui.vector(.{ .drawing })`：保留式矢量绘制（字形、矩形、线、路径），公式和图表使用。
- `RichSpan.embed`：富文本内嵌绘制（行内公式与正文基线对齐）；`RichSpan.link` + `richText.on_link`。
- `ui.column(.{ .scroll = true })`：纵向滚动容器。
- `InputOptions.selection / on_selection / on_key`：受控选区与按键钩子；`ButtonOptions.focusable`。
- 运行时主题切换（State 声明 `pub fn theme`）与应用快捷键（`pub fn onKey`）；`tokens.antDark()`。
- `dialogs.confirm / saveFileAs / openUrl / writeFile`。
- `ZANT_SNAPSHOT=out.png ZANT_SNAPSHOT_EXIT=1`：截图后退出，便于视觉回归。
- antd 风格组件：`ui.icon` / `ui.iconButton`（Zant 线性图标集）、`ui.dropdownMenu`、
  `ui.segmented`、`ui.dismissLayer`；`ui.*` 默认颜色跟随运行时主题（`ui.currentTheme()`）。
- 多行输入框 `content_max_width`：输入框全宽、文字列居中，滚动条贴右边缘。
- 块缓存按“公共前缀 + 公共后缀”匹配：在文档中间插入/删除一块只重新解析变化的块；
  每个缓存条目有稳定 `id`，预览块的 key 为 `"{key_prefix}-e{id}"`。
  `markdown.blockKeyAtOffset(&cache, key_prefix, offset, &buf)` 返回源码偏移所在块的 key，
  配合 `ui.scrollIntoView(key)` 实现大纲/查找跳转到预览位置。
- `markdown.html.toHtml(gpa, &doc, .{ .title, .standalone, .dark })`：导出 HTML（GFM 标记，
  独立页面内置排版样式并用 KaTeX auto-render 渲染 `\(..\)` / `\[..\]`；原始 HTML 块被转义）。
- 编辑器分栏模式且 `mode_switch = false` 时，格式工具栏位于编辑窗格上方。

示例：`zig build run-markdown`（`ZANT_DARK=1` 深色）、`zig build run-math`。
覆盖全部 Markdown 与公式渲染的示例文档：[`showcase.md`](showcase.md)（README 截图即 still. 打开它的效果）。
