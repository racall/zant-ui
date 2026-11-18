# 性能优化方案（2026-11-18）

由 Claude（Opus 5.5）、Codex（gpt-6.2-astra）、Fable 5.2 各自独立读代码出方案，再合并而成。
数据来自 2026-11-18 的两平台实测（Release，`ZANT_FRAME_STATS=1`；Mac M3 Max，Windows i7-13700KF + D3D12，
150% 缩放，均为 60 Hz）。标「已证实」的根因已对照代码核实，标「待证实」的要先按「确认」一栏做实验。

## 现状

| 编号 | 问题 | 现在 | 目标 |
|---|---|---|---|
| A | Windows 页面有持续动画时拖动缩放 | 新尺寸帧 ≈ 42/s，延迟 ≈ 15 ms（07804f6 之前 ≈ 15/s、32 ms） | ≥ 55/s，延迟 ≤ 1 vsync |
| B | 流式 Markdown 单帧耗时随回复长度线性增长 | 60 KB 时 Mac ≈ 14 ms、Windows 13–16 ms；首帧 164–307 ms | 不随长度增长，≤ 6 ms |
| C | Windows 拖标题栏移动窗口时流式内容停止刷新 | 移动期间 3.6–4.3 s 无帧，松手后首帧 33–766 ms | 移动中 ≥ 55 fps，无积压帧 |
| D | Windows TSX gallery 预热阶段私有内存多涨 | 178 → 257 MB 后稳定（Mac 111 → 133 MB） | 找到归属，能省则省 |
| E | 自动化测试下偶发 150–180 ms 慢帧 | rebuild + layout + paint < 2 ms，其余时间未计时 | 消除，或证明与产品无关 |
| F | Windows 上 bench-tsx 闸门不稳 | 同一二进制三次：2 次 FAIL、1 次 PASS | 10 连跑结论一致，灵敏度不降 |

## 三方共识与分歧

- **共识**：
  - 先补测量，再动刀：A、C、E 的剩余根因都在未计时或未记录来源的地方。
  - A 和 C 共用 Windows 移动/缩放模态循环，应该一起设计成统一的出帧节奏（pacing），不要各打补丁。
  - B 的大头在每帧 O(全文) 的工作，必须改成增量处理。
- **分歧与取舍**：
  - **顺序**：Codex 主张 C → A 优先；Fable 主张先做测量和 B。
    - 取舍：测量地基先行（成本低）；随后做已证实、改动小的 B1；再做 A+C 的 pacing。
  - **A 的指标**：Fable 指出统计口径（只计新尺寸帧 ≥ 30 的整秒，混入了钳制和截图时段）会低估。
    - 核对后确认：即使只看真实拖动的整秒，也只有 39–48 帧，差距真实存在。口径要修，问题也要修。
  - **B 的绘制裁剪、布局缓存、首帧从尾部构建**：三方都认为收益高、风险也高，放最后单独验收。

## 阶段 0：测量地基（低成本，先做）

| 任务 | 内容 | 文件 |
|---|---|---|
| M1 | `[slow-frame]` 增加分段：tick / view_state / commands / focus / layout_file，以及 `rebuilds=`、`layout_passes=` | `runtime/frame.rs`、`runtime/debug.rs` |
| M2 | `[frames]` 增加 `resize_events`（收到的尺寸变化数），`resize-check.ps1` 只统计 `resize_events ≥ 55` 的秒 | `runtime/debug.rs`、`runtime/dispatch/mod.rs`、`scripts/win/resize-check.ps1` |
| M3 | Windows 帧来源环形记录（SIZE / PAINT / TIMER / WAKE / 其他 + 像素尺寸 + 等 waitable 的耗时），`WM_EXITSIZEMOVE` 时输出一行汇总；移动期间统计 WAKE、PAINT、TIMER、WM_MOVE 次数 | `window_render.c`、`window_events.c`、`window_internal.h` |
| M4 | Markdown 流水线分段计时：remend / split / parse / view，以及布局、绘制访问的节点数 | `markdown/cache.rs`、`runtime/debug.rs` |
| E1 | 布局文件改为后台线程写：只在内容变化时写，只保留最新一份，写临时文件再替换。顺带证实或排除 E | 新 `runtime/layout_file.rs`、`runtime/frame.rs` |
| F1 | bench 统计改造：比较中位数、两侧成对交替、亚毫秒行自适应补足样本到 ≥ 300、加 A/A 对照（手写对手写）量噪声；判定为「比值 > 1.2 且差值超过 A/A 噪声」。注入 30% 减速确认 10 次都判 FAIL | `zant_tsx_frame_bench/mod.rs`、`suites.rs`、`scripts/tsx/bench-tsx.sh` |

