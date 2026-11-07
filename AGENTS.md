# AGENTS.md

## ★ 最高优先级：设计规范 [DESIGN.md](DESIGN.md)

所有界面与交互的设计、实现和评审都**必须**遵循同目录的 [DESIGN.md](DESIGN.md)（UI/UX 设计指导原则）。
它的优先级高于本文件中的其他所有条目；与其他约定（包括 antd 风格规范）冲突时以 DESIGN.md 为准。
改动界面前先读它；完成后按其第 6 条检查不同窗口尺寸、浅/深色主题和各交互状态。

## 项目介绍

**Zant UI** 是一个用 Zig 编写的原生桌面 UI 框架，支持 macOS 与 Windows。

- 语言：Zig 0.17.1
- 窗口：macOS AppKit / NSWindow；Windows Win32 HWND（原生，不使用 WebView）
- 渲染：Skia Ganesh + Metal（macOS）/ Direct3D 12（Windows）；CPU 光栅回退用于测试/特殊环境
- 布局：Zig 实现的 CSS 风格 Flex（含换行 wrap、百分比、绝对定位）
- 文本：Skia Paragraph，支持中英文、换行、字体回退
- 状态：父组件持有状态 + 回调；可选 Zustand 风格 `zant.store.Store(T)`
- 脚手架：编译后的 `zant` CLI，不使用 shell 脚手架
- PDF：阅读器 `zant.pdf.view` 与单页预览（macOS PDFKit，Windows PDFium）
- 平台：macOS、Windows 10/11 x64 均已完成（见下文「跨平台规范」）

这个仓库既包含框架，也包含生成项目的模板和 CLI。

## 常用命令

```bash
./scripts/fetch_skia.sh       # 首次准备 Skia 依赖（macOS）
# Windows: powershell -ExecutionPolicy Bypass -File scripts/fetch_skia_windows.ps1
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
zant compile --release        # 只编译
zant package                  # 本机测试：macOS 生成 .app 并 codesign，Windows 生成 zig-out/<标题>/ 目录
zant build                    # 发布：签名、DMG / 公证 / App Store，或 Windows 安装包 / MSIX（docs/build.md）
zant update                   # 安装版：更新到最新发布包；源码版：git pull + 依赖 + 重新编译 CLI
```

## 目录结构

