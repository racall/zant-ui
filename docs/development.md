# 开发

## 依赖

- Rust 1.94.1（edition 2024；工作区 `rust-toolchain.toml` 固定精确版本，`rust-version = "1.94"` 声明最低版本；
  `zant create` 生成的应用复制框架固定的 channel，当前也是 `1.94.1`）
- macOS 13+（Apple Silicon）、Xcode 命令行工具，Skia 预编译包（首次执行 `./scripts/fetch_skia.sh`）
- 或 Windows 10/11 x64、Visual Studio 2022+ / Build Tools（「使用 C++ 的桌面开发」），
  依赖（Skia、头文件、PDFium、STIX Two Math）首次执行
  `powershell -ExecutionPolicy Bypass -File scripts/fetch_skia_windows.ps1`。
  原生代码由 `crates/zant-sys/build.rs` 用 MSVC 直接编进可执行文件（没有单独的 Skia DLL）。

沙箱 / AI 执行环境里 `~/.cargo` 不可写，**一律用 `./scripts/cargo.sh` 代替 `cargo`**
（它把 `CARGO_HOME` 指到仓库内的 `.cargo-home/`，已 gitignore）。普通机器上可以直接用 `cargo`。

Windows 上 Debug 构建是控制台程序（双击运行会带控制台窗口，便于看 stderr），Release 构建为 GUI 子系统。

## 热刷新验证