**验收**：两平台跑一遍 `resize-check` / `stream-resize-check` / `zant-md-stream` 60 KB，得到 A、B、C 各自的
帧来源与分段数据，并写回本文档。E1 之后 `components-check.ps1` 40 轮不再出现「三段 < 2 ms 却 > 16.7 ms」的帧。

## 阶段 1：B1 —— 列表项高度变化不再整树重建（已证实）

- **根因**（已证实）：
  - 不定高的 `List` 项都挂了空的 `on_resize`（`components/list/build.rs:243-257`）。
  - 流式回复每次长高都会触发 `notify_resizes`，在同一帧里多做一次整个应用 rebuild 加重新布局，最多 3 遍（`runtime/frame.rs:83-97`）。
  - 这和 07804f6 修掉的 Markdown 问题是同一种模式。
- **改法**（通用）：
  - 尺寸回调返回「是否需要重建」（沿用 `ResizeStep` 的思路）。
  - List 在回调里直接记录项高并修正锚点；跟随尾部时内容列按底部锚定，长高不必重建。
  - 只有可见范围真的变化（需要建新项）时才重建。
- **文件**：`runtime/frame.rs`、`components/list/build.rs`、`components/list/state.rs`。
- **验收**：
  - M1 的 `rebuilds=` 在流式帧上为 1。
  - `zant-md-stream` 60 KB 时最慢帧明显下降。
  - `bench.sh` 的 stream / open / scroll（两种模式）不退步。

## 阶段 2：Windows 统一出帧节奏（A + C）

- **根因**（待 M3 证实）：
  - 模态循环里，出帧依赖排队的 WM_PAINT 和 `ZANT_WM_WAKE`，同尺寸的帧仍会插进来。
  - 每帧都在 frame-latency waitable 上等到 vsync 再 `Present(1)`（`skia_bridge_win.cpp:205,220`），任何一帧同尺寸的帧都会让下一帧新尺寸晚一拍。
  - 流式不算动画，模态计时器不补帧；`wake_pending` 置位后，后续的 wake 都会被合并（`window_lifecycle.c:235`）。
- **改法**：新增 `window_pacing.c`，作为统一的出帧入口：
  - 用 `frame_pending` 区分「有新内容」和「有新尺寸」。
  - `in_size_move` 时 `zant_request` 只置标记，不 `InvalidateRect`。
  - 在 `WM_SIZE`（新尺寸，立即出帧）、`WM_MOVE`、`ZANT_TIMER_MODAL`、`ZANT_WM_WAKE` 这几个触发点上，消化待处理的 wake，每个刷新周期最多出一帧。
  - 只有最近两个刷新周期没有尺寸变化时，才补非尺寸帧（替换现在写死的 50 ms）。
  - GPU 路径的 WM_PAINT 没有新内容时只做 Begin/EndPaint，flip 交换链会保留画面。
- **风险**：
  - 菜单、对话框等其他模态循环，以及遮挡恢复。需要回归 `components-check.ps1`、`smoke`、`desktop-checks.ps1`。
  - macOS 用 display link 出帧，不受影响，只共享帧统计字段。
- **任务**：
  - P1 `window_pacing.c` + `sources-windows.txt`
  - P2 `window_events.c` 接入
  - P3 `window_render.c`（WM_PAINT 空帧）
