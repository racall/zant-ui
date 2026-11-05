# AGENTS.md

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
- 当前测试 52/52；新增行为应补测试。

## 代码质量

- 运行 `zig fmt`。
- 保持 API 简单，优先组合而不是新增抽象。
- 不要引入不必要的依赖；Skia 通过 `fetch_skia.sh` 固定版本。
- 变更 C/ObjC++ ABI 时同步更新 `.h`、Zig extern 和文档。
- 提交前检查 `git diff`，确认没有密钥、本机绝对路径、测试产物。

## 已知限制

- Windows 未实现。
- 仅浅色主题。
- Flex 纵向 wrap、wrap-reverse、auto/calc 未支持。
- Select/Tabs 为基础实现。
- 真实 macOS 拼音输入法未自动化端到端验证。
- 无障碍 API 未暴露。