TSX 项目默认 `zant run` 原地刷新，行为与限制见 [脚手架](scaffold.md#热更新)。
普通模式 `zant run --no-hot` 用来对照帧耗时与沙盒行为；`--watch` 选择旧重启模式。
构建宿主和共享原生层后，应用编辑只重新编译 UI crate。框架由 `crates/zant-dylib` 共享，
会话内只加载一次；框架源码不在应用监听范围内，修改后须结束并重新运行 `zant run`，重建宿主和 UI 库。
应用配置、Cargo 清单等触发 CLI 自身重启；完整分类规则见 `crates/zant-cli/src/hot/classify.rs`。

- `ZANT_HOT_TRACE=1`：输出文件事件到防抖结束、代码生成、库发布、后台加载、UI 线程的保存 / 销毁 /
  创建与首帧耗时，以及每次替换后旧 UI 映像是否仍在加载列表中（`gone` / `still mapped`）。
  Cargo 的构建耗时包含代码生成与链接；不要把包含关系中的数值重复相加。
- 旧 UI 库默认在旧实例销毁、新实例成功启动后卸载：框架（`zant-dylib`）只加载一次，
  每次热刷新只换应用自己的小库，避免逐代保留应用库。
  停止实例时 `zant::hot::release_app()` 关闭副窗口并清理框架持有的应用回调、native 模块、
  定时器、桌面注册、构建上下文和资源地址缓存；宿主主窗口保留。
  应用自行创建的线程、全局变量与外部资源仍须自行停止 / 释放，不能在卸载后执行应用代码。
  `ZANT_HOT_KEEP_LIBS=1` 禁止卸载，用于诊断；启动失败回滚时始终保留旧库。
- `ZANT_HOT_SNAPSHOT=/path/shots`：首次加载与每次热刷新后保存 `reload-<PID>-NNNN.png`；加载前保存
  `state-<PID>-NNNN.json`（包含应用状态和视图状态），同时关闭动效。快照可能包含输入内容，只存开发者指定目录。
  截图在本机后端执行；没有 WindowServer 时只能验证 CPU 光栅结果。
- `ZANT_HOT_POLL=1`：不能使用原生文件通知的网络盘 / 沙箱中，显式启用 100 ms 内容轮询。
  会增加空闲扫描与最多约 100 ms 检测延迟，正常开发使用默认原生监听。相同内容保存不构建。
- `ZANT_HOT_CONTROL=/path/fifo`：显式打开一条阻塞读的测试输入通道，接受 `pointer <类型> <x> <y>`
  （1 移动 / 2 按下 / 3 松开）、`scroll <x> <y> <dy>`、`key <键码> <修饰位>`、`text <内容>`、
  `snapshot`、`close`。坐标为逻辑像素，键码 / 修饰位来自 `platform.h`。所有事件在宿主 UI 线程转发。
  这是 host/runtime 边界回放，不能替代真实 macOS 鼠标 / 输入法 / GPU 端到端检查。

macOS 可重复的无桌面验证（保留新建应用、日志、PID、截图与状态 envelope；输出目录需为空）：

```bash
./scripts/cargo.sh build -p zant-cli
python3 scripts/tsx/hot-check.py /tmp/zant-hot-evidence
```

脚本检查计数、类型变更只重置对应字段、滚动 / 焦点 / 光标保持，新增组件与布局，TSX / Rust 错误恢复，
原生代码修改、不变保存、构建中多次保存合并、配置 / Cargo 清单重启以及关窗结束会话。

单独检查模板的 TSX 与 native 规范代码生成（不链接或启动应用）：

```bash
./scripts/cargo.sh run -q -p zant-tsx --bin zant-tsx -- \
  --out /tmp/zant-template-rust --src scaffold/template-ts/src --native scaffold/template-ts/native
```

有桌面权限时再用下文 `scripts/mac/ui-scenario.swift` 按 `ZANT_LAYOUT_FILE` 的 key 实际点击。

需要分离 rustc 与链接耗时时可选用 Python wrapper（普通开发不启用）：

```bash
RUSTC_WRAPPER=/path/zant-ui/scripts/tsx/hot-profile.py \
ZANT_HOT_PROFILE=/tmp/hot-profile ZANT_HOT_TRACE=1 zant run
```

`compiler.jsonl` 中 `rustc` 包含 `link`，相减得到编译器其余阶段；wrapper 本身有进程启动开销，
完整保存到首帧时间应再关闭 wrapper 单独测量。UI crate 使用较少调试信息，macOS 使用 `unpacked`
避免生成 dSYM；依赖的 profile、features 和 `ZANT_NATIVE_SHARED` 在整个会话固定。
每代库用新路径，优先硬链接（失败回退复制）；库加载在后台，UI 线程只保存、销毁和恢复状态。
macOS 加载新库可能受系统安全检查影响；后台加载超过 250 ms 时宿主每个进程只提示一次，
`ZANT_HOT_QUIET=1` 可关闭提示。热刷新发布库时不额外执行 codesign。Cargo 通过 nice / BELOW_NORMAL_PRIORITY_CLASS 降低优先级，
受限环境若拒绝 nice 会显示系统错误。发布构建保持普通静态链接与原有签名流程。

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
- `ZANT_THEME=light|dark`：固定 TSX 应用主题（截图、测试）；TSX 应用默认跟随系统。
- `ZANT_DARK=1`：示例程序切到深色主题。
- `ZANT_RAW_LOGS=1`：`zant run` 显示原始日志（macOS 默认隐藏输入法框架的无害日志；Windows 不过滤）。
- `ZANT_LAYOUT_FILE=/path/layout.txt`：每帧写出带 key 节点的位置（`key x y 宽 高`，逻辑像素，
  Tab 分隔），供 UI 自动化按 key 点击。
- `ZANT_INPUT_TRACE=1`：在 stderr 记录指针按下 / 松开进入运行时、每帧开始的时间；macOS Metal 后端还记录
  从帧提交到实际呈现的间隔，用于定位输入到屏幕的延迟。需在启动前设置，日常测性能时关闭，避免日志影响结果。
- `ZANT_FRAME_STATS=1`：每秒在 stderr 输出 `[frames]`：帧数、最慢一帧及其 rebuild / layout / paint
  耗时（Rust 与 Skia 录制，不含 GPU）、最长帧间隔、缩放时画出的新尺寸帧数与「尺寸变化 → 出帧」最长延迟；
  超过 8 ms 的帧另打一行 `[slow-frame]`（含当帧新建的文字排版数与耗时）。
- `ZANT_AUTOPLAY=1`（`zant-components`）：示例自己循环菜单、对话框、提示、主题等动画，
  用来无人值守地测帧率；`=quiet` 不打印 `[event]`。
- `ZANT_WINDOW_SIZE=WxH`：覆盖初始窗口尺寸（自动化截图用）。

性能要看 Release 构建（`cargo build --release`）：不要用 Debug 构建的耗时推断发布版性能。
Windows 上写 stderr 会影响帧耗时，不要在每帧都会执行的路径里打印。

视觉检查的示例文档：[`docs/showcase.md`](showcase.md) 覆盖全部 Markdown 与公式渲染。

## 测试

`cargo test --workspace`（macOS 与 Windows 都要通过）覆盖 Flex 布局、组件、声明式 UI 与协调、
节点生命周期、动效（假时钟）、文本、Store / SQLite、指针区域、文字选择、PDF 阅读器（选择 / 搜索 /
批注 / 导出）、Markdown（含 remend 522 条上游用例）与数学排版（含 mhchem 823 条用例全部可排版）、
以及 `zant-tsx` 的 lexer / parser / hooks / codegen 黄金用例。

- 沙箱里：`./scripts/cargo.sh test --workspace`。
- 只跑某个 crate：`./scripts/cargo.sh test -p zant markdown`、`./scripts/cargo.sh test -p zant-tsx`。
- 两张截图的像素对比：`python3 scripts/lib/imgdiff.py a.png b.png [--diff out.png]`
  （默认像素通道差 > 2 的像素占比 ≤ 0.1% 算一致；纯标准库，不需要 Pillow）。
- 时间相关测试注入假时钟（`runtime.clock = fake_clock`），不要 sleep。

## 帧基准

无窗口（光栅画布）驱动运行时，按场景统计整帧耗时（rebuild + layout + paint）：`open`（打开 30 条
约 5 KB 的长对话的首帧）、`stream`（流式回复）、`typing`（长文档上方输入）、`caret`（光标闪烁帧）、
`hover`（指针扫过长列表）、`resize`（窗口宽度变化）、`scroll`（滚轮）。`ZANT_BENCH_LIST=1` 用虚拟列表，
`ZANT_BENCH_MESSAGES` 改对话长度。结果确定、可复现，适合改动前后、两个平台对比。

```bash
scripts/bench.sh                    # 构建 Release 的 zant-bench，两种模式各跑 3 轮取最好一轮
scripts/bench.sh --no-build stream  # 用已有的 target/release/zant-bench，只跑 stream
```

> 闸门要求见 AGENTS.md「性能规则」：各场景平均帧耗时不超过记录基线 **1.25 倍**，稳态场景低于 16.7 ms
> （`scripts/bench.sh` 内置记录基线数字；原始输出在 `target/bench/`）。Windows 测试机上用
> `scripts/win/bench.ps1` 跑同样的场景并留下日志。

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

示例用虚拟列表渲染聊天，打开长对话只排版可见的消息；通过 `ZANT_FRAME_STATS=1` 记录本机结果。
需要无窗口验证时设置 `ZANT_RENDERER=raster` 与 `ZANT_SNAPSHOT`；真实窗口的滚动与缩放检查见下文。

## macOS 界面场景测试

`scripts/mac/ui-scenario.swift` 用**真实的**鼠标和键盘事件驱动 Zant 窗口，并在指定时刻截取窗口截图
（Windows 上对应 `scripts/win/ui-auto.ps1`）。控件位置来自 `ZANT_LAYOUT_FILE`（按 key 定位），
所以步骤里写 key 而不是坐标：

```bash
swiftc -O scripts/mac/ui-scenario.swift -o /tmp/ui-scenario
ZANT_LAYOUT_FILE=/tmp/layout.txt ./target/debug/zant-components &
CONTENT_HEIGHT=820 /tmp/ui-scenario zant-components /tmp/layout.txt /tmp/shots steps.txt
```

`CONTENT_HEIGHT` 要填实际窗口内容高度（点），`zant-components` 初始内容高度为 820；改变窗口大小后需相应调整。

步骤：`click <key> [次数]`、`hover <key>`、`away`（指针移出窗口，截图前用它避免悬停样式）、`type <文字>`
（Unicode 按键，可以输入中文）、`key return|backspace|tab|escape`、`resize <宽> <高>`、`wait <毫秒>`、
`shot <名字>`。终端需要“辅助功能”和“屏幕录制”权限。改界面后按 DESIGN.md 第 6 条逐张检查：
悬停 / 按下 / 聚焦 / 禁用、浅色 / 深色（`ZANT_DARK=1`）、最小窗口尺寸。帧率同时开 `ZANT_FRAME_STATS=1`
（用 Release 构建）。

`scripts/mac/components-check.sh` 使用 `components.steps` 检查组件事件、禁用状态、菜单定位、提示消失、文字复制和虚拟列表，
将 `report.txt`、`app.log` 与 `shots/` 截图写到 `${OUT:-$TMPDIR/zant-checks/components}`。默认不构建；`--build` 用
`./scripts/cargo.sh` 构建，`EXE` 可指定已有程序，`ZANT_DARK=1` 从深色开始（沿用 Windows 截图名，主题切换后会恢复）。
新增步骤 `expect <文字>` 等待至多 1.5 秒，只匹配 `EVENT_LOG` 中上次 `expect` / `noevent` 之后的新完整行；
`noevent <毫秒>` 检查没有新的 `[event]` 行；`rss <标签>` 读取 `APP_PID` 的 RSS（KiB）；`repeat <次数>` 到 `end`
重复步骤，不支持嵌套。滚动、布局和剪贴板辅助断言的参数见 Swift 文件头。脚本预热 20 轮后再跑 20 轮，
RSS 比预热后增长超过 5% 时标记泄漏，并汇总最差的 `[frames] max_frame_ms`；断言失败或泄漏均返回非零，并退出应用。
Debug 耗时只供排查，性能结论需用 Release。没有桌面权限时，编译驱动后执行
`DRY_RUN=1 /tmp/ui-scenario zant-components /tmp/layout.txt /tmp/shots scripts/mac/components.steps`
可解析并打印展开步骤而不访问 GUI，`tsx-counter.steps` 也可同样检查。`STEPS=<文件>` 换用自己的步骤文件（复现单个问题）。

**输入只发给被测应用**：从后台启动的应用在 macOS 14+ 上无法被切到前台（协作式激活），系统事件流里的按键
会进入当时的前台应用（例如你的编辑器）。所以键盘事件用 `postToPid` 只投递给被测进程；鼠标和滚轮按屏幕位置分发，
发送前先检查该位置最上层的窗口属于被测进程，被挡住就停止场景；滚轮事件放在场景最后移动到的位置。
启动时用辅助功能把测试窗口提到最上层。修改 `ui-scenario.swift` 的事件发送时必须保持这一点。

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

`scripts/win/sync.sh` 的远端目录名默认取本 checkout 的目录名；本地目录名与机器上的固定目录
（如 `XyUI`）对不上时，在 `.env` 里加 `ZANT_WIN_PROJECT=XyUI`。
同步是**覆盖**式（打包 tracked + untracked 文件后解包），不会删除远端已不存在的文件：仓库里
删掉的文件会留在远端，让测试看到幽灵文件。
远端源码与仓库不一致时重建：保留 `third_party/`、`target/` 等目录，其余从空目录
重新同步一次。构建产物在 `target\{debug,release}\`。

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
| `scripts/win/scaffold-check.ps1` | 在真实命令行窗口（代码页 936）运行 `zant create`，用 Unicode 按键输入中文标题，检查生成项目并构建、保存无窗口快照 |
| `scripts/win/hot-check.ps1` | 在真实桌面验证 TSX 热刷新与状态恢复；必须传 `-Work <可删除的测试目录>`，可用 `-Zant` 指定 CLI（脚本会重建 Work 目录） |
| `scripts/win/stream-resize-check.ps1 -Sse <录制的 SSE>` | `zant-md-stream` 回放真实回复的同时，真实鼠标拖动标题栏移动窗口、拖右下角放大 / 缩到最小 / 还原，每步截图，统计帧时间 |
| `scripts/win/pdf-check.ps1 -Folder <目录>` | 对目录里的 PDF 检查页数、文字、链接与目录、首页 / 中间页截图；真实鼠标拖选 + Ctrl+C、双击选词、缩放、滚轮、搜索、翻页 |
| `scripts/win/desktop-checks.ps1` | 依次跑上面这些检查（同一时间只能一个程序占鼠标 / 前台），把每份 report 汇总到 `%TEMP%\zant-checks\desktop\summary.txt`（`-Out` 可改）；`-Checks resize,stream` 可选子集，`-ComponentsExe` / `-MdStreamExe` / `-PdfExe` 指定被测程序（默认 `target\release\`），`-StillExe` 指定 still. 应用（`smoke` / `docs` 两项用） |

用 `run-desktop.ps1 -Exe powershell.exe -Arguments "-NoProfile -ExecutionPolicy Bypass -File <脚本>"`
在桌面会话运行；多数脚本的结果与截图在 `%TEMP%\zant-checks\` 下（`hot-check.ps1` 用 `-Work`）。含中文的 `.ps1` 要保存为带 BOM 的
UTF-8（Windows PowerShell 5.1）。单个桌面脚本默认跑 `target\debug\` 里的示例；测 Release 时传
`-Exe <repo>\target\release\<app>.exe`（`pdf-check.ps1` / `smoke-stillmd.ps1` / `docs-check.ps1` 传 `-App`）。
`stream-resize-check.ps1 -Sse` 需要一份录制的 SSE；仓库里的合成样例是
`scripts/win/fixtures/stream-replay.sse`（几十个 `data: {"type":"text_delta",…}`）。

## 脚手架

`crates/zant-cli/` 编译为 `target/debug/zant`：

```bash
cargo build -p zant-cli
./target/debug/zant create my-app
```

推荐用 TSX 编写应用。生成项目的 `src/` 只放 `.tsx` / `.ts`，所有 Rust 放在 `native/`：
`native/main.rs` 是 `zant::app!();`，`native/build.rs` 是 `fn main() { zant_build::tsx_app(); }`。
`Cargo.toml` 指定这两个路径；构建器生成 `$OUT_DIR/zant_app.rs`、`zant_main.rs`、`zant_hot.rs`，
自动声明顶层原生模块，并注册带同名 `.ts` 规范的 `Native` 实现，无需手写模块树或注册表。
模板位于 `scaffold/template-ts/`，由 CLI 写入目标目录。默认 `zant run` 使用开发宿主与应用动态库；`zant compile` / `zant run --no-hot` 构建
普通可执行文件，产物在 `target/{debug,release}/<Cargo 二进制名>`（Windows 加 `.exe`）。框架更新后执行 `zant update` 重编译全局 CLI。
详见 [脚手架](scaffold.md)。

## 发布

开发者用一行命令安装（`install.sh` / `install.ps1`，见 README「安装」），装的是 GitHub Release 上的发布包：

1. 改根 `Cargo.toml` 的 `[workspace.package].version`（以及需要的话 `rust-version`；工具链由
   `rust-toolchain.toml` 固定，安装脚本装发布包里这个文件写的 channel，CI 用同一 channel，不用再改版本号），
   提交到 main；
2. 打标签并推送：`git tag v0.2.0 && git push origin v0.2.0`；
3. GitHub Actions（`.github/workflows/release.yml`）在 macOS 和 Windows 上下载 Skia 等依赖，用 rustup
   安装 `rust-toolchain.toml` 固定的工具链，先用 `scripts/release/prebuilt.sh` / `prebuilt.ps1` 构建原生层，
   并运行 `cargo test -p zant-sys` 验证预编译库可链接、可运行；再以 `ZANT_NATIVE_PREBUILT` 指向该库，
   用 `cargo build --release --locked -p zant-cli --bin zant` 编译 zant，用
   `scripts/release/package.sh` / `package.ps1` 打包（发布包 = `bin/zant` + `zant-ui/`，含 Rust workspace、
   原生源码、Skia 库与头文件，以及 `third_party/zant-native-prebuilt/` 的原生静态库与源码哈希元数据）；最后发布 Release，附件为
   `zant-macos-arm64.tar.gz`、`zant-windows-x64.zip`、各自的 `.sha256`、`version.txt` 和两个安装脚本。

手动运行这个工作流（Actions → release → Run workflow）只构建发布包作为 artifact，不发布。

本地自检与试打包（先准备 Skia 和 `scripts/release/prebuilt.sh` 生成的原生库，
或用 `ZANT_NATIVE_PREBUILT` 指向现有库）：`ZANT_DRY_RUN=1 ./scripts/release/package.sh dev dist` 只检查输入并列出会打进包里的
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
- `crates/zant/src/platform/`：平台 ABI 的 Rust 包装；原生实现在 `crates/zant-sys/native/platform/`。
- `crates/zant-tsx/`：TSX 编译器（lexer / parser / hooks / codegen / tags / d.ts）。
- `crates/zant-dylib/`：热刷新共享的 Rust 框架；`crates/zant-hot-host/`：持有窗口、替换应用库的开发宿主。
- `crates/zant-cli/`：`zant` 命令；`crates/zant-build/`：应用 `build.rs` 辅助。
- `crates/zant/src/tsx/`：TSX 生成代码的运行时（hooks、字符串、内置、定时器、窗口）。
- `crates/zant/src/core/keys.rs`、`crates/zant/src/core/paths.rs`：平台快捷键与应用数据目录。
- `crates/zant/src/db/`：SQLite 封装（rusqlite；`#[derive(Table)]` 对象集合）。
- `scripts/win/`：Windows 测试机同步、远程执行与桌面冒烟测试。
- `scripts/tsx/node-parity.sh`：TSX 编译后的运行输出与 Node 逐行对照（探针在 `crates/zant-tsx/tests/node_parity/*.ts`，各导出 `run()`）；新增或修改语言特性时补探针并运行。
- `scripts/bench.sh`：帧基准闸门（内置记录基线）；`scripts/tsx/`：TSX 构建、示例与基准检查；
  `scripts/lib/imgdiff.py`：PNG 解码与像素对比。
