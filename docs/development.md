# 开发

## 依赖

- Rust 1.94+（edition 2024；`rust-toolchain.toml` 固定 `stable` 通道，`rust-version = "1.94"` 兜底最低版本）
- macOS 13+（Apple Silicon）、Xcode 命令行工具，Skia 预编译包（首次执行 `./scripts/fetch_skia.sh`）
- 或 Windows 10/11 x64、Visual Studio 2022+ / Build Tools（「使用 C++ 的桌面开发」），
  依赖（Skia、头文件、PDFium、STIX Two Math）首次执行
  `powershell -ExecutionPolicy Bypass -File scripts/fetch_skia_windows.ps1`。
  原生代码由 `crates/zant-sys/build.rs` 用 MSVC 直接编进可执行文件（不再有 `zant_skia.dll`）。

沙箱 / AI 执行环境里 `~/.cargo` 不可写，**一律用 `./scripts/cargo.sh` 代替 `cargo`**
（它把 `CARGO_HOME` 指到仓库内的 `.cargo-home/`，已 gitignore）。普通机器上可以直接用 `cargo`。

Windows 上 Debug 构建是控制台程序（双击运行会带控制台窗口，便于看 stderr），Release 构建为 GUI 子系统。

## 常用命令

```bash
./scripts/fetch_skia.sh    # macOS 首次；Windows 用 scripts/fetch_skia_windows.ps1
cargo build --workspace
cargo test --workspace
cargo build --release --workspace
cargo run -p zant-examples --bin zant-demo       # 声明式示例
cargo run -p zant-examples --bin zant-gallery    # 命令式画廊
cargo run -p zant-examples --bin zant-markdown   # Markdown 渲染画廊（ZANT_DARK=1 深色）
cargo run -p zant-examples --bin zant-math       # 公式画廊
cargo run -p zant-examples --bin zant-components # 全部组件总览（ZANT_DARK=1 深色）
cargo run -p zant-examples --bin zant-pdf -- a.pdf # PDF 阅读器（ZANT_PDF_PAGE=起始页，从 0 开始，last 为最后一页；ZANT_PDF_ZOOM=1.5；
                                                   # ZANT_PDF_SEARCH=搜索词；ZANT_PDF_DEMO=1 示例批注；ZANT_PDF_PROBE=1 只输出文档信息）
```

沙箱里把 `cargo` 换成 `./scripts/cargo.sh`，例如 `./scripts/cargo.sh test -p zant markdown`。

## 环境变量

- 默认 GPU：macOS Skia Ganesh Metal，Windows Skia Ganesh Direct3D 12。
- `ZANT_RENDERER=raster`：CPU 光栅回退。
- `ZANT_SNAPSHOT=/path/out.png`：绘制后保存 PNG，用于视觉检查（截图模式关闭动效）。
- `ZANT_SNAPSHOT_EXIT=1`：截图后退出。
- `ZANT_NO_MOTION=1`：关闭动效（系统“减少动态效果”打开时也会关闭）。
- `ZANT_THEME=light|dark`：固定主题（截图、测试）；默认跟随系统。
- `ZANT_DARK=1`：示例程序切到深色主题。
- `ZANT_RAW_LOGS=1`：`zant run` 显示原始日志（macOS 默认隐藏输入法框架的无害日志；Windows 不过滤）。
- `ZANT_LAYOUT_FILE=/path/layout.txt`：每帧写出带 key 节点的位置（`key x y 宽 高`，逻辑像素，
  Tab 分隔），供 UI 自动化按 key 点击。
- `ZANT_FRAME_STATS=1`：每秒在 stderr 输出 `[frames]`：帧数、最慢一帧及其 rebuild / layout / paint
  耗时（Rust 与 Skia 录制，不含 GPU）、最长帧间隔、缩放时画出的新尺寸帧数与「尺寸变化 → 出帧」最长延迟；
  超过 8 ms 的帧另打一行 `[slow-frame]`（含当帧新建的文字排版数与耗时）。
- `ZANT_AUTOPLAY=1`（`zant-components`）：示例自己循环菜单、对话框、提示、主题等动画，
  用来无人值守地测帧率；`=quiet` 不打印 `[event]`。
