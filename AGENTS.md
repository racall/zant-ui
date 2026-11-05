# AGENTS.md

## ★ 最高优先级：设计规范 [DESIGN.md](DESIGN.md)

所有界面与交互的设计、实现和评审都**必须**遵循同目录的 [DESIGN.md](DESIGN.md)（UI/UX 设计指导原则）。
它的优先级高于本文件中的其他所有条目；与其他约定（包括 antd 风格规范）冲突时以 DESIGN.md 为准。
改动界面前先读它；完成后按其第 6 条检查不同窗口尺寸、浅/深色主题和各交互状态。

## 项目介绍

**Zant UI** 是一个用 Zig 编写的原生 macOS 桌面 UI 框架。

- 语言：Zig 0.17.1
- 窗口：AppKit / NSWindow（原生，不使用 WebView）
- 渲染：Skia Ganesh + Metal；CPU 光栅回退用于测试/特殊环境
- 布局：Zig 实现的单行 Flex
- 文本：Skia Paragraph，支持中英文、换行、字体回退
- 状态：父组件持有状态 + 回调；可选 Zustand 风格 `zant.store.Store(T)`
- 脚手架：编译后的 `zant` CLI，不使用 shell 脚手架
- PDF：原生 PDFKit 预览组件
- 平台：macOS 已完成；Windows 为二期，当前未实现

这个仓库既包含框架，也包含生成项目的模板和 CLI。

## 常用命令

```bash
./scripts/fetch_skia.sh       # 首次准备 Skia 依赖
zig build                     # 构建框架、示例和 zant CLI
zig build test                # 全部测试
zig build run                 # 声明式示例
zig build run-gallery         # 命令式组件画廊
zig build -Doptimize=ReleaseFast
```

全局 CLI（已安装时）：

```bash
zant create my-app
cd my-app
zant run
zant run --watch              # 重启式热更新，非沙盒
zant build --release
zant package                  # ReleaseFast + .app + codesign
zant update                   # 重新编译框架并更新全局二进制
```

## 目录结构

```text
src/
  ui.zig                声明式 ui.* builders、ui.styled、PDF 组件
  ui_runtime.zig        协调器、事件、焦点、IME、帧调度
  launch.zig            最小启动入口
  store.zig             Zustand 风格 Store
  core/                 节点、树、布局、几何、事件、Unicode
  components/           文本、按钮、输入、Toggle、Select、图片
  renderer/skia/        Skia C ABI 与 Metal/CPU 渲染
  platform/macos/       AppKit 窗口、PDFKit 桥
  platform/windows/     二期说明，未实现
  motion/               动效：缓动曲线（antd）、Animated、presence 预设
  markdown/             Streamdown 移植：remend/ parser/ render/ blocks cache editor html
  math/                 LaTeX 公式（KaTeX 排版算法，STIX Two Math）
tools/zant/main.zig     编译后的脚手架 CLI
scaffold/template/      生成项目模板（含组件、状态、AGENTS/CLAUDE）
tests/                  布局/组件/UI/文本/Store 测试
docs/                   中文文档和截图
third_party/            Skia 预编译依赖（不提交到 Git）
```

## 状态与事件规范

- 应用状态只保留一个来源。
- 父组件通过 props 把只读值传给子组件。
- 子组件不直接改父状态，通过回调发事件。
- 数据流：子组件按钮 → 回调 → `State` 方法 → Runtime 重绘。
- 回调必须用 `ui.action` / `ui.textAction` / `ui.boolAction` /
  `ui.indexAction` 包装；Zig 没有闭包。
- 需要共享或异步状态时使用 `zant.store.Store(T)`，并在 UI 线程修改。
- 不要在组件内部藏全局可变状态；不要跨线程修改 UI。

## 组件与样式规范

