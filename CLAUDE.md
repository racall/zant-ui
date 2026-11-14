# CLAUDE.md

开始工作前先阅读同目录的 [AGENTS.md](AGENTS.md)，那里是完整规范。

## ★ 最高优先级：设计规范 [DESIGN.md](DESIGN.md)

所有界面与交互的设计、实现和评审都**必须**遵循同目录的 [DESIGN.md](DESIGN.md)（UI/UX 设计指导原则）。
它的优先级高于本文件中的其他所有条目；与其他约定（包括 antd 风格规范）冲突时以 DESIGN.md 为准。
改动界面前先读它；完成后按其第 6 条检查不同窗口尺寸、浅/深色主题和各交互状态。

## 快速上下文

- 项目：Zant UI，TSX + Rust 的原生桌面 UI 框架（macOS：Metal + AppKit；Windows：Direct3D 12 + Win32）。
  界面用 TSX 写，编译期由 `zant-tsx` 转成 Rust；底层能力用 Rust 写在应用的 `native/` 里。
- 工作区（Cargo workspace）：`crates/{zant-sys,zant,zant-macros,zant-tsx,zant-build,zant-cli,zant-examples}`。
- 入口/示例：`crates/zant-examples/src/bin/`（`zant-demo`、`zant-gallery`、`zant-components`、
  `zant-markdown`、`zant-md-stream`、`zant-math`、`zant-pdf`）；TSX 应用模板 `scaffold/template-ts/`。
- 声明式 API：`crates/zant/src/ui/`；协调器：`crates/zant/src/runtime/`。
- 组件：`crates/zant/src/components/`；样式统一入口：`ui::styled`（TSX 用组件自带属性 + style）。
- TSX 编译器：`crates/zant-tsx/`（tags / codegen / d.ts 生成）；TSX 运行时：`crates/zant/src/tsx/`。
- 脚手架：`crates/zant-cli/`；模板：`scaffold/template-ts/`；应用构建辅助：`crates/zant-build/`。
- 测试：`cargo test --workspace`（macOS 与 Windows 都要过；含 remend 522 条、mhchem 823 条上游用例）。
  沙箱里用 `./scripts/cargo.sh test --workspace`。
- 质量门：`./scripts/cargo.sh clippy --workspace --all-targets -- -D warnings`、`fmt --all -- --check`。
- Windows：测试机见 `AGENTS.md`「跨平台规范」；`scripts/win/` 同步、远程构建、桌面冒烟测试。
- 动效：`crates/zant/src/motion/`，规范见 `docs/motion.md` 与 `AGENTS.md`「动效规范」；
  `ZANT_NO_MOTION=1` 关闭动效。
- 全局 CLI：`zant`；框架更新后执行 `zant update`。一键安装 `install.sh` / `install.ps1`（发布包，
  `scripts/release/`，推送 `v*` 标签由 `.github/workflows/release.yml` 发布，见 `docs/development.md`「发布」）。
- 发布构建：`zant build [配置文件]`（`build.config.json`，`docs/build.md`）：zant 读配置、提问、编译签名，
  平台步骤在 `crates/zant-cli/src/distribute.rs` 与 `scripts/` 下的打包脚本
  （macOS DMG / 公证 / App Store，Windows Inno Setup 安装包 / MSIX）。
- 热更新：`zant run --watch`（重启式，非沙盒，监视 `src/`、`native/`、`app.config.json`、`Cargo.toml`、`build.rs`）。
- macOS 上 `zant run` 会隐藏输入法框架的无害日志（IMKCFRunLoopWakeUpReliable 等），
  需要原始输出时设 `ZANT_RAW_LOGS=1`。
- PDF：阅读器 `PdfViewer`（`crates/zant/src/pdf/`：连续滚动、缩放、选择复制、搜索、批注与导出；
  算法参照 EmbedPDF），单页预览 `PdfPreview`；macOS PDFKit，Windows PDFium。
- TSX：`crates/zant-tsx/`（TSX → Rust，编译期完成，无 JS 运行时），规范与进度见 `docs/tsx.md`；
  应用 `build.rs` 调 `zant_build::tsx_app()`，生成到 `OUT_DIR/app.rs` 由 `src/main.rs` `include!`。
  示例与编译器应用层（WP-5.6 / 5.8–5.10、WP-6.2）在**迁移中**，以 `docs/rust-port/status.md` 为准。
- TS 应用的 Rust 代码放 `native/`（`<名字>.ts` 规范 + `.rs` 实现，编译期由 trait 检查），见 `docs/tsx-native.md`。
- 原始指针输入：`PointerArea`（拖动捕获、多击、滚轮、按键）；尺寸变化 `onResize`；
  内存图片 `zant::image::Image` + `<Image image>`；后台线程唤醒 UI `ui::post_wake()`。