- `ZANT_WINDOW_SIZE=WxH`：覆盖初始窗口尺寸（parity 截图用）。
- `ZANT_APP_EVENT_*` / `ZANT_POINTER_*` / `ZANT_KEY_*`：headless 测试驱动事件（见 `crates/zant/tests/`）。

性能要看 Release 构建（`cargo build --release`）：Debug 构建的 rebuild 会慢 10 倍以上。
Windows 上写 stderr 很贵（每行约 10 ms），不要在每帧都会执行的路径里打印。

视觉检查的示例文档：[`docs/showcase.md`](showcase.md) 覆盖全部 Markdown 与公式渲染。

## 测试

`cargo test --workspace`（macOS 与 Windows 都要通过）覆盖 Flex 布局、组件、声明式 UI 与协调、
节点生命周期、动效（假时钟）、文本、Store / SQLite、指针区域、文字选择、PDF 阅读器（选择 / 搜索 /
批注 / 导出）、Markdown（含 remend 522 条上游用例）与数学排版（含 mhchem 823 条用例全部可排版）、
以及 `zant-tsx` 的 lexer / parser / hooks / codegen 黄金用例。

- 沙箱里：`./scripts/cargo.sh test --workspace`。
- 只跑某个 crate：`./scripts/cargo.sh test -p zant markdown`、`./scripts/cargo.sh test -p zant-tsx`。
- 迁移期间同时用 **parity 对照**验证 Rust 版与 Zig 基线行为一致：
  `ZANT_ZIG_BIN=/path/to/zig-out-baseline/bin scripts/parity/compare.sh zant-demo`
  （layout 每个 key 差值 ≤ 0.01；像素通道差 > 2 的像素占比 ≤ 0.1%）。
- 时间相关测试注入假时钟（`runtime.clock = fake_clock`），不要 sleep。

## 帧基准

无窗口（光栅画布）驱动运行时，按场景统计整帧耗时（rebuild + layout + paint）：`open`（打开 30 条
约 5 KB 的长对话的首帧）、`stream`（流式回复）、`typing`（长文档上方输入）、`caret`（光标闪烁帧）、
`hover`（指针扫过长列表）、`resize`（窗口宽度变化）、`scroll`（滚轮）。`ZANT_BENCH_LIST=1` 用虚拟列表，
`ZANT_BENCH_MESSAGES` 改对话长度。结果确定、可复现，适合改动前后、两个平台对比。

> **迁移中（WP-2.11）**：`zant-bench` 的 Rust 版尚未合入。闸门要求（见 AGENTS.md「性能规则」）：
> 各场景不超过 Zig 基线 **1.25 倍**；在合入前用 Zig 版 `zig-out-baseline/bin/zant-bench` 与
> `zant-md-stream` 的帧统计做参照。

## 流式 Markdown 基准

`zant-md-stream`（`crates/zant-examples/src/stream_app.rs`）模拟聊天：若干历史消息 + 一条流式回复，
工作线程追加文本后 `ui::post_wake()`。用 Release 构建测：

```bash
cargo build --release -p zant-examples
ZANT_FRAME_STATS=1 ZANT_STREAM_EXIT=1 ./target/release/zant-md-stream            # 内置回复
ZANT_STREAM_HISTORY=30 ZANT_STREAM_CHARS=12 ZANT_STREAM_MS=8 ./target/release/zant-md-stream long.md
curl -N … | ./target/release/zant-md-stream --sse                                  # 真实 SSE（text_delta）
./target/release/zant-md-stream --sse-replay recorded.sse                          # 回放录制的 SSE
```

参考（Apple Silicon / i7-13700KF，4K 150%）：真实对话接口回放最慢一帧约 7 ms；30 条长历史 +
29 KB 回复以约 1200 字节/秒输出时稳定 60 帧。示例用虚拟列表渲染聊天，打开长对话只排版可见的消息。
测试不依赖真实窗口。

## macOS 界面场景测试