- **验收**：
  - 动效开、关两种情况下，`resize-check.ps1` 新尺寸帧 ≥ 55/s，延迟 < 20 ms。
  - `stream-resize-check.ps1` 移动期间 fps ≥ 55，第一秒之后最慢帧 < 16.7 ms。
  - Mac 用同样轨迹复跑。

## 阶段 0 + B1 结果（2026-11-18，已完成）

**做了什么**
- M1、M2：`[slow-frame]` 增加 `other` / `rebuilds` / `layouts` / `md_ms`，`[frames]` 增加 `resize_events`。
- M3：Windows `ZANT_FRAME_TRACE=1` 输出 `[frame-trace]`（Codex）。
- M4：Markdown 流水线计时（`markdown/timing.rs`）。
- E1：布局文件改由后台线程写（`runtime/layout_file.rs`）。
- F1：bench-tsx 改为成对轮次比中位数，加手写对手写的对照组（Codex）。
- B1：
  - 新增通用的 `ActionCx::skip_rebuild` / `defer_rebuild`，`on_resize` 回调可以声明不重建，或下一帧再重建；
  - 列表项高度变化由 `list/resize.rs` 判断是否需要重建；
  - 跟随尾部时内容列按底部定位。

**实测**
- **B1（Mac，60 KB 回复 + 30 条历史）**：
  - 末段每秒最慢帧 ≈ 14 → ≈ 9.5 ms，layout 7 → 1.8–4.4 ms；
  - 超过 8 ms 的帧 403 → 268；
  - 流式帧 `rebuilds=1`。
  - 剩余：rebuild ≈ 5 ms，其中 remend 对全文一遍就 1.3–2.6 ms；paint ≈ 3 ms。对应阶段 3。
- **E1（Windows）**：`components-check.ps1` 慢帧 0 个（之前偶发 150–180 ms）。
- **A 的追踪**（Windows，zant-components，7.3 s 拖动）：
  - 共 423 帧，其中新尺寸 182 帧、同尺寸 241 帧；同尺寸帧来自模态计时器 155 帧、WM_PAINT 86 帧（由 178 次 wake 触发）。
  - 每帧都等一个 vsync（`begin_us` 平均 6.3 ms，`render_us` 平均 10.3 ms），所以 `WM_SIZE` 只有约 25–40 次/秒，`resize_events` 和 `resize_frames` 相等。
  - 结论：同尺寸帧占了 57% 的 vsync，阶段 2 的 pacing 要合并 wake 和计时器帧。
- **C 的追踪**（Windows，md-stream 拖标题栏 2.4 s）：
  - `move_msgs=120`、`wakes=0`、0 帧：移动期间窗口过程一次 wake 都没收到。
  - 下一步在投递端（`zant_window_post_wake`）计数，确认工作线程这段时间是否调用、`wake_pending` 是否卡在 1。
  - 松手后一帧 rebuild 33–55 ms，但 `md_ms` 只有 0.2 ms，慢在 rebuild 的其他部分，待阶段 3 定位。
- **F1**：Mac 上 A/A 噪声 ≤ 0.03，注入 1.3 倍减速时 6 行全部判 FAIL。
  - Windows 上同机 A/A 单轮偏差 0.1–0.8，仍会偶发单行超标（一次 `local-mount` 1.33 倍，重跑 PASS）。
  - 需要 F2：固定到性能核、提高优先级。
- **闸门**：
  - 两平台 `cargo test` 115/115、clippy 0；
  - Mac `bench.sh` 两种模式 7/7、`bench-tsx.sh` 11/11、soak 0 泄漏、组件真实点击 473/473；
  - Windows 组件真实点击 41/41、帧基准稳态 < 5 ms、soak 0 泄漏。

## 阶段 3：B2 + 绘制裁剪 —— 流式 O(全文) 变 O(尾部)