```text
src/
  ui.zig                声明式 ui.* builders、ui.styled、pointerArea、单页 PDF 预览
  ui_runtime.zig        协调器、事件、焦点、IME、帧调度
  launch.zig            最小启动入口
  store.zig             Zustand 风格 Store
  core/                 节点、树、布局、几何、事件、Unicode
  components/           按钮、输入、开关、Select、图片、菜单、Segmented、Modal、Message、
                        虚拟列表、快捷键列表 / 录入、标题栏按钮
  renderer/skia/        Skia C ABI：可移植核心 + mac（CoreText/Metal）/ win（DirectWrite/D3D12）
  platform/platform.h   平台 C ABI（两个平台各实现一份）
  platform/macos/       AppKit 窗口、PDFKit 桥
  platform/windows/     Win32 窗口、IMM32、自绘标题栏、对话框、PDFium、manifest
  motion/               动效：缓动曲线（antd）、Animated、presence 预设
  markdown/             Streamdown 移植：remend/ parser/ render/ blocks cache editor html
  math/                 LaTeX 公式（KaTeX 排版算法，STIX Two Math）、mhchem.zig（\ce / \pu）
  pdf/                  PDF 阅读器：document text renderer annotations viewer native
  image.zig             内存图片（zant.image.Image）
  core/sync.zig         工作线程用的 Mutex / Condition
  core/text_select.zig  selectable 容器的文字选择（命中、复制文本）
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
    Segmented（`ui.segmented`）、Modal（`ui.modal`）、快捷键列表（`ui.shortcutList`）、全局提示（`ui.message`）、
    Windows 标题栏按钮（`ui.windowControls`，运行时自动添加）。全部组件的可交互示例在
    `examples/components.zig`（`zig build run-components`）。
  - **通用组件属于 Zant UI**：任何应用都可能用到的组件（模态框、快捷键列表、菜单、提示…）
    必须加到框架 `src/components/` 并经 `ui.*` 导出，应用里只写业务数据和组合；
    应用里发现缺的通用组件，先补进框架再使用。
  - 图标统一用 Zant 图标集（`ui.icon` / `ui.iconButton` / 按钮 `icon`，24×24 线性
    图标、统一线宽），不要用文字字符（☰、⋯）冒充图标。
  - 弹出层（菜单、面板）前放 `ui.dismissLayer`：点击外部任意位置关闭，且不绘制遮罩
    （需要遮罩时传 `background`）。弹层/模态容器设 `block_pointer = true`，空白处不穿透。
  - 弹出层放在根容器里，用 `ui.keyBounds(触发控件的 key)` 定位到触发控件下方（放不下时翻到上方），
    不要写死窗口坐标（见 `docs/components/dropdown-menu.md`）。

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
- 文档类应用：`ui.documentEdited(bool)`（在 render 里声明未保存状态 → macOS 关闭按钮圆点，
  Windows 标题前 `*`）；`WindowOptions.titlebar_height`（`.hidden_titlebar` 时 macOS 红绿灯垂直居中、
  Windows 标题按钮填满自定义顶栏高度），顶栏两侧用 `ui.titlebarInsets()` 留位。
- 绝对定位同时设 `left` 与 `right`（且无宽度）时按 CSS 拉伸到两侧之间；单行文本设
  `max_lines = 1` 时在空间不足处以“…”截断。

- 组件是普通函数：`fn component(props) ui.Node`。
- 业务组件只组合 `ui.*`，不直接调用 AppKit / Win32 / Skia。
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

## 桌面集成（详见 docs/desktop.md）

- 托盘 / 状态栏、全局快捷键、剪贴板监听与图片、多窗口与 `.panel` 快捷面板、单实例、系统深色模式
  都走 `zant.desktop`，应用里不要写平台代码；示例项目 ClipHop（局域网剪贴板同步）。
- 应用钩子：`onStart`、`onAppEvent`、`onWake`、`onCloseRequest`、`onQuitRequest`。
- 多个窗口渲染同一份状态，任何窗口的动作都会重建所有窗口；`ui.focus` 等命令只作用于发出它的窗口。
- 程序坞图标 / 任务栏按钮由框架按窗口自动处理（有托盘时只在普通窗口开着时显示），应用不要自己切换；
  开机启动用 `setLaunchAtLogin`（系统机制），登录时后台启动用 `WindowOptions.hidden_at_login`。
- 网络代码用阻塞调用放在独立线程（Zig 0.17 `std.Io` 带超时的连接 / 接收在两端都不可用）。
- 改了 `scripts/win/sync.sh` 同步的文件列表后确认 Windows 端源码与本机一致（同步失败要能看到报错）。

## 窗口与配置

- 项目配置：`app.config.json`。
- 脚手架两端通用：`zant create` 只问通用设置（标题、窗口风格、应用标识 `bundle_id`）；
  平台专属的设置与文件由 `zant run` / `build` / `package` 在该平台补充（macOS：`macos` 段的
  `sandbox` / `signing_identity`，Info.plist、entitlements）。不要再让 create 问单个平台的问题。
- 图标：`zant create` 带 `assets/icons/` 默认图标（两端规范见 `docs/scaffold.md`「图标」）；
  `configureApp` 把它们加为 `@embedFile` 名字（`icon_tray_dark` 等，assets/icons 里每个 PNG 都是 `icon_<文件名>`），Windows 编译时生成 .ico 资源，
  `zant package` 在 macOS 生成 .icns；PNG 读写与缩放在 `src/core/png.zig`（CLI 与 `zant-icon` 工具共用）。
- 生成文件：`src/app_config.zig`（两端）、`Info.plist`、`app.entitlements`（仅 macOS），
  不要手改。Windows 上 `bundle_id` 用作 `%APPDATA%` 下的数据目录名（`zant.paths.appDataDir`）。
- 配置字段：标题、`bundle_id`、版本，`window` 下的 style/width/height/min/background/
  resizable/always_on_top/movable_by_background/center，以及 `macos.sandbox`、
  `macos.signing_identity`、`macos.entitlements`；发布设置在独立的 `build.config.json`
  （`zant build`，`tools/zant/distribute.zig` + `tools/zant/package/` 脚本，见 `docs/build.md`）。
- 代码中的 `WindowOptions` 优先级高于 JSON。
- 修改 JSON 后执行 `zant run` / `zant compile` / `zant package`。

## 沙盒与签名（macOS）

- 沙盒只用于 macOS，Windows 上 `sandbox` 被忽略；Windows 的 `zant package` 不签名，`zant build` 可以用
  signtool 签名（证书存储或 .pfx，见 `docs/build.md`）。
- `sandbox: true` 时打包写入 `com.apple.security.app-sandbox` entitlement。
- 沙盒应用必须通过 LaunchServices 启动；`zant run` 已处理。
- 签名身份来自本机 keychain 或 `app.config.json`；`none` 表示 ad-hoc。
- **不要把私钥、证书、密码、token、真实签名身份提交到 Git。**
  示例和文档使用通用占位符。
- 发布用 `zant build`：Developer ID 签名、DMG、公证与装订（notarytool + stapler），或 Mac App Store；
  见 `docs/build.md`。

## Markdown / 数学

- 以 Vercel Streamdown 为蓝本逐模块移植（见 `docs/markdown.md`），改动前先对照
  上游实现，避免引入额外行为差异；remend 改动必须通过 `tests/remend_test.zig`。
- 性能优先：流式场景用 `markdown.Cache` + `markdown.streamdown`（块级解析缓存 +
  `ui.memo`），不要每帧整篇解析。
- `ui.memo` 子树内的回调上下文必须比帧活得久（用缓存条目的 arena），不要用
  `ui.frameAllocator()`。
- 数学排版改动参照 KaTeX 源码与字体参数，并用 `zig build run-math` 截图检查。
- 预览默认可选择复制（`Options.selectable`，框架能力 `BoxOptions.selectable`）：新增渲染内容时，
  非正文（行号、标题栏、标记符号）用 `unselectable = true` 排除，嵌入绘制用 `RichSpan.copy` /
  `ui.vector(.{ .copy })` 提供复制文本（公式为 LaTeX 源码）。
- 扩展点：`Options.block_renderers`（按围栏语言渲染自定义块）、`Options.image_loader`（远程图片，
  加载完成后 `ui.postWake()`，图片 id 进入块的 memo hash）；`ParseOptions.latex_delimiters`。
- 数学新命令先对照 KaTeX 源码（`functions/*.js`、`stretchy.js`）的尺寸与间距再实现。
- Streamdown 为 Apache-2.0、KaTeX 为 MIT，署名见 `THIRD_PARTY_NOTICES.md`。

## PDF 组件

- 阅读器 `zant.pdf.view`（`src/pdf/`）：`Viewer` 由应用持有；连续滚动、缩放（适合宽度 / 页面、
  ⌘/Ctrl+滚轮、捏合）、文字选择复制、搜索、链接、批注（高亮 / 下划线 / 删除线 / 波浪线 /
  画笔 / 便签，撤销重做）与 `saveAnnotated` 导出标准 PDF 批注。见 `docs/components/pdf.md`。
- 改阅读器前先对照 EmbedPDF 对应插件（选区、缩放、分块渲染、批注）的实现，避免自创行为。
- 页面在后台线程分块渲染到内存（`zant.image.Image`），完成后 `ui.postWake()`；不要在 render
  里同步渲染页面。原生 PDF 调用都线程安全（PDFium 全局锁、PDFKit 每文档锁）。
- 单页预览 `ui.pdfPreview` 仍用 PNG 临时文件。Windows 需要 `pdfium.dll`
  （`configureApp` / `zant package` 会放到 exe 旁边，缺失时显示“无法打开”）。
- 扫描版 PDF 没有文本层时，无法选择 / 搜索。
- 交互参考 EmbedPDF，但不嵌入 WebView/JS。

## 热更新

- `zant run --watch`：监听 `src/`、`src/components/`、`app.config.json`、
  `build.zig`、`build.zig.zon`，自动重编译并重启。
- 这是**重启式热更新**：窗口和内存状态会重建。
- App Sandbox 模式下不支持自动重启。
- macOS：编译失败时保留旧窗口并等待下一次改动。Windows 不能覆盖正在运行的 exe，
  所以先结束应用再编译，编译失败时重新打开上一次的版本。
- 修改框架本身后执行 `zant update` 更新全局二进制（工作区有未提交修改时只重新编译，不拉取）。
- 一键安装：`install.sh` / `install.ps1` 下载 GitHub Release 的发布包（`scripts/release/package.sh` /
  `package.ps1` 打包，`.github/workflows/release.yml` 在推送 `v*` 标签时构建并发布）。发布包布局：
  `<安装目录>/bin/zant` + `<安装目录>/zant-ui/`（含 third_party 和 Windows 预编译桥接库
  `third_party/zant-skia-prebuilt/`），zant 运行时据此找到框架（`main.zig` 的 `findRoot`）。

## 性能规则

- 事件驱动重绘：静态窗口不要持续刷新。
- 文本布局按节点和宽度缓存；避免每帧重新 shaping。
- 图片按路径加载并缓存；避免每帧解码。
- PDF 页面由阅读器后台线程渲染并按页 / 缩放 / 分块缓存；不要每帧渲染 PDF。
- 避免在 render 内做文件 I/O、网络、睡眠或大分配。
- 新增分配必须跟随节点/帧生命周期释放。
- 修改热点路径后运行 `zig build test`，并检查内存/时间回归：Release 构建下用 `ZANT_FRAME_STATS=1`
  （`zant-components` 配合 `ZANT_AUTOPLAY=quiet`）看每帧耗时，单帧应远低于 16.7 ms；
  Windows 上用 `scripts/win/resize-check.ps1` 测拖动缩放帧率（应跟满刷新率）。
- 每帧路径里不要写 stderr（Windows 上每行约 10 ms）。
- 当前测试 164/164（`tests/` + 框架模块内单元测试），macOS 与 Windows 都要通过；新增行为应补测试。
- 文字排版缓存：`TextData` / `RichTextData` 保留当前与上一个宽度的排版，测量自然宽度走独立缓存
  （`measureWidth` / `naturalWidth`）；不要在测量路径里调 `ensureLayout` 换宽度，否则每帧会重排所有文字。
- 无确定高度的 column 不计算子节点的 flex 基准高度（不会用到），避免整棵子树按估计宽度多排一遍。
- 流式 Markdown：变化区间里的块按位置继承旧块的 id（UI key），正在生长的块复用节点；
  改 `cache.zig` / `blocks.zig` 后用 `zant-md-stream` 复测帧率。
- 宽度变化时文字复用已整形的排版只重新断行（`zant_text_layout_relayout`）；新增排版缓存逻辑要保证
  与新建排版结果一致（`tests/text_test.zig` 有对照测试）。
- 长列表 / 聊天用 `ui.list`（`docs/components/list.md`），不要把成百上千条放进普通滚动容器。
- 性能改动先用 `zig build bench`（`zant-bench`，无窗口、可复现）记录改动前数据，两边平台都对比；
  没有可测量收益的优化不要合入。

## 代码质量

- 运行 `zig fmt`。
- 保持 API 简单，优先组合而不是新增抽象。
- 不要引入不必要的依赖；Skia 通过 `fetch_skia.sh` / `fetch_skia_windows.ps1` 固定版本。
- 变更 C/ObjC++ ABI 时同步更新 `.h`、Zig extern、两个平台的实现和文档；改了
  `skia_bridge.h` / `pdf.h` 的函数要运行 `scripts/win/gen_def.sh` 更新 DLL 导出表。

## 跨平台规范

- 平台差异只放在 `src/platform/` 与 `src/renderer/skia/skia_bridge_{mac.mm,win.cpp}`；
  组件和应用代码不写 `builtin.os.tag` 分支，需要时用框架 API：
  - 快捷键：`key.modifiers.primary()`（⌘ / Ctrl）、`zant.keys.label("mod+shift+s")`
    生成按键文字（⇧⌘S / Ctrl+Shift+S），平台习惯不同用 `zant.keys.pick(mac, win)`。
  - 自绘标题栏：`ui.titlebarInsets()` 给出系统控件占位（macOS 左侧红绿灯，Windows 右侧标题按钮）；
    Windows 的最小化/最大化/关闭按钮由运行时自动绘制（`ui.windowControls`）。
  - 弹出层跟随控件：`ui.keyBounds(key)`（上一帧的窗口坐标）。
  - 数据目录：`zant.paths.appDataDir`；文件对话框：`zant.dialogs.*`（扩展名由调用方给）。
- Windows 构建：Zig 编译 `window.c`，Skia 桥接与 PDF 由 `scripts/win/build_bridge.ps1`
  用 MSVC 编成 `zant_skia.dll`（预编译 Skia 是 MSVC STL）；应用的 `build.zig` 调
  `@import("zant_ui").configureApp(...)` 嵌入 manifest、安装 DLL。
- 测试机：主机与目录写在不提交的 `scripts/win/.env`（`ZANT_WIN_HOST` / `ZANT_WIN_DIR` /
  `ZANT_WIN_ZIG`）；`scripts/win/sync.sh [../app]` 同步，`scripts/win/remote.sh <dir> '<cmd>'` 远程执行，
  `scripts/win/run-desktop.ps1` 在可见桌面运行（SSH 会话看不到窗口），
  桌面自动化测试见 `docs/development.md`「Windows 测试机」（`components-check.ps1` 点击全部组件、
  `smoke-stillmd.ps1`、`docs-check.ps1`、`pdf-check.ps1`、`resize-check.ps1`、
  `stream-resize-check.ps1` 流式输出中移动 / 缩放窗口），按 key 定位靠
  `ZANT_LAYOUT_FILE`。PowerShell 变量名不区分大小写，测试脚本里不要让 `$app` 和参数 `$App` 撞名。
- 新组件要加进 `examples/components.zig`（交互时打印 `[event] ...`），并在
  `scripts/win/components-check.ps1` 里补点击用例。
- 提交前检查 `git diff`，确认没有密钥、本机绝对路径、测试产物。

## 已知限制

- Windows 输入法走 IMM32；TSF 专有能力（重新转换等）未实现。Windows 仅 x64。
- 内置 `antLight` / `antDark`；应用可通过 `pub fn theme` 运行时切换。
- Flex 纵向 wrap、wrap-reverse、auto/calc 未支持。
- Select/Tabs 为基础实现。
- 真实 macOS 拼音输入法未自动化端到端验证（Windows 微软拼音已由冒烟测试覆盖）。
- 无障碍 API 未暴露。