`scripts/mac/ui-scenario.swift` 用**真实的**鼠标和键盘事件驱动 Zant 窗口，并在指定时刻截取窗口截图
（Windows 上对应 `scripts/win/ui-auto.ps1`）。控件位置来自 `ZANT_LAYOUT_FILE`（按 key 定位），
所以步骤里写 key 而不是坐标：

```bash
swiftc -O scripts/mac/ui-scenario.swift -o /tmp/ui-scenario
ZANT_LAYOUT_FILE=/tmp/layout.txt ./target/debug/zant-components &
CONTENT_HEIGHT=480 /tmp/ui-scenario zant-components /tmp/layout.txt /tmp/shots <步骤文件>
```

步骤：`click <key> [次数]`、`hover <key>`、`away`（指针移出窗口，截图前用它避免悬停样式）、`type <文字>`
（Unicode 按键，可以输入中文）、`key return|backspace|tab|escape`、`resize <宽> <高>`、`wait <毫秒>`、
`shot <名字>`。终端需要“辅助功能”和“屏幕录制”权限。改界面后按 DESIGN.md 第 6 条逐张检查：
悬停 / 按下 / 聚焦 / 禁用、浅色 / 深色（`ZANT_DARK=1`）、最小窗口尺寸。帧率同时开 `ZANT_FRAME_STATS=1`
（用 Release 构建）。

## Windows 测试机

在 Mac 上开发、到 Windows 测试机构建和验证。主机与目录由 `ZANT_WIN_HOST`（`user@host`）和
`ZANT_WIN_DIR`（父目录，如 `D:/dev`）指定，可写在不提交的 `scripts/win/.env` 里。

```bash
scripts/win/sync.sh ../stillmd                     # 同步框架和应用（保持同级目录）
scripts/win/remote.sh XyUI 'cargo test --workspace'   # 远程执行（UTF-8 输出）
scripts/win/remote.sh XyUI 'powershell -File scripts/win/run-desktop.ps1 -Exe target\release\zant-demo.exe -Env "ZANT_SNAPSHOT=$env:TEMP\a.png;ZANT_SNAPSHOT_EXIT=1" -Wait 30'
```

**受限环境（AI 执行者 / 沙箱）**：`~/.ssh` 不可写时 `ssh` 无法更新 `known_hosts`，所有
`sync.sh` / `remote.sh` / 裸 `ssh` 调用前先设置 `ZANT_WIN_SSH_OPTS`（脚本会原样传给 `ssh` / `scp`）：

```bash
export ZANT_WIN_SSH_OPTS="-o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=/tmp/winhome/.ssh/known_hosts -o LogLevel=ERROR"
```

`scripts/win/sync.sh` 的远端目录名默认取本 checkout 的目录名；在 git worktree 里（目录名如
`wp-win`）与机器上的固定目录（如 `XyUI`）对不上时，在 `.env` 里加 `ZANT_WIN_PROJECT=XyUI`。
同步是**覆盖**式（打包 tracked + untracked 文件后解包），不会删除远端已不存在的文件：仓库里
删掉的文件（例如 M6 删掉的 `scaffold/template-ts/build.zig`）会留在远端，让测试看到幽灵文件。
远端源码与仓库不一致时重建：保留 `third_party/`、`target/`、`.zig-cache` 等目录，其余从空目录
重新同步一次。