- **所有组件的外观与交互以 Ant Design (antd 5) 为准**：新增或修改组件前先查对应的
  antd 组件与 design token（尺寸、圆角、间距、颜色、阴影、悬停/按下/禁用/聚焦状态），
  按 antd 规范实现，不要自创风格。没有对应 antd 组件时，用 antd token 组合出一致的视觉。
  - 颜色只用 `theme/tokens.zig` 中的 antd token（`color_text`、`color_fill_secondary`、
    `color_bg_elevated`、`color_primary_bg` …），并保证 `antLight` / `antDark` 都正确。
  - 常用规格：控件高 24/32/40，圆角 4/6/8，弹出层圆角 8、内边距 4、菜单项高 32、
    左右内边距 12。
  - 填充色与 antd 一致为**半透明**（`color_fill` .15 / `fill_secondary` .06 /
    `fill_tertiary` .04 / `fill_quaternary` .02，深色为白色 .18/.12/.08/.04），
    不要改成不透明色，否则叠在不同底色上会出现两种色调。
  - 文字/图标按钮：悬停 `fill_secondary`、按下 `fill`（不要用蓝色 `primary_bg`）；
    `primary_bg` 只用于菜单/列表的“选中项”。
  - 绝对定位元素的 `height_percent` / `width_percent` 优先于组件自带的固定尺寸
    （例如 `ui.styled(ui.button(..), .{ .height_percent = 100 })` 会填满父容器）。
  - 绝对定位与 CSS 一致：相对父元素的**含内边距盒子**定位和计算百分比，所以
    全尺寸点击层/悬停底色会覆盖父元素的 padding。
  - 已按 antd 实现：Button、Input、Checkbox/Switch/Radio、Select、Tabs、Card、Tag、
    Alert、Avatar、Progress、Divider、Tooltip、Dropdown 菜单（`ui.dropdownMenu`）、
    Segmented（`ui.segmented`）、Modal（`ui.modal`）、快捷键列表（`ui.shortcutList`）、全局提示（`ui.message`）。
  - **通用组件属于 Zant UI**：任何应用都可能用到的组件（模态框、快捷键列表、菜单、提示…）
    必须加到框架 `src/components/` 并经 `ui.*` 导出，应用里只写业务数据和组合；
    应用里发现缺的通用组件，先补进框架再使用。
  - 图标统一用 Zant 图标集（`ui.icon` / `ui.iconButton` / 按钮 `icon`，24×24 线性
    图标、统一线宽），不要用文字字符（☰、⋯）冒充图标。
  - 弹出层（菜单、面板）前放 `ui.dismissLayer`：点击外部任意位置关闭，且不绘制遮罩
    （需要遮罩时传 `background`）。弹层/模态容器设 `block_pointer = true`，空白处不穿透。

## 动效规范（详见 docs/motion.md）

- 动效规格照 antd motion（时长 100/200/300ms 与其缓动曲线），机制是 `src/motion/`：
  基于时间戳的 `motion.Animated`、presence（进场/退场）、layout/FLIP（`layout_transition` + `anchor`）。
- 新组件必须有符合 antd 的状态过渡（悬停/按下/聚焦/选中）：布尔状态是唯一来源，
  在组件的 `animate(data, now, theme)` 里用 `Animated.retarget` 缓动出呈现值供 paint 使用。
- 弹层、提示、模态用 presence（`ui.styled(.., .{ .motion = .slide_up / .zoom_big / .fade / .move_up })`），
  并给稳定 `key`，重新出现时复用同一节点。
- **动画帧只重绘、不重建**：不要用应用状态驱动动画，也不要在 `render` 里依赖动画进度。
- 状态变化处用 `tree.poke(id)` 登记动画节点；运行时只推进 `tree.animating`，不扫全树。
- 节点会被释放、`NodeId` 会被复用：框架外不要长期持有 `NodeId`；运行时新增跨帧
  保存 `NodeId` 的字段必须同时在 `Runtime.forgetNode` 里清理。
- 时间相关测试用假时钟（`runtime.clock = fakeClock`），不要 sleep；
  `ZANT_NO_MOTION=1`、截图模式、系统“减少动态效果”都会关闭动效。
- 命令式接口：`ui.focus(key)`、`ui.scrollIntoView(key)`（布局后执行）、`ui.wakeAfter(ms)`、
  `ui.nowMs()`；关窗/退出拦截：State 声明 `pub fn onCloseRequest(self) bool`。
- 文档类应用：`ui.documentEdited(bool)`（在 render 里声明未保存状态 → 关闭按钮圆点）；
  `WindowOptions.titlebar_height`（`.hidden_titlebar` 时让红绿灯垂直居中到自定义顶栏高度）。
- 绝对定位同时设 `left` 与 `right`（且无宽度）时按 CSS 拉伸到两侧之间；单行文本设
  `max_lines = 1` 时在空间不足处以“…”截断。

- 组件是普通函数：`fn component(props) ui.Node`。
- 业务组件只组合 `ui.*`，不直接调用 AppKit/Skia。
- 通用样式覆盖使用：

```zig
ui.styled(node, .{
    .background = 0xff111111,
    .border_color = 0xff333333,
    .border_width = 1,
    .radius = 4,
    .padding = ui.inset(12),
    .width = 240,
    .text_color = 0xffffffff,
})
```

- 支持 `background`、`border_color`、`border_width`、`radius`、`padding`、
  `margin`、`width/height`、`min/max`、`grow/shrink`、`clip`、文本颜色/字号/
  字重/对齐/行高/换行。
- 新增组件时必须验证 `ui.styled` 的外观覆盖实际生效，并补测试或视觉检查。
- 优先复用主题 token，不要散落魔法颜色。
- 单个组件自身 options 优先表达语义（variant、size、placeholder 等）；
  `ui.styled` 用于设计系统级统一覆盖。