- **根因**（已证实存在，各自占比待 M4 确认）：
  - 每帧对全文做 remend，并新建 String（`markdown/cache.rs:93-94`）。
  - Splitter 每帧切分全文并克隆所有块（`markdown/blocks.rs:352-378`）。
  - 每帧逐块比较前缀、后缀（`cache.rs:107-119`）。
  - 每块都要拼 key、建 local_state、建闭包（`render/view.rs:191-229`）。
  - 示例每次 build 都拷贝缓冲（`stream_app.rs:349-352`）。
  - 绘制只裁剪 scroll 容器的直接子节点，List 内容和 Markdown 块不裁剪；每个容器还 `children.clone()`（`core/tree/paint.rs:233-246`）。
- **改法**：
  - B2a：Splitter 记住已经稳定的块边界，只切分尾部。
  - B2b：Cache 只对尾部做 remend，块原文用 `Rc<str>`，按稳定块数跳过比较。
    - 引用定义、脚注、任意位置编辑、选项变化时回退全量。
  - B2c：view 层复用稳定块的 key 和节点。
  - B3b：绘制沿递归传裁剪矩形，剔除不相交的子树（溢出、阴影、动画节点除外）；去掉 `children.clone()`。
- **风险**：分窗 remend 可能偏离上游的全文奇偶计数。
  - 对每个前缀做「增量 vs 全量」差分测试。
  - remend 522 条、mhchem 823 条夹具不动。
- **验收**：
  - `zant-md-stream` 60 KB 最慢帧 ≤ 6 ms，且不随长度增长。
  - `bench.sh`、`bench-tsx.sh` markdown 行、soak，两平台都过。

## 阶段 4：高收益高风险，单独验收

- **B3a 布局缓存**：节点干净且约束相同，就复用尺寸；位置变了只平移（`core/layout/container.rs`）。
  - 需要和全量布局做对照测试，并依赖脏标记完整。
- **B4 首帧**：跟随尾部时从最后一项往前构建。
  - Fable 推算：首帧 4530 个文字排版来自按 120 px 估高定锚点，整排了 3 条约 60 KB 的长回复，而只有最后一屏可见（`list/build.rs:129-191`）。
  - 超长单条消息的块级惰性排版（类似 content-visibility）放在之后。
- **D 内存**：
  - 先确认归属：用 `ZANT_RENDERER=raster` 跑同样 40 轮；新增 `zant_renderer_cache_bytes`，在 `[frames]` 里打印 GPU 缓存大小。
  - 若是 Ganesh 资源缓存（Modal 遮罩和面板淡入淡出走整窗离屏层，`MakeD3D` 用默认选项，没设上限也不定期清理），就在 `skia_bridge_renderer.cpp` 统一策略：
    - 按后备缓冲大小设缓存上限；
    - 每秒 `performDeferredCleanup`；
    - 窗口隐藏或最小化时清理未锁定资源。
  - 两平台共用这套策略，所以要过 Mac 的基准，检查重新打开 Modal 时的帧时间。

## 优先级与分工

| 顺序 | 项 | 收益 | 成本 | 风险 |
|---|---|---|---|---|
| 1 | 阶段 0（M1–M4、E1、F1） | 测量可信，定位 A / B / C 的剩余根因 | 低 | 低 |
| 2 | B1 | 两平台流式帧时间显著下降 | 低 | 低 |
| 3 | A + C pacing | Windows 移动 / 缩放达标 | 中 | 中（仅 Windows 模态循环） |
| 4 | B2 + 绘制裁剪 | 流式帧时间不随长度增长 | 中 | 中（上游行为差异） |
| 5 | D | 少几十 MB 常驻 | 低 | 中（两端缓存策略） |
| 6 | B3a、B4 | 长文档布局与首帧 | 高 | 高（布局正确性） |

分工沿用现有方式：
- Claude 负责核心改动（B1、pacing、B2）、全部测试（含 Windows 真实桌面）、审查和提交。
- Codex 一次接一个小任务（M1、M2、M4、E1、F1、D 的统计 ABI），每个 1–3 个文件。
- 每个阶段结束时，两平台都要过质量门：`cargo test`、clippy、`bench.sh`、`bench-tsx.sh`、soak，以及对应的桌面脚本。
- 只合入有可测收益的改动，前后数据回填本文档。