> `scripts/win/remote.sh` / `run-desktop.ps1` 里的工具链（`ZANT_WIN_ZIG`）与产物路径（`zig-out\`）
> 还在从 Zig 迁移到 Cargo（**迁移中**），在 Windows 脚本更新前按脚本当前写法传参。
> Zig 基线在 `zig build -Doptimize=ReleaseFast` 之后位于 `zig-out\bin\`（含 `zant_skia.dll`、
> `pdfium.dll`，要一起用）；Rust 产物是 `target\{debug,release}\`。

一致性对照（Rust vs Zig 基线，plan.md §6/§7.2）在 Windows 上这样跑：`parity-compare.sh` 用
`parity-capture.ps1` 在机器上按 浅/深色 × 三种窗口尺寸抓 `shot.png` + `layout.txt`（Zig 用
`zig-out\bin`，Rust 用 `target\release`），把结果拷回 Mac 后用 `scripts/parity/{layoutdiff,imgdiff}.py`
比对；`ZANT_WIN_SKIP_CAPTURE=1` 只重跑比对。

SSH 会话里的窗口不可见、也拿不到真实缩放，所以视觉检查用 `run-desktop.ps1` 在已登录的
桌面会话里运行。一直运行的程序（例如测试同步时的应用）用自己的 `-Name`（计划任务名，一个名字
同时只能运行一个程序）和 `-Log <文件>`，之后再用 `run-desktop.ps1` 启动测试脚本不会互相影响。
参数里可以有中文路径（启动脚本按 UTF-8 写入）。桌面自动化测试（都输出 PASS / FAIL 报告和截图）：

| 脚本 | 内容 |
|---|---|
| `scripts/win/ui-auto.ps1` | 公共函数：启动应用（`ZANT_LAYOUT_FILE`）、按 key 点击 / 悬停、截图、核对 `[event]` 输出 |
| `scripts/win/components-check.ps1` | 点击 `zant-components` 的每个组件（按钮、输入、选择、菜单、对话框、提示、深色…） |
| `scripts/win/smoke-stillmd.ps1 -App <still. app.exe>` | 驱动 still.：输入、保存（CRLF）、剪贴板、对话框、拼音输入法、标题栏、拖动、关闭确认 |
| `scripts/win/docs-check.ps1 -App <still. app.exe>` | 用 still. 打开 `docs/` 下每个 md（预览 / 编辑）并截图 |
| `scripts/win/resize-check.ps1` | 真实鼠标 120 Hz 拖动窗口右下角缩放（含缩到最小），统计缩放帧率与延迟；先跑 `ZANT_AUTOPLAY` 测动画帧率；检查布局与最小尺寸 |
| `scripts/win/scaffold-check.ps1（Rust TSX 脚手架：create → cargo build → headless 快照）` | 在真实命令行窗口（代码页 936）运行 `zant create`，用 Unicode 按键输入中文标题，截图并检查生成的 `app.config.json` |
| `scripts/win/stream-resize-check.ps1 -Sse <录制的 SSE>` | `zant-md-stream` 回放真实回复的同时，真实鼠标拖动标题栏移动窗口、拖右下角放大 / 缩到最小 / 还原，每步截图，统计帧时间 |
| `scripts/win/pdf-check.ps1 -Folder <目录>` | 对目录里的 PDF 检查页数、文字、链接与目录、首页 / 中间页截图；真实鼠标拖选 + Ctrl+C、双击选词、缩放、滚轮、搜索、翻页 |
| `scripts/win/desktop-checks.ps1` | 依次跑上面这些检查（同一时间只能一个程序占鼠标 / 前台），把每份 report 汇总到 `E:\dev\checks\desktop\summary.txt`；`-Checks resize,stream` 可选子集，`-ComponentsExe` / `-MdStreamExe` / `-PdfExe` 换成任意实现（例如 Zig 基线做对照） |

用 `run-desktop.ps1 -Exe powershell.exe -Arguments "-NoProfile -ExecutionPolicy Bypass -File <脚本>"`
在桌面会话运行；结果与截图在 `%TEMP%\zant-checks\` 下。含中文的 `.ps1` 要保存为带 BOM 的
UTF-8（Windows PowerShell 5.1）。桌面脚本默认跑 `zig-out\bin\` 里的 Zig 版本；验证 Rust 版时传
`-Exe <repo>\target\release\<app>.exe`（`smoke-stillmd.ps1` / `docs-check.ps1` 传 `-App`）。
`stream-resize-check.ps1 -Sse` 需要一份录制的 SSE；仓库里的合成样例是
`scripts/win/fixtures/stream-replay.sse`（几十个 `data: {"type":"text_delta",…}`）。

## 脚手架

`crates/zant-cli/` 编译为 `target/debug/zant`：

```bash
cargo build -p zant-cli
./target/debug/zant create my-app
```

生成项目是 TSX 应用：`Cargo.toml` + `build.rs`（调 `zant_build::tsx_app()`）+ `src/App.tsx` +
`native/`，模板位于 `scaffold/template-ts/`，由 CLI 写入目标目录。`zant run` 用 `cargo build` 构建，
产物在 `target/{debug,release}/<应用名>`。框架更新后执行 `zant update` 重编译全局 CLI。
详见 [脚手架](scaffold.md)。

> **迁移中**：`zant run/compile/package` 对 TSX 应用的编译接口（WP-6.2）与 TSX 应用层
> （WP-5.6 / 5.8–5.10）尚未合入，当前只对已有 `Cargo.toml` 的 Rust 应用生效；
> 进度见 [`docs/rust-port/status.md`](rust-port/status.md)。

## 发布

开发者用一行命令安装（`install.sh` / `install.ps1`，见 README「安装」），装的是 GitHub Release 上的发布包：

1. 改根 `Cargo.toml` 的 `[workspace.package].version`（以及需要的话 `rust-version`；工具链由
   `rust-toolchain.toml` 固定，安装脚本装发布包里这个文件写的 channel，CI 用同一 channel，不用再改版本号），
   提交到 main；
2. 打标签并推送：`git tag v0.2.0 && git push origin v0.2.0`；
3. GitHub Actions（`.github/workflows/release.yml`）在 macOS 和 Windows 上下载 Skia 等依赖，用 rustup
   安装 `rust-toolchain.toml` 固定的工具链、`cargo build --release --locked` 编译 zant，用
   `scripts/release/package.sh` / `package.ps1` 打包（发布包 = `bin/zant` + `zant-ui/`，含 Rust workspace、
   原生源码和预编译的 Skia 库与头文件）；最后发布 Release，附件为
   `zant-macos-arm64.tar.gz`、`zant-windows-x64.zip`、各自的 `.sha256`、`version.txt` 和两个安装脚本。

手动运行这个工作流（Actions → release → Run workflow）只构建发布包作为 artifact，不发布。

本地自检与试打包：`ZANT_DRY_RUN=1 ./scripts/release/package.sh dev dist` 只检查输入并列出会打进包里的
文件（不编译、不写文件，可离线）；`./scripts/release/package.sh v0.0.0-test /tmp/rel` 真正打包，再用
`ZANT_HOME=/tmp/zh ZANT_DOWNLOAD=file:///tmp/rel bash install.sh` 安装测试（需要先把 `version.txt` 和
`install.sh` 放进那个目录）。