## 布局规范

- 使用 `ui.column` / `ui.row` / `ui.card`，不要手写绝对坐标。
- 支持单行 Flex、横向 wrap、百分比宽高、order、align-self、align-content。
- 纵向 wrap、wrap-reverse、auto/calc 仍未支持。
- 固定高度控件（input/button/card）默认 `shrink = 0`，避免窗口缩小时被压扁。
- 生成项目默认最小窗口 600×620；内容放不下时优先调整布局，不要靠重叠。
- 文本宽度受容器约束；长文本使用 `ui.paragraph`。

## 窗口与配置

- 项目配置：`app.config.json`。
- 生成文件：`src/app_config.zig`、`Info.plist`、`app.entitlements`，
  不要手改。
- 配置字段：标题、Bundle ID、版本、`sandbox`、`signing_identity`，以及
  `window` 下的 style/width/height/min/background/resizable/always_on_top/
  movable_by_background/center。
- 代码中的 `WindowOptions` 优先级高于 JSON。
- 修改 JSON 后执行 `zant run` / `zant build` / `zant package`。

## 沙盒与签名

- `sandbox: true` 时打包写入 `com.apple.security.app-sandbox` entitlement。
- 沙盒应用必须通过 LaunchServices 启动；`zant run` 已处理。
- 签名身份来自本机 keychain 或 `app.config.json`；`none` 表示 ad-hoc。
- **不要把私钥、证书、密码、token、真实签名身份提交到 Git。**
  示例和文档使用通用占位符。
- 发布到外部分发前需要 notarization + stapling，当前未自动完成。

## Markdown / 数学

- 以 Vercel Streamdown 为蓝本逐模块移植（见 `docs/markdown.md`），改动前先对照
  上游实现，避免引入额外行为差异；remend 改动必须通过 `tests/remend_test.zig`。
- 性能优先：流式场景用 `markdown.Cache` + `markdown.streamdown`（块级解析缓存 +
  `ui.memo`），不要每帧整篇解析。
- `ui.memo` 子树内的回调上下文必须比帧活得久（用缓存条目的 arena），不要用
  `ui.frameAllocator()`。
- 数学排版改动参照 KaTeX 源码与字体参数，并用 `zig build run-math` 截图检查。
- Streamdown 为 Apache-2.0、KaTeX 为 MIT，署名见 `THIRD_PARTY_NOTICES.md`。

## PDF 组件

- `ui.pdfPreview` 使用原生 PDFKit 渲染到 PNG，再走 Skia 图片管线。
- 基本功能：首页/上一页/下一页/末页、页码、复制本页文本。
- 页面图片缓存在系统临时目录；不要每帧重复渲染。
- 扫描版 PDF 没有文本层时，复制无内容。
- 当前不支持拖拽选区、搜索、缩略图、连续滚动和缩放控件。
- 交互参考 EmbedPDF，但不嵌入 WebView/JS。

## 热更新

- `zant run --watch`：监听 `src/`、`src/components/`、`app.config.json`、
  `build.zig`、`build.zig.zon`，自动重编译并重启。
- 这是**重启式热更新**：窗口和内存状态会重建。
- App Sandbox 模式下不支持自动重启。
- 编译失败时保留旧窗口并等待下一次改动。
- 修改框架本身后执行 `zant update` 更新全局二进制。

## 性能规则

- 事件驱动重绘：静态窗口不要持续刷新。
- 文本布局按节点和宽度缓存；避免每帧重新 shaping。
- 图片按路径加载并缓存；避免每帧解码。
- PDF 页面按文件/页码/尺寸缓存 PNG；不要每帧渲染 PDF。
- 避免在 render 内做文件 I/O、网络、睡眠或大分配。
- 新增分配必须跟随节点/帧生命周期释放。
- 修改热点路径后运行 `zig build test`，并检查内存/时间回归。
- 当前测试 123/123（`tests/` + 框架模块内单元测试）；新增行为应补测试。

## 代码质量

- 运行 `zig fmt`。
- 保持 API 简单，优先组合而不是新增抽象。
- 不要引入不必要的依赖；Skia 通过 `fetch_skia.sh` 固定版本。
- 变更 C/ObjC++ ABI 时同步更新 `.h`、Zig extern 和文档。
- 提交前检查 `git diff`，确认没有密钥、本机绝对路径、测试产物。

## 已知限制

- Windows 未实现。
- 内置 `antLight` / `antDark`；应用可通过 `pub fn theme` 运行时切换。
- Flex 纵向 wrap、wrap-reverse、auto/calc 未支持。
- Select/Tabs 为基础实现。
- 真实 macOS 拼音输入法未自动化端到端验证。
- 无障碍 API 未暴露。