- Markdown：`crates/zant/src/markdown/`（Streamdown 移植，模块化），数学：`crates/zant/src/math/`
  （KaTeX 算法，mhchem 为 KaTeX contrib/mhchem 的逐字节对齐移植）。详见 `docs/markdown.md`；
  示例 `zant-markdown` / `zant-math` / `zant-md-stream`（流式帧率基准，可读真实 SSE）。
- 性能基准：`zant-bench`（打开长对话、流式、打字、光标、悬停、缩放、滚动；`ZANT_BENCH_LIST=1`
  用虚拟列表；**迁移中**：WP-2.11）。优化前先建基线（Release），两边平台对比。
- 桌面集成：`zant::desktop`（`docs/desktop.md`）：多窗口 / `.panel` 快捷面板、托盘与状态栏、全局快捷键、
  剪贴板监听与图片、单实例、深色模式；TSX hooks `useTray` / `useHotkey` / `useClipboardChange` /
  `useReopen` / `useWake` / `useAppExit` / `createWindow`。
  组件 `Clickable`、`HotkeyInput`、`showOnHover`。示例项目 ClipHop。
- 虚拟列表：`List` + `ui::ListState`（`docs/components/list.md`）；悬停样式 `hoverBackground`；
  焦点范围 `focusScope`（`Modal` 自带）。
- 流式性能：改 Markdown / 布局 / 文本后，用 Release 的 `zant-md-stream` 配 `ZANT_FRAME_STATS=1`
  复测（长回复 + 多条历史），单帧应远低于 16.7 ms，`[slow-frame]` 的 text_layouts 不应随内容增长。
- 视觉检查：`ZANT_SNAPSHOT=out.png ZANT_SNAPSHOT_EXIT=1 ./target/release/<app>`；Windows 的窗口要在
  已登录桌面里看（`scripts/win/run-desktop.ps1`），SSH 会话里不可见。
- 自动化与性能：`ZANT_LAYOUT_FILE`（按 key 定位控件）、`ZANT_FRAME_STATS=1`（每秒帧统计，
  性能看 Release 构建）、`zant-components` 的 `ZANT_AUTOPLAY=quiet`；Windows 桌面测试脚本见
  `docs/development.md`「Windows 测试机」。
- 平台差异不写进组件和应用：快捷键用 `keys::label` / `keyLabel`，自绘标题栏用 `ui::titlebar_insets()`，
  浮层跟随控件用 `ui::key_bounds()`，数据目录用 `zant::core::paths`（见 `AGENTS.md`「跨平台规范」）。
- 迁移文档：`docs/rust-port/`（`plan.md`、`api.md`、`status.md`、`conventions.md`、`tsx-codegen.md`）。

## 改动原则

0. **所有组件风格参考 Ant Design (antd 5)**：尺寸、圆角、间距、颜色 token、阴影和
   悬停/按下/禁用状态都按 antd 规范实现，浅色/深色都要正确；图标用 Zant 图标集（`Icon`），
   弹出菜单用 `DropdownMenu` + `DismissLayer`。详见 `AGENTS.md`「组件与样式规范」。
   每个组件的状态变化都要有 antd 风格的细腻过渡（`docs/motion.md`）；动画帧只重绘不重建。
   通用组件（模态框、快捷键列表、菜单…）一律放进 Zant UI（`crates/zant/src/components/` + `ui::*` 导出），
   应用里只写业务数据；应用缺通用组件时先补框架。

1. 先读 `AGENTS.md`，再改代码。
2. 保持状态单一来源：props 向下，事件向上。
3. 新组件必须是 `fn(props) -> ui::Node`（TSX 是函数组件），不依赖平台细节。
4. 新增外观属性要真正被 paint 使用；不要只加字段不渲染。
5. 改布局/渲染后补测试，并运行 `cargo test --workspace`。
6. 不提交密钥、证书、token、真实签名身份或本机绝对路径；Windows 测试机的地址只写在
   不提交的 `scripts/win/.env` 里。
7. 提交前运行 `./scripts/cargo.sh fmt --all`、`clippy` 与 `git diff` 检查；不要提交 `target/`、
   `.cargo-home/`、`third_party` 或测试项目；改了 `skia_bridge.h` / `pdf.h` 的函数要同步
   `zant-sys` 的 bindgen 绑定（`scripts/gen_bindings.sh`），并确认 macOS 与 Windows 都通过测试。