## 目录

- `crates/zant/src/ui/`：声明式 builders、选项结构体、`ui::styled`、memo、命令式辅助。
- `crates/zant/src/runtime/`：协调器、事件、焦点、IME、帧调度。
- `crates/zant/src/core/`：节点、树、布局、几何、Unicode、图标、矢量绘制、结构化哈希。
- `crates/zant/src/motion/`：缓动曲线、Animated、presence 预设。
- `crates/zant/src/markdown/`、`crates/zant/src/math/`：Markdown（Streamdown 移植）与 LaTeX 公式。
- `crates/zant/src/components/`：antd 风格组件（菜单、Segmented、Modal、Message …）。
- `crates/zant/src/renderer/`：Skia 桥（可移植核心 + mac / win 平台文件）。
- `crates/zant/src/platform/`：平台 ABI 的 Rust 包装；原生实现仍在 `src/platform/`（M8 移入 `zant-sys`）。
- `crates/zant-tsx/`：TSX 编译器（lexer / parser / hooks / codegen / tags / d.ts）。
- `crates/zant-cli/`：`zant` 命令；`crates/zant-build/`：应用 `build.rs` 辅助。
- `crates/zant/src/tsx/`：TSX 生成代码的运行时（hooks、字符串、内置、定时器、窗口）。
- `crates/zant/src/core/keys.rs`、`crates/zant/src/core/paths.rs`：平台快捷键与应用数据目录。
- `crates/zant/src/db/`：SQLite 封装（rusqlite；`#[derive(Table)]` 对象集合）。
- `scripts/win/`：Windows 测试机同步、远程执行与桌面冒烟测试。
- `scripts/parity/`：Rust 与 Zig 基线的布局 / 像素对照。
