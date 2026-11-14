# Rust 迁移进度（status）

> 每个执行者领取 / 完成 / 阻塞时更新自己那一行。状态取值：`待领取` / `进行中` / `完成` / `阻塞` / `待验证`。
> 分支：并行期间所有 WP 直接提交到 `feat/rust-port`（多人共用同一工作区，无法为每个 WP 开独立分支），
> 提交信息带 `WP-x.y: ` 前缀，由 lead 审核后提交。

## 当前快照（最近更新）

| 项 | 值 |
|---|---|
| 分支 | `feat/rust-port`（全部 WP 直接合入这里，提交带 `WP-x.y:` 前缀） |
| Rust 测试 | `cargo test --workspace` 全绿（`rust/wp-coverage`）：**74 个测试套件 / 1076 passed / 0 failed**（35 条 doc 示例标注 `ignore`），含 31 个 TSX golden 的批量 `cargo check`、端到端 fixture/模板；WP-COVERAGE 新增 17 条（`Uint8Array`/desktop 9 + store 诊断 1 + string 2 + React 兼容 5），并解除 `desktop` 黄金阻塞（见下方「WP-COVERAGE」） |
| Zig 测试 | `zig build test` 仍全绿（Zig 版未改行为，只加了 `ZANT_WINDOW_SIZE` 钩子与 `ZANT_STATIC_DATA` 原生补丁）；WP-COVERAGE 未改任何 `.zig`（`git status -- '*.zig'` 为空） |
| 质量门 | `cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets -- -D warnings` 0 告警 |
| 已完成 | M0 全部（0.1 macOS/0.2/0.3/0.4/0.5）、M1 全部（1.1a–d/1.2/1.3/1.4/1.5，parity 6/6 0 像素）、M2（2.1–2.11 除 2.11 已并入全绿）、M3（3.1–3.9、3.11，zant-pdf parity 6/6）、M4（4.1–4.4）、M5（5.1–5.8、5.9 黄金/端到端，见下方偏差表）、M6（6.1、6.3、6.4、6.5）、M7（7.3 文档）、WP-5.10（TSX 示例 + 编译基准）、**WP-COVERAGE（desktop 黄金 + 测试覆盖对照，见下方「WP-COVERAGE」）** |
| 进行中 | —（WP-5.10 / WP-COVERAGE 已完成；待 lead 合并各 worktree 并做全量验收） |
| 下一步 | lead 合并 → 全量验收（macOS + Windows）→ 补齐 WP-5.9 / WP-5.10 报告的框架 API 与编译器 Bug（`ui` 的 markdown/pdf_viewer 导出、跨文件窗口组件、生成代码 lint 兜底）→ 更新 `docs/tsx.md` 的 `Uint8Array` 说明（`&'static [u8]` vs 运行时 `tsx::Bytes`）→ TSX 帧对比基准（TSX 生成代码 ≤1.2× 手写 Rust）→ M8 切换（需用户确认） |

| Rust 测试 | `cargo test --workspace` 全绿（`rust/wp-5.10b`）：**75 个测试二进制 / 1060 passed / 0 failed**，含 WP-5.10 新增 6 条（3 × 示例 native `system`、3 条 `zant-tsx-bench`）与 WP-5.10b 新增 1 条（notes native `plural`）；工作区全量以 lead 合并后为准 |
| Zig 测试 | `zig build test` 仍全绿（Zig 版未改行为，只加了 `ZANT_WINDOW_SIZE` 钩子与 `ZANT_STATIC_DATA` 原生补丁） |
| 质量门 | `cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets -- -D warnings` 0 告警 |
| 已完成 | M0 全部（0.1 macOS/0.2/0.3/0.4/0.5）、M1 全部（1.1a–d/1.2/1.3/1.4/1.5，parity 6/6 0 像素）、M2（2.1–2.11 除 2.11 已并入全绿）、M3（3.1–3.9、3.11，zant-pdf parity 6/6）、M4（4.1–4.4）、M5（5.1–5.8、5.9 黄金/端到端，见下方偏差表）、M6（6.1、6.3、6.4、6.5）、M7（7.3 文档）、WP-5.10（TSX 示例 + 编译基准）、**WP-5.10b（M5 剩余的 gallery/todos/reader/notes 示例 + Zig 对照 + 真实 GUI 场景，见下方「WP-5.10b 完成情况」）** |
| 进行中 | —（WP-5.10 / WP-5.10b 已完成；待 lead 合并各 worktree 并做全量验收） |
| 下一步 | lead 合并 → 全量验收（macOS + Windows）→ 补齐 WP-5.9 / WP-5.10 / WP-5.10b 报告的框架 API 与编译器 bug（`ui` 的 markdown/pdf_viewer 导出、跨文件窗口组件、`Uint8Array` 映射、生成代码 lint 兜底、子组件回调 prop 被 `ui::effect` 吞掉、`Option<Node>` 裸元素、富文本末尾字面量）→ TSX 帧对比基准（TSX 生成代码 ≤1.2× 手写 Rust）→ M8 切换（需用户确认） |

| Rust 测试 | `cargo test --workspace` 全绿（`rust/wp-5.10c`）：**72 个测试二进制 / 1066 passed / 0 failed**，含 WP-5.10 的 1059 条 + WP-5.10c 新增 7 条（帧基准统计 / 参数 / 生成树结构对照）；工作区全量以 lead 合并后为准 |
| Zig 测试 | `zig build test` 仍全绿（Zig 版未改行为，只加了 `ZANT_WINDOW_SIZE` 钩子与 `ZANT_STATIC_DATA` 原生补丁） |
| 质量门 | `cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets -- -D warnings` 0 告警 |
| 已完成 | M0 全部（0.1 macOS/0.2/0.3/0.4/0.5）、M1 全部（1.1a–d/1.2/1.3/1.4/1.5，parity 6/6 0 像素）、M2（2.1–2.11 除 2.11 已并入全绿）、M3（3.1–3.9、3.11，zant-pdf parity 6/6）、M4（4.1–4.4）、M5（5.1–5.8、5.9 黄金/端到端、5.10 TSX 示例/编译基准、5.10c 帧对比基准，见下方完成情况）、M6（6.1、6.3、6.4、6.5）、M7（7.3 文档）、**WP-5.10c（TSX 帧基准 ≤1.2× 验收，见下方「WP-5.10c 完成情况」）** |
| 进行中 | —（WP-5.10 / WP-5.10c 已完成；待 lead 合并各 worktree 并做全量验收） |
| 下一步 | lead 合并 → 全量验收（macOS + Windows）→ 补齐 WP-5.9 / WP-5.10 / WP-5.10c 报告的框架 API 与编译器 bug（`ui` 的 markdown/pdf_viewer 导出、跨文件窗口组件、`Uint8Array` 映射、非根 `<List>` 生成代码、生成代码 lint 兜底）→ M8 切换（需用户确认） |
| Windows | 测试机 192.168.31.24 全程不可达；所有 Windows 验收待补 |

| Rust 测试 | `cargo test --workspace` 全绿（`rust/wp-tsx-bugs`）：**81 个测试结果（含 doc-tests）/ 1092 passed / 0 failed**，含 WP-TSX-BUGS 新增 7 条（`crates/zant-tsx/tests/compile_regressions.rs` 6 条编译器回归 + `crates/zant/tests/tsx_callbacks_test.rs` 1 条同步回调运行期用例）；工作区全量以 lead 合并后为准 |
| Zig 测试 | 本 WP 未改任何 `.zig`（Zig 树冻结，未跑 `zig build test`） |
| 质量门 | `cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets -- -D warnings` 0 告警 |
| 已完成 | **WP-TSX-BUGS（`zant-tsx` 四个已验证代码生成 bug：子组件回调 prop 同步执行、`Option<Node>` 裸元素、富文本末尾字面量、非根 `<List>`；gallery/todos/reader/notes 与 Zig 基线 0 px，见下方「WP-TSX-BUGS 完成情况」）** |
| 进行中 | —（WP-TSX-BUGS 已完成；待 lead 合并并做全量验收） |
| 下一步 | lead 合并 → 复跑 `examples-check`（gallery 0 px）/ `bench-tsx` / `parity`；Windows 验收待补 |

## 环境与基线（M0 记录）

| 项 | 值 |
|---|---|
| 迁移起点 | `feat/tsx-compiler` HEAD `905955f`（本仓库即该提交） |
| 迁移分支 | `feat/rust-port` |
| Zig | 0.17.1（`/opt/homebrew/bin/zig`） |
| Rust | stable 1.94.1，edition 2024；`rust-toolchain.toml` 用 `channel = "stable"`（沙箱内 rustup 不能装精确版本，见 conventions.md） |
| 基线机器 | macOS 27.2.1，arm64（Apple Silicon） |
| `zig build test`（macOS arm64） | **通过**：6 个测试二进制，共 **380** 个测试（tests 聚合 124、框架单元 99、zant CLI 8、TSX 编译器 149）。文档里旧的「164/164」是过时数字 |
| ReleaseFast 基线 | `zig-out-baseline/bin/`（`zant-demo`、`zant-components`、`zant-gallery`、`zant-markdown`、`zant-math`、`zant-pdf`、`zant-md-stream`、`zant-bench`、`zant`） |
| Windows 基线 | **待验证**：测试机 `192.168.31.24` 当前不可达（No route to host）。恢复后要补：`zig build test`、`build_bridge.ps1`、rustup / VS Build Tools / 1.94.1 工具链检查 |
| 沙箱注意 | `cargo` 一律通过 `./scripts/cargo.sh`（CARGO_HOME 指向 `.cargo-home/`）；Zig 命令带 `ZIG_GLOBAL_CACHE_DIR=$PWD/.zig-global-cache` |

## 性能基线（WP-2.11，`zant-bench`）

`scripts/parity/bench.sh [scenario...]`：Rust Release（`target/release/zant-bench`）与 Zig 基线
（`ZANT_ZIG_BIN=<主 checkout>/zig-out-baseline/bin`）跑同一矩阵（默认 30 条消息；场景
open / stream / typing / caret / hover / resize / scroll）。默认同时跑两种渲染模式：`default`
（全量重建）与 `list`（`ZANT_BENCH_LIST=1` 虚拟列表）。每边跑 `ZANT_BENCH_REPEAT`（默认 3，本表用
8）轮、交替先后，每场景取最好一轮，输出 avg / p95 / max 与 Rust/Zig 比值；闸门按 plan.md §4/§7.3：
每场景 avg ≤ 1.25× Zig，稳态 avg ≤ 16.7 ms（`open` 是冷启动首帧，Zig 基线自己就 ~120 ms 全量重建
/ ~21 ms 虚拟列表，因此只判比值）。原始日志在 `target/parity/bench/`。

- 机器：macOS 27.2.1 arm64（Apple Silicon），14 核。测量期间 load average 6–11（多个并行 worktree
  同时跑 cargo/zig 构建），单轮波动明显，故用 best-of-8；同一脚本可在安静机器上用
  `ZANT_BENCH_REPEAT=1` 复跑。
- Zig 基线：`/Users/xiaoyi/dev/XyUI/zig-out-baseline/bin/zant-bench`（Zig 0.17.1 ReleaseFast，WP-0.1）。
- 命令：`./scripts/parity/bench.sh`（构建 Rust 后跑全部）；`ZANT_BENCH_MODES=default ./scripts/parity/bench.sh --no-build stream` 等。

### default（全量重建，30 条 ~2.4 KB Markdown 消息 + 流式回复）

| 场景 | frames | avg rust | avg zig | ratio | p95 rust | max rust | p95 zig | max zig | 闸门 |
|---|---|---|---|---|---|---|---|---|---|
| open | 5 | 122.12 ms | 120.88 ms | 1.01× | 146.17 | 146.17 | 143.71 | 143.71 | PASS*（冷启动，只判比值） |
| stream | 186 | 2.78 ms | 2.24 ms | **1.24×** | 3.27 | 3.54 | 2.82 | 3.21 | PASS |
| typing | 200 | 0.98 ms | 0.84 ms | 1.17× | 1.20 | 1.34 | 1.04 | 1.12 | PASS |
| caret | 100 | 0.65 ms | 0.67 ms | 0.97× | 0.80 | 0.84 | 0.79 | 0.83 | PASS |
| hover | 300 | 0.35 ms | 0.35 ms | 1.00× | 0.40 | 0.60 | 0.43 | 0.54 | PASS |
| resize | 120 | 5.05 ms | 4.69 ms | 1.08× | 5.24 | 83.17 | 4.69 | 82.64 | PASS |
| scroll | 300 | 0.72 ms | 0.75 ms | 0.96× | 0.96 | 1.35 | 1.01 | 1.29 | PASS |

### virtual list（`ZANT_BENCH_LIST=1`）

| 场景 | frames | avg rust | avg zig | ratio | p95 rust | max rust | p95 zig | max zig | 闸门 |
|---|---|---|---|---|---|---|---|---|---|
| open | 5 | 21.84 ms | 20.98 ms | 1.04× | 43.19 | 43.19 | 41.48 | 41.48 | PASS*（冷启动，只判比值） |
| stream | 186 | 0.89 ms | 0.85 ms | 1.05× | 1.16 | 4.55 | 1.09 | 4.50 | PASS |
| typing | 200 | 0.70 ms | 0.69 ms | 1.01× | 0.92 | 1.26 | 0.90 | 1.06 | PASS |
| caret | 100 | 0.65 ms | 0.64 ms | 1.02× | 0.75 | 0.96 | 0.77 | 0.88 | PASS |
| hover | 300 | 0.35 ms | 0.35 ms | 1.00× | 0.43 | 0.57 | 0.39 | 0.55 | PASS |
| resize | 120 | 0.75 ms | 0.72 ms | 1.04× | 0.93 | 4.18 | 0.93 | 3.98 | PASS |
| scroll | 300 | 0.63 ms | 0.62 ms | 1.02× | 0.74 | 0.85 | 0.74 | 0.90 | PASS |

**结论：两模式 7/7 场景 ≤1.25× Zig，稳态帧远低于 16.7 ms（全部 <6 ms，60 fps 余量充足）；
闸门通过，但 `stream`（全量重建）余量很薄（1.24× vs 1.25×），建议合入前应用下面的框架补丁。**
`resize` 的 max 83 ms 是首帧从 900→700 px 的冷布局（Zig 82.6 ms，非回归）；`open` 是 30 条消息
的首次整篇解析 / 排版（Zig 同量级）。

#### `stream` 热点（报告给 lead 的框架 patch，未提交）

`ZANT_FRAME_STATS=1` + 分相计时（`core::layout` 4 项拆开）显示 30 条消息时单帧 ≈ rebuild 0.40 ms +
**layout 1.69 ms** + `refresh_key_bounds` 0.16 ms + paint 0.70 ms；layout 遍历 **10,922** 个节点。
根因：Rust 版 `core/layout` 在**每个容器**上克隆子节点列表（Zig 直接迭代 `node.children.items`）：

- `crates/zant/src/core/layout/intrinsic.rs::intrinsic_width`（容器分支，~114 行）与
  `intrinsic_height`（~205 行）各 `let children: Vec<NodeId> = nodes[id].children.clone();`；
- `crates/zant/src/core/layout/container.rs::layout_container`（~205 行）同样 clone。

把这三处改成 `let child_count = nodes[id].children.len(); for i in 0..child_count { let child = nodes[id].children[i]; ... }`
（行为不变）后实测：`layout` 1.69 → 1.41 ms，stream 2.78 → 2.71 ms（ratio 1.24 → 1.20；另一次
patched 全矩阵 stream 1.08×），`cargo test -p zant` 523 lib + 全部集成测试通过。补丁在
`target/wp-2.11-hotpath-proposal.patch`（框架文件归 lead / 后续 WP，本 WP 未改）。次要：
`Runtime::refresh_key_bounds` 每次布局重建 `HashMap<String, Rect>`（2,961 个 key、~0.16 ms/帧，
Zig 直接读活树），可改惰性重建或复用节点已有的 `Str`。

### `zant-md-stream` 流式（plan §7.3）

Release、窗口模式（Metal），`ZANT_FRAME_STATS=1 ZANT_STREAM_CHARS=6 ZANT_STREAM_MS=16 ZANT_STREAM_EXIT=1`，
内置长回复（1990 bytes / 322 deltas）：

| 历史 | 实现 | 挂载帧 | 挂载 text_layouts | 之后 [slow-frame] | 流式最大帧 | build avg/max |
|---|---|---|---|---|---|---|
| 8 | Rust | 32.49 ms (layout 31.84) | 244 | 无 | 1.16 / 1.14 / 2.10 / 2.51 / 3.17 ms | 0.07 / 0.24 ms |
| 8 | Zig | 36.69 ms (layout 34.95) | 244 | 无 | 3.09 / 2.49 / 5.02 / 2.69 / 2.77 ms | 0.06 / 0.43 ms |
| 16 | Rust | 31.35 ms (layout 30.92) | 244 | 无 | 1.09 / 1.53 / 3.15 / 2.84 / 2.91 ms | 0.09 / 0.28 ms |
| 16 | Zig | 28.69 ms (layout 28.21) | 244 | 无 | 1.97 / 2.34 / 2.02 / 1.19 / 1.08 ms | 0.04 / 0.15 ms |

结论：历史 8→16 内容翻倍，`text_layouts` 仍是 244（不随内容增长）；只有挂载首帧超过 8 ms，
流式期间没有 `[slow-frame]` 且没有增长，满足 plan §7.3；Rust 挂载帧与流式帧都不差于 Zig。

## 工作包进度

### M0 基线与地基

| WP | 内容 | 负责人 | 状态 | 分支 | 备注 |
|---|---|---|---|---|---|
| 0.1 | 建分支、两平台基线测试、ReleaseFast 基线、Windows 机器确认 | lead | 完成（macOS）/待验证（Windows） | feat/rust-port | macOS 基线见上表；Windows 机器不可达，恢复后补 `zig build test` 与工具链检查 |
| 0.2 | Cargo workspace、rust-toolchain、空 crate 骨架、.gitignore、CI rust.yml | lead | 完成（macOS）/待验证（Windows） | feat/rust-port | `cargo build --workspace` 通过；模块树桩文件已预置（子模块声明齐全，并行 WP 不必抢改 mod.rs） |
| 0.3 | `zant-sys`：bindgen 签入 + build.rs 编原生代码 / 链 Skia | lead | 完成（macOS）/待验证（Windows） | feat/rust-port | bindgen 0.73.2 生成 56/50/12 个函数并签入；`cargo test -p zant-sys` 冒烟 2/2 通过（raster 画矩形读像素、TextLayout 量宽度）；Windows 静态数据 `ZANT_STATIC_DATA` 补丁已加但未在 Windows 上验证 |
| 0.4 | `scripts/parity/` 对照工具（capture/compare） | lead | 完成 | feat/rust-port | `capture.sh` + `compare.sh` + `layoutdiff.py` + `imgdiff.py`（纯标准库，无 Pillow 依赖）；Zig 自己对自己：zant-demo 浅/深色 × 600x620/900x640/1200x800 共 6 组，布局 8 个 key 一致、像素 0 差异 |
| 0.5 | `docs/rust-port/{plan,conventions,api,status}.md` | lead | 完成 | feat/rust-port | 另有 `agent-brief.md`（并行执行者工作说明） |

### M1 核心骨架

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 1.1a | geometry、theme、motion、layout/flex + 测试 | wp-1-1a | 完成（已并入） | 17 个测试全过；flex/motion 的 bit-exact 对照 658 行；`tests/flex_test.zig` 24 个 Tree 级用例归 WP-2.10 |
| 1.1b | core/{unicode,paths,hash} | wp-1-1b | 完成（已并入） | 9 个测试；hash 是 Zig Wyhash 4.2 的逐行移植并钉死 20 个 Zig 实测值；keys/text_select 改派 lead |
| 1.1c | renderer/{canvas,text,rich_text,renderer}、image | lead | 完成 | 依赖 0.3；14 个测试（renderer raster/像素、TextLayout 度量/换行/relayout/caret、rich text、image PNG/缩略图/data URI）；`Image` 是唯一 `unsafe impl Send + Sync` 的句柄 |
| 1.1d | platform/{platform,dialog}、core/events、core/keys | lead | 完成 | `Host` 回调表 → `Box<dyn HostHandler>` 蹦床；`ZANT_WINDOW_SIZE` 已照搬；`monotonic_us` 用 `Instant`（deviation）；事件 text 用 owned `String`；14 个测试 |
| 1.2 | actions、ui Element / 选项 / 构建上下文 | wp-1-2 | 完成（已并入） | 39 个新测试；52 字段 Element + 32 个选项结构体逐一对照 Zig；只做 column/row/text/button/styled，其余选项类型已声明 |
| 1.3 | node、tree、layout、components/button、text_select | wp-1-3 | 完成（已并入） | 24 个 flex_test + 25 个新单测；`Tree`/`RetainedNode`/`NodeData`/text 缓存/命中/滚动/焦点/输入编辑；Icon 重复已去重；rich text 选择高亮待接（range_rects 已补） |
| 1.4 | ui_runtime 构建/协调/patch/事件/帧循环、launch、App trait | wp-1-4 | 完成（worktree wp-1.4，待并入） | ★★★；`App`/`RunOptions`/`run()` + `runtime/` 15 个模块；51 个新测试（ui_runtime.zig / launch.zig 无 test 块，全部为新增 headless 测试；zant lib 测试 210 → 261）；Zig 环境变量除 `ZANT_AUTOPLAY`（Zig runtime 不读）与 `ZANT_SNAPSHOT_EXIT`（原生处理）外全部覆盖；多窗口 / caption 按钮 / select 弹层 / input·toggle·select 组件动画留给 WP-2.x；`ui::key_bounds` 用每帧快照而非活树（借用检查） |
| 1.5 | `zant-demo`（= `examples/declarative.zig`，注意不是 src/main.zig） | wp-1-5 | 完成（已并入） | **M1 验收通过**：parity 6/6（light/dark × 3 尺寸），layout 8 keys 一致、像素 0 差异；input 组件 paint 已补 |

### M2 组件与运行时补全

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 2.1 | input + runtime 键盘 / IME / 焦点 / 文本选择（runtime 交互接线收尾） | wp-2-1 | 完成（已并入） | `rust/wp-2.1`；补齐 status.md「已知框架缺口」1–6：toggle/radio/switch 的 BoolAction/IndexAction 派发（pointer click + Space/Enter）、image 路径加载 / 内存源接线、animate tick 覆盖 Input/Toggle/Select、select 弹层 hover/click/键盘 + popup 在 tree paint 之后绘制、rich text 选择高亮、input 助手三处重复合并到 `components/input.rs`。新增 8 个 headless 测试（lib 354，+8）；parity 6/6、0 像素。IME 真机（macOS 拼音）未自动化验证 |
| 2.2 | toggle、select、image + select 弹层、tooltip | wp-2-2 | 完成（已并入），runtime 接线待 1.4 | 32 个测试（含 raster 像素断言）；`Image::draw_rect` 已加；select/tooltip 的浮层生命周期、image load 调用点在 1.4 |
| 2.3 | ui.zig 其余构建器、core/icons | wp-2-3a + wp-2-3b | 完成（已并入） | icons + 30 个 builder（builders/ 拆 8 个文件）；53 个 builder 测试；`ui.icon` 与 Zig 一样生成 Vector 节点（Element 无独立 icon 色/描边字段） |
| 2.4 | memo、localState、portals、effects、commands | wp-2-4（local_state 由 lead 提前完成） | 完成（已并入） | `core/local_state.rs`（Store/Slot/child_id，Rc<RefCell<T>> + TypeId panic，child_id 与 Zig 钉死值一致）+ `ui/local_state` 包装；`ui::{memo,portals,commands}` 与运行时钩子（`memo_hash`/`memo_build`/`resolve_memo`、`take_portals`、`take_commands_for`/`requeue_command`/`set_command_owner`、`set_build_states`）完成；本模块新增 22 个测试 |
| 2.5 | menu、modal、segmented、message、shortcut_list、hotkey_input、view、window_controls | wp-2-5 | 完成（已并入） | 8 个 builder + 选项结构体（含 `TsxProps`）+ 47 个新测试（Zig 0 个 test 块：全部为新增结构 / raster 断言）；`ui::*` 导出待 lead 在 `ui/mod.rs` 加 8 行（见待决问题 4）；`desktop.Shortcut` 子集暂放 `components/hotkey_input.rs`（见待决问题 5） |
| 2.6 | components/list（虚拟列表）+ ListState | — | 待领取 | |
| 2.7 | 动效接入：presence、FLIP、layout_transition、runtime tick | wp-2-7 | 完成（worktree wp-2.7，待并入） | `rust/wp-2.7`。核对结论：presence（enter/exit keep-alive、revive、exit 结束由 tick 摘除并 free）、FLIP（`Tree::layout` 的 `flip_prev`/`flip_from`/`flip_delta` + paint 的旧盒绘制）、`next_frame_delay`、`ZANT_NO_MOTION`/`ZANT_SNAPSHOT`/reduce-motion 开关在基线（1.3/1.4/2.1/2.5）已实现且与 Zig 逐段一致，本 WP 未改生产代码；补齐 `tests/motion_test.zig` 后 6 个 runtime 用例（fake clock，同名同断言）+ 3 个本 WP 用例（`next_frame_delay`、modal/message presence、motion 关闭）。`tests/motion_test.zig` 10 个用例全部有了 Rust 对应（原文件 4 + 新增 6） |
| 2.8 | desktop、store、多窗口 | wp-2-8 | 完成（worktree wp-2.8，待并入） | `desktop/{mod,app,app_event,clipboard,hotkey,tray,window}.rs` + `runtime/window.rs`（共享 `SharedApp`、窗口注册表、`open_window` / 延迟关窗、`invalidateAll`）；`store.rs` 已在基线完成；34 个新测试（lib 346 → 380，全部 headless；真实托盘 / 热键 / 多窗口为手测项）；跨文件小改 3 处：`platform` 安全 app-event 蹦床、`Image::read_rgba`（托盘图标）、`FrameStats::for_window`，见待决问题 4；另 `hotkey_input` 的 `Shortcut` 重复待合并（待决问题 5） |
| 2.9 | examples/{declarative,components,gallery}、gallery.zig、app.zig | — | 待领取 | |
| 2.10 | tests/*_test.zig 对应测试移植 | wp-2-10 | 完成（worktree wp-2.10） | 分支 `rust/wp-2.10`。13 个新文件（`crates/zant/tests/{all,antd_components,component,lifecycle,local_state,mhchem_render,pointer,selection,store,text,text_align,ui,view}_test.rs`），新增 53 个测试；`tests/` 的 Rust 集成测试 72 → 134（含 merge 进来的其他 WP 新用例），全 crate `test -p zant` 595 passed / 0 failed；`clippy -p zant --all-targets -- -D warnings` 通过。Zig 侧非 TSX 测试已全部覆盖：`list_test.zig` 3 条已由 WP-2.6 在 `src/components/list.rs` 内同名移植，`motion_test.zig` 剩余 6 条由 WP-2.7 移植。key-bounds 修复（`cbab28c`）后 `pointer_test.zig` 的 pointer_area 用例已补齐。未移植 1 条：`component_test.zig` 的 gallery 用例（`Zig src/app.zig` 的 gallery 现在是 `zant-examples` 的 `gallery_app`，不是框架 API，不做跨 crate 测试依赖） |
| 2.11 | examples/bench.zig → zant-bench；性能闸门 | wp-2-11 | 完成（worktree wp-2.11，待并入） | 分支 `rust/wp-2.11`。`crates/zant-examples/src/bin/zant-bench.rs`（thin entry）+ bin-local `src/bin/zant_bench/bench_app.rs`（1:1 移植：open / stream / typing / caret / hover / resize / scroll、`ZANT_BENCH_MESSAGES`、`ZANT_BENCH_LIST=1`、相同迭代数/预热/输出格式）。偏差：Zig bench 不调用 `Runtime.run`，因此 `ZANT_FRAME_STATS=1` 在 Zig 里对 bench 无效；Rust 版在 `new_runtime` 里接线（打印每秒 `[frames]` 与 `[slow-frame]` 的 rebuild/layout/paint 分相），默认输出与 Zig 逐字符一致。新增 `scripts/parity/bench.sh [scenario...]`：构建 Rust Release、对 Zig 基线（`ZANT_ZIG_BIN`）跑同一矩阵（默认 + 虚拟列表两模式），每边 `ZANT_BENCH_REPEAT` 轮交替取最好一轮，输出 avg/p95/max + Rust/Zig 比值 + 闸门判定（退出码）。本机 best-of-8：默认重建 7/7 PASS（stream 1.24× 最紧）、虚拟列表 7/7 PASS；`zant-md-stream` 历史 8/16 流式：挂载帧 `text_layouts` 都是 244（不随内容增长）、之后无 `[slow-frame]`。详见下方「性能基线」。bin 内 3 个测试；`cargo test -p zant-examples`、`clippy --all-targets -D warnings` 通过 |

### M3 内容模块

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 3.1 | markdown/remend（522 条夹具） | wp-3-1 | 完成（已并入） | 522/522 夹具通过；5 个 Zig 测试移植 |
| 3.2 | markdown/parser + blocks/cache/html/safety/highlight | wp-3-2 | 完成（已并入） | 23 个内联测试 + 6/12 个 markdown_test（余 6 个需 render/editor → 3.3）；与 Zig 逐字节差分 15.2MB 输出 sha256 一致 |
| 3.3 | markdown/render、editor/edit、widgets 的 markdown 部分 | wp-3-3 | 完成（已并入） | 分支 `rust/wp-3.3`。render/12 文件 + editor/edit/theme + widgets 的 markdown 部分；Zig `edit.zig` 3/3 测试同名移植，`tests/markdown_test.zig` 剩余 6 个渲染/编辑器用例已补，另加 11 个模块单测与 5 个 headless 冒烟（README + 1 万行文档 + memo 跳过）。偏差见执行报告：editor 的模式切换暂用 `ui::button` 组合（`ui::segmented` 属 WP-2.5，缺失）；widgets 的 PDF 半（含其 1 个测试）留给 WP-3.9；`math/mod.zig:inlineDrawing` 暂落在 `render::spans`；`safety::Policy` 缺 `Clone` 由 render/options 补 impl；crate 根 `markdown/mod.rs` 的 re-export 需 lead 统一补 |
| 3.4 | markdown/mermaid | wp-3-4 | 完成（已并入） | 8 测试（Zig 2）；flowchart/sequence/pie + fallback 逐段移植 |
| 3.5 | math/{parser,symbols,fonts} | wp-3-5 | 完成（已并入） | 16 个 Rust 测试（Zig 2）；`parse(&str) -> Result<Vec<Node>, ParseError>`，Node 为 owned enum；symbols/fonts 数据逐条对照 |
| 3.6 | math/layout | wp-3-6 | 完成（已并入） | 6 测试（Zig 6）；入口 `layout(&str, Options) -> Drawing`；18 个示例公式排版非空；`math/mod.zig` 的 UI 层（block/render）留给 2.10/3.11 |
| 3.7 | math/mhchem（823 条夹具，逐字节对齐） | wp-3-7 | 完成（已并入） | 823/823；207 条状态机转移与 Zig 逐条对照；拆成 15 个文件；`to_tex(&str, Machine) -> Result<String, MhchemError>` |
| 3.8 | pdf/{document,text,renderer,annotations,native}、platform/pdf | wp-3-8 | 完成（已并入） | `Document` Send+Sync（原生层已串行化）；渲染 worker 的 wake 用注入回调（3.9 传 `ui::post_wake`）；缓存返回 `Arc` 快照；Windows PDFium 待验证 |
| 3.9 | pdf/viewer、ui.pdfPreview | wp-3-9 | 完成（分支 rust/wp-3.9，待并入） | 7 测试（Zig viewer 2 + 新增 5，pdf_viewer_test.zig 两条延迟用例已移植）；`Viewer` = `Rc<RefCell<Inner>>` 句柄（回调需 `'static` 闭包）；`ui::post_wake` 直接作 renderer wake，无需运行时改动；`on_change`/`on_annotation` 改为下一次 build 的 effect；`ui::segmented`（WP-2.5 未并入）暂用图标按钮，见报告 |
| 3.10 | db/sqlite → rusqlite + derive(Table) | wp-3-10 | 完成（已并入） | 生成的 SQL 与 Zig 逐字节一致（含迁移 ALTER）；Zig 建的库 Rust 可读已实测；fallback 表名差异登记见待决问题 |
| 3.11 | examples/{markdown,math,markdown_stream,pdf} | wp-3-11 | 完成（worktree wp-3.11） | `rust/wp-3.11`。4 个新 bin（`zant-markdown` / `zant-math` / `zant-md-stream` / `zant-pdf`）+ 9 个 headless 测试（markdown 3、math 3、stream 3、pdf 3；components 由 6 → 8）。parity：`zant-markdown` 6/6（0 像素）、`zant-math` 6/6（自定义截图；Zig 示例没有 key，`layout.txt` 为空，`capture.sh` 因此拒绝，见待决问题 9）、`zant-md-stream` 6/6（`ZANT_STREAM_CHARS=1000000 ZANT_STREAM_MS=16 ZANT_STREAM_HISTORY=3`，0 像素）、`zant-components` 6/6（`pdf-2002` 占位换成真 viewer；换之前 layout 6/6 FAIL）、`zant-demo` 6/6。`zant-pdf` 未过：Zig 示例必须带 argv、且 Rust viewer 工具条的批注切换用图标按钮（Zig 用 `ui.segmented`），layout 差 25 项、像素差 0.7–2.0%，全部在工具条，**页面区域与页面瓦片完全一致**（见待决问题 8）。性能（Release，`ZANT_FRAME_STATS=1`，history 8/16）：`[slow-frame]` 只有挂载首帧 29.5 ms（Zig 29.1 ms，两边 `text_layouts` 都是 **244**，history 8→16 内容翻倍仍是 244、不增长），流式帧 1–2 ms（最慢一帧 7.7 ms，Zig 1.3–2.2 ms），build avg 0.06–0.07 ms（Zig 0.04 ms）。偏差：`zant-pdf` 无 argv 时打开 `tests/fixtures/widgets.pdf`（Zig 打印 usage 退出）；`math/mod.zig` 的 UI 层 `block` 落在 example crate；`ui.pdfViewer` 框架侧未移植，example 内同名实现（待决问题 7/8）。 |

### M4 TSX 编译器前端

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 4.1 | lexer、ast、jsx_text、jsx_entities | wp-4-1 | 完成 | `rust/wp-4.1`；10/10 Rust 测试（Zig 可移植 10 个）；AST 字符串用 `Vec<u8>`（见待决问题）；`jsx_entities_test.zig` 另 2 个依赖 parser/compile 的测试待 4.2/4.3 补 |
| 4.2 | parser、expr_parser、jsx_parser、type_parser、loop_parser、import_parser、react_compat | wp-4-2 | 完成（已并入） | 33 测试；`dump_program`（确定性 AST dump，格式见报告）；仓库 32 个 testdata + examples/tsx 全部可解析 |
| 4.3 | hooks/{expand,rewrite,resolve,leftover} | wp-4-3 | 完成（已并入） | 41 个 zant-tsx 测试；5/5 Zig hooks 测试同名移植；真实 tests/tsx/hooks 项目与 Zig 编译器输出逐名对照 |
| 4.4 | tags、tag_aliases、string_method_names、诊断 | wp-4-4 | 进行中（worktree wp-4.4） | |

### M5 TSX 后端与运行时

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 5.1 | 代码生成设计文档 `tsx-codegen.md` | lead | 完成 | ★★★；见 docs/rust-port/tsx-codegen.md（类型映射、非 Copy 一律 clone、props/事件、组件/局部状态、JS 内建语义、黄金用例策略） |
| 5.2 | zant-macros derive(TsxProps) + zant-tsx-tags + dts | — | 待领取 | |
| 5.3 | codegen 核心（types/scope/expr/stmt/rust_text/builtins/narrowing/functions/loops/aggregate） | — | 待领取 | |
| 5.4 | JSX 生成（jsx/jsx_values/jsx_children/rich_text/list/events） | — | 待领取 | |
| 5.5 | 组件与状态（codegen/components/model/stateful/hooks/stores/array_methods/local_state） | wp-5-5 | 完成（分支 rust/wp-5.5） | 入口 `codegen::generate::generate(source_name, &Module, &Options)`（另有 `generate_with(.., install_extra)`）→ `GeneratedFile { source }`（syn + prettyplease 校验/格式化，头部 `@generated` 注释在格式化后拼回）。新增 `generate/components/stateful/hooks/stores/array_methods/string_methods/captures/callbacks/free_names/globals/spread/nesting` 模块；`mod.rs` 加 Generator 生产字段与 WP-5.4/5.6/5.8 扩展钩子。测试：lib 114（+5：free_names 2、nesting 2、string_methods 2、stateful 2、stores 2、globals 2、array 1 等）、集成 `tests/codegen_components.rs` 7 条黄金用例。5.4 未落地：JSX/事件用测试桩注入（`install_extra`），合并时按 `generate::install` 里的注释逐行填 `emit_jsx/scan_body/scan_expr/emit_setter/method_param/...`；细节见待决问题 |
| 5.6 | TSX 应用层：imports/windows/desktop_hooks/zant_api/assets/entry/compile/main | wp-5-6 + lead | 完成（已并入） | 项目级编译 API `compile_project_root_with`；app 层 + 编译器入口；lead 补齐 native spec 生成/导入、嵌套组件分发、forwarded 回调、prop clone |
| 5.7 | src/tsx*.zig → zant/src/tsx/ | — | 待领取 | |
| 5.8 | native 模块（trait 化） | — | 待领取 | ★★★ |
| 5.9 | 黄金用例换成 .rs | wp-5-9 | 完成（worktree wp-5.9，待并入） | 分支 `rust/wp-5.9`。`crates/zant-tsx/testdata/*.rs` 30 个黄金（32 个 Zig 用例里 `desktop`/`entities` 无法生成，见下方「WP-5.9 偏差表」）；`tests/golden.rs`（黄金逐字节比对 + 全部黄金批量写入一个 workspace 外临时 crate，`cargo check` 通过 **29/30**，`widgets` 因框架 API 缺失排除）；`tests/tsx_end_to_end.rs`（`compile_project_root_with` 驱动 `tests/fixtures/{hooks,crossfile,rust_types,media}`，Zig 模块移植为 `.rs` 镜像）；逐服务器审阅 30 个黄金时修掉 30+ 处编译器/联调 bug（见「WP-5.9 提出」）。 |
| 5.10 | examples/tsx 入口、bench-tsx | wp-5-10 | 完成（worktree wp-5.10，待并入） | 分支 `rust/wp-5.10`。3 个 TSX 示例应用改成 Cargo workspace 成员（`crates/zant-examples/tsx/{tsx-counter,tsx-demo,tsx-bench}`，template 形态：`build.rs` → `zant_build::try_tsx_app()`）；`scripts/tsx/examples-check.sh` 无窗口跑 3 个应用并断言 PNG 非空白 / 尺寸 / 确定性 / `ZANT_LAYOUT_FILE` keyed 控件；`zant-tsx-bench` 编译基准（32 个语料，Rust 编译 30 个，best-of-3 平均 **1.08×** Zig in-process，闸门 ≤1.25× PASS）。详见下方「WP-5.10 完成情况」与「WP-5.10 提出」。**WP-5.10b**（wp-5-10b，分支 `rust/wp-5.10b`）：plan M5 验收剩下的 4 个 TSX 应用（gallery/todos/reader/notes）+ notes 的 `format.zig` → native 模块 + `examples-check.sh` 的 Zig 基线对照 + `tsx-counter.steps` 真实 GUI 场景，详见下方「WP-5.10b 完成情况」/「WP-5.10b 提出」。 |

| 5.10 | examples/tsx 入口、bench-tsx | wp-5-10 | 完成（worktree wp-5.10，待并入） | 分支 `rust/wp-5.10`。3 个 TSX 示例应用改成 Cargo workspace 成员（`crates/zant-examples/tsx/{tsx-counter,tsx-demo,tsx-bench}`，template 形态：`build.rs` → `zant_build::try_tsx_app()`）；`scripts/tsx/examples-check.sh` 无窗口跑 3 个应用并断言 PNG 非空白 / 尺寸 / 确定性 / `ZANT_LAYOUT_FILE` keyed 控件；`zant-tsx-bench` 编译基准（32 个语料，Rust 编译 30 个，best-of-3 平均 **1.08×** Zig in-process，闸门 ≤1.25× PASS）。详见下方「WP-5.10 完成情况」与「WP-5.10 提出」。 |
| 5.10c | `bench-tsx`：TSX 生成代码 vs 手写 Rust 帧对比 | wp-5-10c | 完成（worktree wp-5.10c，待并入） | 分支 `rust/wp-5.10c`。M5 验收「TSX 生成代码不慢于手写 Rust 对照组 1.2 倍」**全部通过**：`zant-tsx-frame-bench`（`crates/zant-examples/src/bin/`，`build.rs` 用 `zant_build::tsx_app_in` 把 `examples/tsx/` 六个基准页编成 Rust）+ `scripts/tsx/bench-tsx.sh`（Release、best-of-3、交替先后、逐 app 及逐场景判 ≤1.2×）。本机 6/6 apps、11/11 scenarios PASS，最高 **1.11×**。详见下方「WP-5.10c 完成情况」与「WP-5.10c 提出」。 |
| TSX-BUGS | `zant-tsx` 已验证编译器 bug 修复（子组件回调同步执行 / `Option<Node>` 裸元素 / 富文本末尾字面量 / 非根 `<List>`） | wp-tsx-bugs | 完成（worktree wp-tsx-bugs，待并入） | 分支 `rust/wp-tsx-bugs`。子组件 handler 现在带 `_cx: &mut ui::ActionCx<'_>` 同步调用回调 prop（去掉被 `begin_frame` 清掉的 `ui::effect`）；`<List>` 把 `_list`/`_state` 提到行闭包外、闭包内 action 用 hoisted `_state`（原 workaround 生成根 action，运行期 `ActionCx::state` 会 panic）；bug 2/3 的 base staged fix 补上回归测试。golden 重新生成 5 个（其余逐字节不变）；gallery/todos/reader/notes 与 Zig 基线 **0 px**，bench-tsx 6/6 apps、11/11 scenarios PASS。详见下方「WP-TSX-BUGS 完成情况」。 |

### M6 CLI / 构建 / 脚手架 / 发布

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 6.1 | tools/zant → zant-cli | wp-6-1 | 完成（worktree wp-6.1，待并入） | 分支 `rust/wp-6.1`。7 个 Zig test 同名移植 + 28 个新增（29 单元 + 6 集成 = 35 全过）；`create`/`sync` 的 stdout/stderr、`app.config.json`、`Info.plist` 与 Zig 版实测逐字节一致；`run --watch` 手工冒烟通过（改 src 后重编译并重启）；`zant build` 走到 `macos.sh developer-id`（DMG 本轮沙箱里 hdiutil 被卡住，见报告）。TSX 编译接口以 `ensure_rust_project` 的明确报错占位，见「M6 接口约定」 |
| 6.2 | `zant_build::tsx_app()` + 图标/manifest | wp-6-2 | 完成（已并入） | OUT_DIR 模块 + main.rs + `_assets`/`_icons` + dep 文件 + rerun-if-changed；macOS/Win 资源；`scripts/tsx/build-check.sh` e2e |
| 6.3 | scaffold/template-ts | — | 待领取 | |
| 6.4 | package 脚本产物路径 | — | 待领取 | |
| 6.5 | release / install / CI | — | 待领取 | |
| 6.6 | 原生库 CI 预构建 | wp-6-6 | 完成（已并入，可选） | `ZANT_NATIVE_PREBUILT` + 源哈希校验 + 失败回退；release 包附带 212K 预编译库；原生编译 3.6s→0.6s |

### M7 下游应用与文档

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 7.1 | ../cliphop-ts native 引擎移植 | — | 待领取 | |
| 7.2 | ../stillmd 移植 | — | 待领取 | |
| 7.3 | 重写 README/AGENTS/CLAUDE 与 docs/ | wp-7-3 | 完成（已并入） | 54 个文档改为 Rust/TSX；链接/代码块检查通过 |

### M8 切换

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 8 | 全量验证、移动原生源码、删除 Zig、CI、合并 main、发布 | lead | 待领取 | **需用户确认后执行** |

**M8 切换脚本就绪（WP-M8-PREP，2026-11-14）**：`scripts/migrate/cutover-m8.sh` +
`docs/rust-port/m8-cutover.md`。脚本默认 dry-run（打印全部 `git mv` / `git rm` / 路径改写并校验
前置条件），`--apply` 先打 `rust-migration-pre-m8` 备份 tag，再按 plan M8.2/M8.3 移动
`crates/zant-sys/native/`、删除 348 个 `*.zig` / `build.zig(.zon)` / `tools/` / `scaffold/template/` /
Zig 构建脚本，改写 `build.rs`、bindgen、预编译库列表、CI、release/package、Rust 夹具与文档引用，
最后跑全量验收；`--verify-only` 可单独复跑验收。`tools/zant/package/` 移到 `scripts/package/`，
`tools/tsx/testdata/*.tsx` 与 `examples/tsx/**` 语料移到 `crates/zant-tsx/testdata/tsx/` 与
`crates/zant-examples/tsx/corpus/`，CLI 删除 `--lang zig`。切换后 parity 只能对旧基线手工跑，
`scripts/parity/bench.sh` 无 Zig 基线时进入 Rust-only 模式并对照本文件记录的数字；
`scripts/tsx/examples-check.sh` 只跑 Rust 侧编译基准。**未执行任何切换**（需用户批准；本机 Windows
验收仍缺）。以最新 `feat/rust-port`（`1429d69`）为基的一次性 `/tmp` 副本演练通过：74 个测试套件 /
1074 passed / 0 failed（比切换前 1076 少 2 条纯 Zig 模板测试）、clippy 0、fmt 干净、模板 / TSX 示例
检查通过、bench（Rust-only，best-of-12）default 7/7 + list 7/7 PASS。细节见
`docs/rust-port/m8-cutover.md`「演练记录」。

## 并行执行与合并方式

- 每个 WP 一份 git worktree：`scripts/worktree.sh new wp-3.1` → `.worktrees/wp-3.1`，分支 `rust/wp-3.1`。
  执行者在 worktree 里提交，lead 逐个 `git merge rust/wp-x.y` 到 `feat/rust-port` 并统一跑
  `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo test --workspace`、`zig build test`。
- worktree 软链共享 `.cargo-home` / `.zig-global-cache` / `third_party`；沙箱里 `cargo` 一律经
  `scripts/cargo.sh`（见 conventions.md）。
- 模块声明（`lib.rs`、各级 `mod.rs`）已在基线里预置，执行者只填自己 WP 的文件；确实要加时在报告里说明。

## WP-2.10 补充待办（由 1.1a 移交）

- `tests/flex_test.zig` 的 24 个用例是 `Tree` / `Node` 级集成测试（`tree.addContainer` / layout / hitTest …），
  依赖 WP-1.3 的保留树，不能放在 1.1a。
- `tests/motion_test.zig` 后 6 个用例依赖 `ui` / `ui_runtime` / renderer / button / segmented
  （WP-1.2 / 1.3 / 1.4 / 1.1c），前 4 个纯 motion 用例已随 1.1a 移植；后 6 个已由 WP-2.7 移植到
  `crates/zant/tests/motion_test.rs`，WP-2.10 不要重复移植。

### WP-2.10 完成情况（wp-2-10，2026-11-13）

- 已移植 53 条（13 个新文件）：`view_test` 7、`selection_test` 6、`lifecycle_test` 6、
  `ui_test` 5、`component_test` 4、`antd_components_test` 12、`local_state_test` 2、
  `pointer_test` 3、`text_test` 3、`text_align_test` 1、`store_test` 1、
  `mhchem_render_test` 1、`all_test` 2（`tests/all.zig` 的 2 条内联用例）。
  其中 `pointer_test.zig` 的 pointer_area 用例在 lead 的 key-bounds 修复（`cbab28c`：快照 hook
  保留到整帧结束）merge 后补上，断言 `on_resize` 里 `ui::key_bounds("area")` 能读到新 bounds。
- 已由其他 WP 覆盖、未重复移植：`flex_test.zig` 24（WP-1.3）、`list_test.zig` 3（WP-2.6，
  在 `src/components/list.rs` 内同名移植）、`motion_test.zig` 前 4 条（WP-1.1a）、
  `markdown_test` / `markdown_render_test` / `math_test` / `pdf*_test` / `db_test` / `remend_test`。
- 仍不能移植（1 条）：`component_test.zig`「gallery button click, input editing, composition and
  focus」：它驱动的是 Zig `src/app.zig` 的 gallery 命令式 `App`（`App.create` / `handle` / `tree`）；
  Rust 侧的 gallery 是 `zant-examples` 的 `gallery_app`（WP-2.9），不是框架 API，`crates/zant/tests`
  不做跨 crate 依赖，故保留未移植（该应用另有 `crates/zant-examples/tests/gallery_smoke.rs` 覆盖）。
- 仍归其他 WP：`motion_test.zig` 后 6 条（WP-2.7）、`tsx*_test.zig`（M4/M5）。

### WP-5.10 完成情况（wp-5-10，2026-11-14）

分支 `rust/wp-5.10`。**TSX 示例与 TSX 编译基准**：`examples/tsx/` 的 Rust 入口（3 个 workspace 成员应用）
+ 32 个语料文件的进程内编译基准（对照 Zig 编译器进程内基线）。

**新增文件**

- `crates/zant-examples/tsx/`（3 个 Cargo workspace 成员，`scaffold/template-ts` 形态：`build.rs` →
  `zant_build::try_tsx_app()`、`src/main.rs` → `zant::tsx::app::run`、`native/system.ts|rs` 原生模块）：
  - `tsx-counter/`（bin `zant-tsx-counter`）：`examples/tsx/counter.tsx` 的 1:1 移植（窗口 520×480，9 个 key）；
  - `tsx-demo/`（bin `zant-tsx-demo`）：`examples/tsx/app/App.tsx` 的移植（`Hello from TSX`、两个 Counter、
    store、tray、panel 窗口、clipboard、`useAppExit` 里的 `native/system` 原生调用；窗口 480×360）；
  - `tsx-bench/`（bin `zant-tsx-bench-app`）：帧基准的重负载页 `examples/tsx/settings.tsx`（40 个开关，
    窗口 640×900，布局 86 个 key）；
  - `tsx/app_config_build.rs`：三个 `build.rs` 共用的 body。workspace 成员没有 `zant sync` 步骤，所以它在
    `build.rs` 里把 `zant sync` 会写的 `src/app_config.rs` 从 `try_tsx_app()` 已解析的 `app.config.json`
    写出来（内容比较后写，避免 `src/` rerun-if-changed 抖动；文件 gitignore）。
- `crates/zant-examples/src/bin/zant-tsx-bench.rs`：32 个 `tools/tsx/testdata/*.tsx` 逐文件在进程内调
  `zant_tsx::compile`（默认 10 轮预热 + 100 次迭代），输出与 Zig `bench_harness.zig` 相同的统计行
  （每文件一行 `frames=… avg=… p95=… max=…` + `total`）；被 Rust 编译器拒绝的文件打印诊断并跳过。
- `tools/tsx/compile_bench.zig`：Zig 基线（独立基准工具，不改 `zant-tsx` 编译器）：同一个
  `tools/tsx/compile.zig` 进程内跑同一语料、同样预热/迭代/输出格式。Zig 的
  `zig-out-baseline/bin/zant-tsx-bench` 是**帧**基准（`clicks frames= 3000 avg= 0.099 ms` …），不是编译时间，
  不能当编译基准的基线。
- `scripts/tsx/examples-check.sh`：构建 3 个示例 → 逐个无窗口跑 `ZANT_SNAPSHOT` 并断言（见下）→ 跑编译基准
  （与 Zig 基线交替跑 3 轮，取每文件最好一轮，闸门 ≤1.25×）。
- 接线：根 `Cargo.toml` +3 个 workspace 成员；`crates/zant-examples/Cargo.toml` +`zant-tsx` 依赖与
  `[[bin]] zant-tsx-bench`。

**截图证据**（`scripts/tsx/examples-check.sh` 每次重新生成；本机浅色 2× 显示，证据文件在
`target/wp-5.10-evidence/`）

| 示例 | 窗口 | 快照 | 断言 |
|---|---|---|---|
| `zant-tsx-counter` | 520×480 | 1040×960，36 404 B，sha256 `e86260f6…` | 9 个 key：title/count/name/inc/reset/step/large/notify/note |
| `zant-tsx-demo` | 480×360 | 960×720，45 653 B，sha256 `7a0a6bb4…` | 8 个 key：label-Apples/label-Pears/minus-Apples/plus-Apples/total/toggle-panel/copy/note |
| `zant-tsx-bench-app` | 640×900 | 1280×1800，132 003 B，sha256 `f6aec2c8…` | changes/row-0/sw-0/row-39/sw-39（布局 86 个 key） |

`examples-check.sh` 的断言：PNG 签名有效、≥16 种颜色且主色 < 99%（非空白）、尺寸 = `app.config.json`
逻辑尺寸 × 1–4 显示倍率、两次运行 PNG 与 layout 字节一致、每个期望 keyed 控件盒非空、二进制 rodata 里
有窗口标题（`TSX Counter`/`TSX App`/`TSX Bench`）与 UI 字面量（`Notifications are on`/`Hello from TSX`/
`Settings`）。AX 不暴露 Skia 文本，所以「标题文字」用二进制 rodata + layout key 两条证据代替。

**编译基准**（Release；macOS 27.2.1 arm64 14 核；best-of-3；逐文件平均编译时间 ms；Rust = `zant-tsx-bench`，
Zig = `tools/tsx/compile_bench.zig`）

| 语料文件 | Rust | Zig | 比值 |
|---|---|---|---|
| app_hooks | 0.173 | 0.028 | 6.18× |
| arrays | 2.030 | 7.368 | **0.28×** |
| builtins | 2.906 | 5.286 | **0.55×** |
| children | 0.702 | 0.160 | 4.39× |
| components | 1.176 | 1.028 | 1.14× |
| conditional | 0.273 | 0.154 | 1.77× |
| counter | 0.354 | 0.090 | 3.93× |
| defaults | 0.339 | 0.056 | 6.05× |
| events | 0.882 | 0.563 | 1.57× |
| functions | 0.418 | 0.075 | 5.57× |
| hooks | 1.055 | 0.247 | 4.27× |
| layout | 0.269 | 0.090 | 2.99× |
| list | 0.369 | 0.144 | 2.56× |
| list_scroll | 0.367 | 0.219 | 1.68× |
| local_state | 0.754 | 0.161 | 4.68× |
| loops | 1.325 | 0.456 | 2.91× |
| named_handlers | 0.556 | 0.136 | 4.09× |
| narrowing | 0.644 | 0.243 | 2.65× |
| nesting | 0.572 | 0.180 | 3.18× |
| platform | 0.415 | 0.165 | 2.52× |
| react_compat | 0.277 | 0.046 | 6.02× |
| spread | 0.301 | 0.071 | 4.24× |
| store | 0.590 | 0.117 | 5.04× |
| strings | 1.437 | 1.396 | 1.03× |
| text | 0.282 | 0.107 | 2.64× |
| theme | 0.289 | 0.062 | 4.66× |
| timers | 0.454 | 0.090 | 5.04× |
| todos | 0.757 | 0.398 | 1.90× |
| widgets | 0.382 | 0.054 | 7.07× |
| windows | 0.433 | 0.060 | 7.22× |
| **平均（共同编译的 30 个）** | **0.693** | **0.642** | **1.08×** |

**结论：平均 1.08× Zig（闸门 ≤1.25×，PASS）。** 小文件 Rust 慢 2–7×（固定开销是 `syn` 解析 +
`prettyplease` 格式化生成的 Rust），大文件反超（`arrays` 0.28×、`builtins` 0.55×）。`desktop`/`entities`
Rust 编译器仍拒绝（见「WP-5.10 提出」4），基准输出 `failed: 2 (desktop, entities)`，比对取两侧共同编译的
30 个文件。

**偏差**

1. `tsx-demo` 的 `Panel` 窗口声明放在 `src/components/Panel.tsx`（组件自己的文件），`App.tsx` 用
   `import { panel } from "./components/Panel"`。Zig 版是 `App.tsx` 里 `createWindow(Panel, …)` + 从
   `./components/Panel` 导入组件；Rust 生成器跨文件窗口组件缺 `impl tsx::window::Component`
   （见「WP-5.10 提出」1）。窗口选项、组件、keys、UI 完全一致。
2. `tsx-counter` 的 `note` 文本保持 Zig 的 `Notifications are on`（原生模块只编译/注册，不在 UI 调用）；
   `tsx-demo` 的原生调用放在 `useAppExit` 日志里（窗口 UI 与 tray tooltip 与 Zig 版一致）：
   `TSX App: exiting with total 0 (14 cpus)`，`ZANT_SNAPSHOT` 退出时实际执行。
3. 示例 crate 级 `#![allow(clippy::all)]` / `#![allow(non_upper_case_globals)]` 与 `#[rustfmt::skip]
   pub mod spec;` 是生成代码的 lint 兜底（见「WP-5.10 提出」2/3），`zant-tsx` 修好后可去掉。
4. 首次 `cargo clean` 后构建这三个应用会打印一次 `zant-build: src/app_config.rs is missing`（
   `try_tsx_app()` 先打印 cargo 指令，共有 build helper 后写文件），之后不再出现，不影响构建。
5. `tsx-bench/` 的包/二进制名是 `zant-tsx-bench-app`，避免与 `zant-examples` 的 `zant-tsx-bench`
   二进制在 `target/` 重名。
6. 只移植了**编译**基准（父任务要求）；Zig `bench.zig` 的**帧**对比（clicks/typing/todos/list/markdown/local，
   TSX 生成代码 vs 手写 Rust ≤1.2×，plan M5 验收）仍未移植，需要把 `*_manual.zig` 对照组与 harness 改成 Rust。

**验收命令**（全部通过）

- `./scripts/cargo.sh test --workspace`：71 个测试二进制 / **1059 passed, 0 failed**（新增 6 条：3 × 示例
  native `system`、3 条 `zant-tsx-bench` 语料/p95/统计行用例）；
- `./scripts/cargo.sh clippy --workspace --all-targets -- -D warnings`：0 告警；
- `./scripts/cargo.sh fmt --all -- --check`：干净；
- `./scripts/tsx/examples-check.sh`：3 个示例 + 编译基准 PASS（1.08×）；
- `./scripts/tsx/build-template-check.sh`、`./scripts/tsx/build-check.sh`：仍通过。

**待验证**：Windows（测试机不可达）——3 个示例应用与编译基准未在 Windows 上跑；应用不带平台专属资源，
`app.config.json` 走通用字段。

### WP-5.10b 完成情况（wp-5-10b，2026-11-14）

分支 `rust/wp-5.10b`。**plan M5 验收剩下的 TSX 示例应用 + 真实 GUI 场景**：把
`examples/tsx/{gallery,todos,reader}.tsx` 与 `examples/tsx/notes/` 的 Rust 入口补成 4 个 Cargo
workspace 成员（与 WP-5.10 的 3 个同形态），扩展 `scripts/tsx/examples-check.sh`，并用
`scripts/mac/ui-scenario.swift` + `scripts/mac/tsx-counter.steps` 真实驱动 `zant-tsx-counter`。

**新增文件**

- `crates/zant-examples/tsx/{tsx-gallery,tsx-todos,tsx-reader,tsx-notes}/`（4 个 workspace 成员，
  `scaffold/template-ts` 形态：`build.rs` → `zant_build::try_tsx_app()`、`src/main.rs` →
  `zant::tsx::app::run`、`app.config.json`、`assets/icons/`（模板小图标，`tsx_build` 的“没有图标”
  警告用）、`.gitignore` 忽略 `src/app_config.rs` / `native/spec.rs`）：
  - `tsx-gallery/`（bin `zant-tsx-gallery`）：`examples/tsx/gallery.tsx` 移植，窗口 960×760；
  - `tsx-todos/`（bin `zant-tsx-todos`）：`examples/tsx/todos.tsx` 移植，窗口 560×640；
  - `tsx-reader/`（bin `zant-tsx-reader`）：`examples/tsx/reader.tsx` 移植，窗口 1120×760；
    PDF 用仓库已有的小夹具 `tests/fixtures/widgets.pdf`（**不复制**，保持 Zig 的路径，从仓库根运行）；
  - `tsx-notes/`（bin `zant-tsx-notes`）：`examples/tsx/notes/App.tsx` + `NoteCard.tsx` 移植，
    窗口 520×560，外加 `native/{format.ts,format.rs,lib.rs}`（见下）。
  TSX 源码逐字复制 Zig 版，只改文件头注释；`tsx-gallery` 的 Modal `footer` 因编译器 bug 改成
  `footer={open ? (<Row>…) : null}`（「WP-5.10b 提出」2，渲染树与 Zig 版相同）。
- `tsx-notes/native/format.ts` + `tsx-notes/native/format.rs`：`examples/tsx/notes/format.zig` 的
  `plural` 移植成 native 模块（`impl FormatSpec for Native`，`native/spec.rs` 的 trait 由
  `zant_build` 生成；`src/App.tsx` 从 `"./format.zig"` 改为 `"native/format"`）。Zig 版导入一个
  普通 Zig 模块，Rust 编译器拒绝 Zig 导入（`imports.rs` 的明确报错），native 模块是同一个函数在
  Rust 应用里的位置；TSX 调用编译成直接 Rust 调用，编译期由 trait 检查签名。附 1 条单测
  `plural_appends_s_only_for_multiple`。
- 接线：根 `Cargo.toml` +4 个 members；`crates/zant-examples/tsx/README.md`；`docs/tsx.md`
  「示例」段；本文件。

**`scripts/tsx/examples-check.sh`**：构建 7 个应用 → 逐个无窗口跑 `ZANT_SNAPSHOT` + `ZANT_LAYOUT_FILE`
（原有断言：PNG 非空白（有效签名、≥16 种颜色、主色 <99%）、尺寸 = 逻辑尺寸 × 1–4 显示倍率、两次运行 PNG 与 layout
字节一致、期望 keyed 控件盒非空、标题与 UI 字面量在二进制 rodata）→ 新增 **Zig 基线对照**段：
`ZANT_ZIG_BIN=<zig-out-baseline/bin>` 时用同一环境跑 Zig 版 4 个应用，断言 `ZANT_LAYOUT_FILE`
**逐字节相同**、像素差 ≤0.1%（`scripts/parity/imgdiff.py`）。然后仍是编译基准（≤1.25× Zig）。

**截图证据**（浅色；`examples-check.sh` 每次重新生成；`target/wp-5.10b-evidence/` 留档）

| 示例 | 窗口 | 快照 | keyed 控件 | Zig 基线 |
|---|---|---|---|---|
| `zant-tsx-gallery` | 960×760 | 1920×1520，21 个 key | page/inputs/name/agree/notify/plan/select/size/display/progress/advance/open/refresh/saved/nesting/rich/save-icon/copy-icon/clickable-card/inner/tabs | layout 相同，21/2 918 400 px（0.0007%，即下方 bug 3 丢的句点） |
| `zant-tsx-todos` | 560×640 | 1120×1280，6 个 key | new/add/filter/empty/left/clear | layout 相同，0 px |
| `zant-tsx-reader` | 1120×760 | 2240×1520，7 个 key（另有 PDF/Markdown 动态 key） | append/finish/reply-scroll/reply/path/open/reader | layout 相同，0 px（PDF 夹具真实渲染） |
| `zant-tsx-notes` | 520×560 | 1040×1120，4 个 key | new/add/search/empty | layout 相同，0 px |

深色（`ZANT_THEME=dark`）同样对照：4 个应用 layout 全等，todos/reader/notes 0 px，gallery 同为 21 px。
4 个应用两次运行 PNG+layout 字节一致。

**真实 GUI 场景（`scripts/mac/tsx-counter.steps`，macOS 登录会话）**

```bash
swiftc -O scripts/mac/ui-scenario.swift -o /tmp/ui-scenario
target/debug/zant-tsx-counter &                     # ZANT_LAYOUT_FILE=/tmp/scenario/layout.txt
CONTENT_HEIGHT=480 /tmp/ui-scenario zant-tsx-counter /tmp/scenario/layout.txt /tmp/scenario scripts/mac/tsx-counter.steps
```

结果：`scenario ok`（0 problem；12 个 `shot` 步骤都执行，但 `screencapture` 因权限失败、不计 failure）。本机**无 Screen Recording 权限**，
`screencapture -l` 输出 `could not create image from window`，所以截图不作为证据；状态变化全部用
`ZANT_LAYOUT_FILE` 盒 + 外部 `CGWindowList` 窗口框断言（`ui-scenario` 的 `layout` 步骤在
`scripts/mac/tsx-counter.steps` 的副本里插到每个动作后）：

| 步骤 | 断言（layout 盒，逻辑 px） |
|---|---|
| 初始 | title 115×24、count 115、inc 45.8、note 在；窗口框 520×512（内容 520×480 + 标题栏 32） |
| `hover inc` | 盒不变（hover 只是 paint；步骤成功 = 无 missing key） |
| `click inc 2` | `count` 115→**114**（文字 `Count: 0 (×2 = 0)` → `Count: 2 (×2 = 4)`） |
| `click step` | `inc` 45.8→**47.9**，reset/step 右移（按钮文字 `+1`→`+5`） |
| `click inc 2` | `count` →**129**，**出现 `many` key**（`That is a lot of clicks.`） |
| `click large` | `title` 115×24→**160×33**，后续控件整体下移 9 px（28 号标题） |
| `click notify` | **`note` key 消失**（`Notifications are on` 条件块） |
| `click name` + `type 编译 ✓` | `title` 115→**256**（`Hello, Zant编译 ✓!` 变宽），高 24→39（CJK 回退字体行高）——键盘输入生效 |
| `key backspace` ×2 | `title` 256→**224**（删掉 `✓` 与空格） |
| `click reset` | `count` →**115**（回到初始），`many` 消失 |
| `resize 420 420` | 盒布局不变（counter 左对齐，无 grow）；窗口框 520×512→**420×452**（内容 420×420 + 标题栏 32）→ resize 生效 |
| `key tab` | 盒不变（焦点环只是 paint） |

另外对 `zant-tsx-notes` / `zant-tsx-gallery` 做了同样的真实点击（证据在 `target/wp-5.10b-evidence/`）：
notes 输入 + Add 后出现 `summary`（**native `plural` 在运行期被调用**）与 `open-1/title-1/pin-1/delete-1`，
点 `open-1` 出现 `detail-1`；gallery 点 `open` 出现 `dialog/dialog-card/cancel/ok`，点 `ok`/`cancel` 关闭
（Modal footer workaround 可用）。notes 的 `pin-1`/`delete-1` 不生效——子组件回调 prop 的编译器 bug，
见「WP-5.10b 提出」1（Zig 基线同步骤能删除）。

**验收命令**（全部通过，见分支报告；Windows 测试机不可达，未验证）

- `./scripts/cargo.sh test --workspace`、`clippy --workspace --all-targets -- -D warnings`、
  `fmt --all -- --check`；
- `./scripts/tsx/build-template-check.sh`；
- `./scripts/tsx/examples-check.sh`（7 个应用 + Zig 4 应用对照 + 编译基准 1.09× PASS）；
- `ZANT_ZIG_BIN=<main checkout>/zig-out-baseline/bin ./scripts/parity/compare.sh zant-demo`（6/6，0 px）。

**偏差 / 剩余**

1. `format.zig` 按本 WP 要求改成 native 模块（Zig 版是 `./format.zig` 普通模块导入）；TSX 的调用
   点因此从 `"./format.zig"` 变成 `"native/format"`，其余逐字相同。
2. gallery 的 Modal `footer` 用三元包了一层（编译器 bug 2 的 workaround），渲染树与 Zig 版相同。
3. 本 WP 新报的编译器 bug（见下）使 notes 的 pin/delete 在 Rust 里不生效；初始快照与 Zig 逐字节
   layout/0 px 一致，交互行为待 `zant-tsx` 修复后复验。
4. 截图证据仍是应用 `ZANT_SNAPSHOT`（渲染 PNG），不是系统截图；`ui-scenario` 的 `shot` 步骤因权限
   失败但不影响步骤断言（本机实测）。
5. TSX **帧**对比基准（TSX 生成代码 vs 手写 Rust ≤1.2×，plan M5）仍未移植（承 WP-5.10 偏差 6）。

### WP-5.10c 完成情况（wp-5-10c，2026-11-14）

分支 `rust/wp-5.10c`。**TSX 帧对比基准**：`zant-tsx-frame-bench` + `scripts/tsx/bench-tsx.sh`；
plan.md §5 M5 验收「`bench-tsx` 中 TSX 生成代码不慢于手写 Rust 对照组 1.2 倍」全部通过。
Zig 的 `bench-tsx`（`examples/tsx/bench.zig` + `bench_harness.zig` + 四个场景文件）由这份 Rust 基准取代。

**新增文件**

- `crates/zant-examples/src/bin/zant-tsx-frame-bench.rs` + `src/bin/zant_tsx_frame_bench/`：
  - `mod.rs`：驱动、`Stats`（avg / p95 / max，p95 用 Zig 的 `len*95/100` 下标）、headless `Harness`
    （640×900 raster、假时钟、未计时 warm-up、按 key 点击 / 输入）、`ZANT_BENCH_REPEAT` 交替 best-of-N、
    `ZANT_BENCH_MAX_RATIO` 闸门、`ZANT_FRAME_STATS=1` 相位统计；
  - `counter` / `todos` / `list` / `local` / `markdown` / `settings`：逐场景；
  - `controls/`：手写对照（`counter_manual.zig` / `todos_manual.zig` / `list_manual.zig` /
    `local_manual.zig` / `bench_markdown.zig` 的 `Manual` 逐行移植 + 新增 settings 对照）；
  - `tsx_views/ListBench.tsx` / `MarkdownView.tsx`：两处语言差异的最小适配（见「偏差」）。
- `crates/zant-examples/build.rs`：为六个基准页在 `$OUT_DIR` 各造一个临时 app root（`src/App.tsx`
  就是 `examples/tsx/*.tsx` / `tsx_views/ListBench.tsx`），调 `zant_build::tsx_app_in`（与 Zant 应用
  build.rs 同一个 TSX → Rust 步骤）；生成的 `main.rs` 由 `mod.rs` `include!` 进六个 case 模块。
- `scripts/tsx/bench-tsx.sh`：Release 构建（并断言 6 个生成 app 存在）→ 跑 best-of-N → python
  打印逐 app / 逐场景对照表 + 闸门；`--no-build`、`--help`，环境变量 `ZANT_BENCH_REPEAT` /
  `ZANT_BENCH_MAX_RATIO` / `ZANT_FRAME_STATS` / `ZANT_RUST_BIN` / `CARGO_TARGET_DIR`。
- 接线：`crates/zant-examples/Cargo.toml`（`[build-dependencies] zant-build`、`[[bin]]`）、`Cargo.lock`。

**两侧怎么跑（诚实性）**

- **TSX 侧是真实生成代码**，不是 stub：`counter.tsx`、`todos.tsx`、`settings.tsx` 原样编译；生成代码
  每帧真的走 `ui::node` / action / 状态字段。`list_bench.tsx` / `markdown_bench.tsx` 有两处最小适配，
  原因都是 Zig 版可以写根状态（`tsx.State`）而 Rust 生成代码的根字段私有：list 的行改由页面自己的
  `useMemo(() => makeRows(), [])` 建一次（未计时的 warm-up 帧内），`<List>` JSX / keys / styling 逐字
  相同；markdown 保留 `markdown_bench.tsx` 作根，另由一个接收 `source` prop 的 `MarkdownView` 走同一个
  `<Markdown>` widget。两处都有文件头注释与下方「偏差」。
- **手写侧**是 `*_manual.zig` 的逐行 Rust 移植（含 Zig 的 click / typing / add+Enter / toggle / filter /
  scroll / mount 计数与顺序）；settings 没有 Zig 对照（Zig 只测 TSX 侧），按 `settings.tsx` 的树新写。
- **同一个 harness**：Runtime + 640×900 raster canvas + 假时钟（每帧 +16 ms）+ motion off + 一个未计时
  warm-up 帧；每帧 `renderer.begin` → `Runtime::render_frame`（rebuild + layout + paint）→ `renderer.end`
  计时，与 `zant-bench` 同构；每侧每轮全新 runtime + canvas。
- **交替 best-of-N**：`ZANT_BENCH_REPEAT`（默认 3）轮，奇数轮 TSX 先跑、偶数轮手写先跑；每侧取平均帧
  时间最低的一轮比较（与 `scripts/parity/bench.sh` 同一思路，shared-machine 负载不会决定闸门）。
- **结构对照测试**：`the_generated_tree_has_the_control_keys` 断言 settings / counter / todos / list
  首帧的 keyed 节点集合与手写对照完全一致（settings 86 个 keyed 节点；list 10 000 行只建一屏，
  `pick-10000` 不在树上）。

**对照表（Release，best-of-3，macOS 27.2.1 arm64；`scripts/tsx/bench-tsx.sh`）**

| 行 | frames | tsx avg (ms) | manual avg (ms) | ratio | tsx p95 | manual p95 | tsx max | manual max |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| **counter** | 3300 | 0.113 | 0.113 | **1.00×** | 0.194 | 0.196 | 0.453 | 0.343 |
| counter-clicks | 3000 | 0.105 | 0.105 | 1.00× | 0.121 | 0.121 | 0.453 | 0.339 |
| counter-typing | 300 | 0.193 | 0.193 | 1.00× | 0.246 | 0.237 | 0.321 | 0.343 |
| **todos** | 1560 | 1.969 | 1.941 | **1.01×** | 3.061 | 3.179 | 12.546 | 12.523 |
| todos-add | 1000 | 1.531 | 1.385 | 1.11× | 2.906 | 2.590 | 4.258 | 3.166 |
| todos-toggle | 500 | 2.490 | 2.640 | 0.94× | 3.177 | 3.439 | 3.766 | 4.007 |
| todos-filter | 60 | 4.920 | 5.373 | 0.92× | 11.477 | 11.857 | 12.546 | 12.523 |
| **list** | 400 | 0.457 | 0.448 | **1.02×** | 0.535 | 0.528 | 0.660 | 0.703 |
| list-scroll | 300 | 0.468 | 0.456 | 1.03× | 0.546 | 0.535 | 0.660 | 0.703 |
| list-click | 100 | 0.426 | 0.423 | 1.01× | 0.506 | 0.500 | 0.547 | 0.568 |
| **local** | 600 | 0.661 | 0.629 | **1.05×** | 0.861 | 0.820 | 1.177 | 1.183 |
| local-mount | 300 | 0.569 | 0.527 | 1.08× | 0.794 | 0.734 | 1.176 | 0.878 |
| local-click | 300 | 0.754 | 0.730 | 1.03× | 0.883 | 0.875 | 1.177 | 1.183 |
| **markdown** | 342 | 0.410 | 0.409 | **1.00×** | 0.644 | 0.637 | 0.905 | 0.911 |
| markdown-stream | 342 | 0.410 | 0.409 | 1.00× | 0.644 | 0.637 | 0.905 | 0.911 |
| **settings** | 2000 | 0.223 | 0.239 | **0.93×** | 0.268 | 0.281 | 0.451 | 0.975 |
| settings-click | 2000 | 0.223 | 0.239 | 0.93× | 0.268 | 0.281 | 0.451 | 0.975 |

**结论：6/6 apps、11/11 scenarios 全部 ≤1.2×，最高 1.11×（`todos-add`），其余 ≤1.08×；4 次复跑稳定在
同一区间**（`counter-clicks` 1.00–1.01×、`todos-add` 1.09–1.11×、`local-mount` 1.08–1.10×、
`settings` 0.92–0.93×；`local-mount` 的旧表行 1.08×，最近一次 1.10×）。`local-mount` 的 ~1.08×
来自 TSX 页面的 React 式不可变更新（`setIds([...ids, next])` 每 mount 复制 + `setNext`），手写对照用
`Vec::push`——这正是 Zig `local_bench` 基准设计的策略差异，不是生成代码的额外开销；`todos-add` 同理
（TSX 用 `[...todos, todo]`，手写用 `Vec::push`）。

**偏差**

1. `list` / `markdown` 两处数据入口适配（见「两侧怎么跑」）；其余四个页面按原 TSX 输入编译。
2. `settings` 手写对照是新写的（Zig 只有 TSX 侧）；其余对照逐行移植 Zig `*_manual.zig`。
3. Rust 的 action 上下文只认根状态：手写 `local` 的行计数器用捕获的 `Rc<RefCell<u32>>`，`list` 的 item
   用 `Rc<Vec<Row>>`（Zig 用 arena 里的子结构指针）；点击/渲染的树与帧数相同。
4. `ZANT_BENCH_MESSAGES` / `ZANT_BENCH_LIST` 不适用（输入固定）；`ZANT_BENCH_REPEAT`、
   `ZANT_BENCH_MAX_RATIO`、`ZANT_FRAME_STATS=1` 支持。
5. Windows 未验证（测试机不可达）：基准只用可移植的 headless raster / 假时钟 API，但未在 Windows 跑过。

**验收命令（本 worktree 全部通过）**

- `./scripts/cargo.sh test --workspace`：72 个测试二进制 / **1066 passed, 0 failed**（新增 7 条：
  p95 下标、best-of-N、参数解析、六 app 覆盖、生成树 ↔ 对照 keyed 集合、generated counter 可点击、
  generated list 10 000 行惰性建树）；
- `./scripts/cargo.sh clippy --workspace --all-targets -- -D warnings`：0 告警；
- `./scripts/cargo.sh fmt --all -- --check`：干净；
- `./scripts/tsx/bench-tsx.sh`：**PASS** 6/6 apps、11/11 scenarios（上表；raw log
  `target/tsx/bench-tsx/run.log`）；
- `./scripts/parity/bench.sh`：**PASS 7/7**，默认模式与 `ZANT_BENCH_LIST=1` 都过（最高 typing 1.16×）；
- `./scripts/tsx/build-template-check.sh`：通过（debug 构建 + `ZANT_SNAPSHOT` 冒烟）。

## 已并入 feat/rust-port 的内容（按时间）

1. M0.2 骨架 + 模块树桩、M0.3 `zant-sys`、M0.4 对照工具、M0.5 文档。
2. WP-1.1a geometry/theme/motion/flex、WP-1.1b unicode/paths/hash（Wyhash 4.2 逐行移植 + 20 个 Zig 实测钉死值）、
   WP-1.1c renderer/image、WP-1.1d platform/dialog/events/keys。
3. WP-3.1 markdown/remend（522/522）、WP-3.5 phase 1（math symbols/fonts）、WP-4.1（TSX lexer/AST/JSX text/entities）、
   WP-3.10（db/sqlite + derive(Table)，SQL 与 Zig 逐字节一致）。
4. 早期落地（原属 1.3/2.8，为解除依赖提前做）：`core/vector.rs`、`core/icons.rs`（WP-2.3a）、`store.rs`、`Str`、
   renderer 的 path / simple-text / measure-ink 包装、`lib.rs` 的 `extern crate self as zant;`。

## 已知框架缺口（WP-1.5 parity 发现，2026-11-13）

> 1–6 已由 WP-2.1 修复（见 `rust/wp-2.1`）；记录保留供核对。

1. ~~**toggle/radio/switch 的 on_change 不派发**~~：已修复。runtime `toggle_set` / `toggle_flip` 从节点拷贝出 `IndexAction`/`BoolAction`/`Action` 后按 Zig `Tree.toggleSet` 语义调用（radio 只置 true）；pointer click 与 Space/Enter 共用。
2. ~~**按路径图片未加载**~~：已修复。`runtime/{reconcile,patch}.rs` 的 image 分支调用 `components::image::load` / `set_source`，比较 intrinsic 尺寸决定 `layout_changed`。
3. ~~**animate tick 跳过 Input/Toggle/Select**~~：已修复。`runtime/animate.rs` 调用三者 `animate(...)`，由 `Tree::poke` 只推进变化节点。
4. ~~**select 弹层 option hover/click 仍 TODO**~~：已修复。pointer move/down 用 `components::select::option_at` 命中，`paint_select_popup` 在 tree paint 之后调用 `paint_popup`。
5. ~~**rich text 选择高亮未绘制**~~：已修复。`core/tree/paint.rs` 用 `renderer::rich_text::Layout::range_rects` + 运行时 selection color。
6. ~~**input 编辑助手三处重复**~~：已修复。`core/tree/input_ops.rs` 与 `runtime/ime.rs` 改为调用 / 再导出 `components::input`。

## M6 接口约定（WP-6.1 提出，供 6.2 / 6.3 / 6.5 使用）

1. **TSX 应用编译**（给 6.2）：`zant run/compile/package` 现在只调用 `cargo build`；检测到 TS 项目（`src/App.tsx`）时直接报错，说明期望的接口是应用 `build.rs` 调用
   `zant_build::tsx_app()`（把 `src/**/*.tsx` 生成到 `OUT_DIR`、`src/main.rs` `include!`）。6.2 落地后请删除 `crates/zant-cli/src/main.rs` 的 `ensure_rust_project` 里该分支。
2. **模板目录常量**（给 6.3）：`crates/zant-cli/src/scaffold.rs` 的 `TEMPLATE_ZIG` / `TEMPLATE_TS` 是唯一模板路径；6.3 删除纯 Zig 模板后，把 `TEMPLATE_ZIG` 指向新的 Rust 模板（或让 `create` 默认 `ts` 并删掉 Zig 分支），并同步 `create` 的 `.gitignore` 文案与「下一步」提示。
3. **应用产物名**（给 6.3）：CLI 用 `cargo metadata` 读应用 `Cargo.toml` 的 bin 名，产物 `target/{debug,release}/<bin>`；模板的 `[package] name` 请与期望的二进制名一致。
4. **安装根目录**（给 6.5）：`find_root` 认定发布包布局是 `<home>/bin/zant` + `<home>/zant-ui/Cargo.toml`（zant-ui 为 Rust workspace）；`zant create` 写入的依赖路径是相对项目目录的 path 依赖。
5. **打包脚本路径**（给 6.4）：CLI 仍调用 `tools/zant/package/{macos.sh,windows.ps1}` 的既有 action，ZANT_* 变量名不变；输出目录从 `zig-out/` 改为 `target/`（`ZANT_OUT=target/dist`、`ZANT_DIR=target/<标题>`、临时文件 `target/build/`），脚本里若还写死 `zig-out` 需要同步。

## GUI 真实交互验证（2026-11-14，macOS 登录会话）

用 `scripts/mac/ui-scenario.swift`（本轮扩展：`clickfrac`/`clickat`/`scroll`/`scrollat`/`drag`/`layout`/`cmdkey` 与方向键等）
对 Rust 示例做**真实鼠标/键盘驱动**，证据读取 `[event]` stderr、`ZANT_LAYOUT_FILE` 快照差异与应用快照（系统截图权限被拒，
因此用应用自带 `ZANT_SNAPSHOT` 渲染的 PNG）。

- **zant-components**：38 个真实事件全通过 —— 9 个按钮（含 disabled 正确不触发）、输入框输入中文（逐字符事件）与密码、
  checkbox/switch/radio、select 弹层点选、segmented ×3、tabs ×2、progress ×4、菜单打开/菜单项、modal 打开/关闭、
  message、pointerArea 拖拽（pointer down/up）、虚拟列表点击+滚轮、Markdown 追加、深色切换、窗口 resize；
  弹层状态由 layout 快照证明（menu 85→83 keys、modal 94→93、message 出现 `message-pill`、vlist 24 行）。
  截图：浅/深色与 2500 高全页快照（含真实 PDF 阅读器）已检查。
- **zant-markdown**：真实点击「代码复制」→ 系统剪贴板得到代码块原文；表格 `copy-md`/`copy-csv` → 剪贴板为 Markdown / CSV；
  滚轮滚动全部块。与 Zig parity 6/6、0 像素（WP-3.11）。
- **zant-pdf**（真实 15 页 MEMGPT.pdf）：翻页（layout 页 key 变化）、缩放（页宽 868→1071）、fit 切换、搜索输入+下一个/上一个、
  页码输入跳转、⌘A+⌘C 整页文字进剪贴板（真实 PDF 文本）、批注工具切换与拖拽/撤销/重做。**工具栏改为 `ui::segmented` 后
  Rust/Zig parity 6/6（light/dark × 3 尺寸，layout 33–35 keys 全等、0 像素差）**。
- **zant-md-stream**：流式期间连续滚轮与点击，块 key 随内容增长（60→63→100 keys），完成行
  `[stream] done: 1990 bytes, 322 deltas, 14306 ms, 410 builds, build avg 0.08 ms, max 0.18 ms`。
- **zant-math**：滚动/缩放/resize 交互，应用快照渲染正确；与 Zig parity 6/6、0 像素（WP-3.11）。
- **zant-gallery**：命令式 retained-mode 示例（无 layout key / 事件日志）只能视觉+像素验收（WP-2.9：图像 parity 6/6）；
  如需交互断言，需要给 gallery 加 debug layout dump（未做，非必需）。

## 真实帧率测量（2026-11-14，Release，macOS，`ZANT_FRAME_STATS=1` + GUI 自动化）

| 场景 | Rust | Zig 基线 | 比值 | 备注 |
|---|---|---|---|---|
| components 首次布局 | 29.5 ms | 39.0 ms | 0.76× | text_layouts=204，text 21.5 ms；单次挂载帧 |
| components 滚轮滚动帧 | max 3.97 ms | max 2.46–3.51 ms | 1.1–1.6× | 绝对远低于 16.7 ms |
| components 连续 resize（16 ms 间隔） | 帧 0.84–1.33 ms，resize 延迟 1.47–1.86 ms | 帧 0.73–0.76 ms，延迟 1.25–1.61 ms | 1.08–1.34× | 延迟仍 ≪16.7 ms（约 600 fps 能力） |
| PDF 阅读器首帧 | 47.3 ms | 46.6 ms | 1.02× | layout 41.8 ms（文本层），与 Zig 同 |
| PDF 滚动帧 | max 1.59 ms | max 1.35 ms | 1.18× | 瓦片异步渲染不阻塞 UI |
| PDF 缩放重建 | 13.4 / 9.4 ms | 12.1 / 9.2 ms | 1.11 / 1.02× | 缩放时树重建，仍 <16.7 ms |
| 流式 Markdown 稳态帧 | max 2.9–3.5 ms | 2.5–3.2 ms | ~1.1× | 流式期间 `text_layouts` 不随内容增长；挂载 31.1 ms vs 36.6 ms |
| 流式整体耗时（history 8） | 6.28 s | 6.35 s | 0.99× | build avg 0.08 vs 0.06 ms |
| 流式峰值内存（history 16） | 123.9 MB | 126.3 MB | 0.98× | `/usr/bin/time -l` |

结论：稳态帧时间全部远低于 16.7 ms（≥60 fps 有余量）；与 Zig 基线的偏差在 0.76–1.34× 之间，
超过 1.25× 的只有 sub-millisecond 的 resize 单一 bucket（绝对值 ~1.8 ms），待 `zant-bench`（WP-2.11）
的正式矩阵确认后再决定是否优化。挂载/缩放帧 30–47 ms 与 Zig 一致（首帧文本整形），不是回归。

## 性能优化记录

1. **布局热路径去掉每容器每帧的 `children.clone()`**（WP-2.11 提出，lead 应用）：
   `core/layout/{intrinsic,container}.rs` 改为按索引访问子节点；bench `stream` layout 1.69 → ~1.4 ms。
2. **`key_bounds` 快照不再每帧分配字符串**：`HashMap<String, Rect>` → `HashMap<Str, Rect>`（`Str` 克隆是引用计数），
   `refresh_key_bounds` 每帧省下 ~3000 次 `String` 分配（bench 中 ~0.16 ms/帧）。
3. **布局临时缓冲池**（lead）：`core/layout` 的 `visible`/`flow`/`absolute`/`items`/`widths`/`heights`
   改为线程本地 scratch 池复用（Zig 用 arena/栈数组），`order_ids` 改为原地排序。
   这是 `stream` 场景（约 10,900 节点整树重建）的主要剩余分配来源。
4. 三轮优化后 `scripts/parity/bench.sh` **PASS 7/7（默认 + 虚拟列表）**：
   `stream` **0.89×**（Rust 1.96 ms vs Zig 2.20 ms，已反超 Zig）、`resize` 0.93×、`scroll` 0.99×、
   `typing` 1.13×、`caret`/`hover` ≈1.0×；虚拟列表模式全部 1.00–1.03×。
   所有稳态帧 0.6–4.3 ms（远低于 16.7 ms），峰值内存比 Zig 低 2%；
   同时 `zant-demo`/`zant-components`/`zant-markdown` parity 仍 6/6、0 像素。

## 端到端里程碑（WP-E2E，已并入）

`scripts/tsx/build-template-check.sh`：`zant create` 生成的 TSX 应用 / `scaffold/template-ts` 现在能
`cargo build` 出可执行文件并渲染真实窗口（截图 sha256 `88c5019e…`：Title、输入框、父组件计数、
`Dashboard` 的两个 `StatCard`（macOS / 14 核 / Rust 1.94.1，来自 `native/system` 规范+实现）、
`CounterCard` +1/Reset）。真实鼠标点击验证：点击 +1 五次后条件文本宽度 45→49、Reset 后回到 45，
无 panic（`ui::action` 现在看到 `Shell<App>` 的内层根组件，修复了点击即 panic 的问题）。
`zant create` → `cargo build` 也验证通过（新项目直接可编译）。

## 最终验证矩阵（2026-11-14）

| 项目 | 结果 |
|---|---|
| `cargo test --workspace` | 67 个测试套件全绿（含 TSX 31 个 golden 编译检查、端到端 fixture/模板） |
| `clippy --workspace --all-targets -- -D warnings` | 0 |
| `fmt --all` | 干净 |
| `zig build test` | 绿色（Zig 基线未动） |
| parity `zant-demo`/`components`/`markdown`/`md-stream` | 6/6 layout+像素 |
| parity `zant-gallery`/`zant-math`（Zig 无 key，图像对照） | 6/6 0 像素 |
| parity `zant-pdf`（fixture PDF，手动对照） | 6/6 layout+像素 |
| `scripts/parity/bench.sh` | **PASS 7/7**（默认+虚拟列表）：stream 0.89×、typing 1.16×、resize 0.93×、scroll 0.97× |
| `zant-md-stream` 流式 | 稳态帧 1–3 ms，`text_layouts` 不随内容增长 |
| `scripts/tsx/build-template-check.sh` / `build-check.sh` | 都通过（含 ZANT_SNAPSHOT 冒烟） |
| `scripts/tsx/bench-tsx.sh`（WP-5.10c） | **PASS 6/6 apps、11/11 scenarios ≤1.2×**（最高 `todos-add` 1.11×，`local-mount` 1.08×，settings 0.93×） |

## 收尾增补（lead）

- **跨文件 `createWindow`**：窗口组件的 `tsx::window::Component` 包装现在由组件**自己的模块**生成
  （项目级预扫描 `createWindow(<组件>)`，`ProjectData::window_components`），因此窗口句柄可以和组件
  分文件声明/导入；已用 tsx-demo 结构验证（`src/windows.tsx` 声明句柄、`Panel.tsx` 导入并调用
  `panel.show()`，编译通过）。单文件 `compile` 仍在本文件生成包装。
- **`ui::markdown` / `ui::MarkdownOptions` 已从 `zant::ui` 导出**，`widgets` golden 加入 golden 编译检查
  （当时 30 个 golden（32 个语料 − `desktop`/`entities`）全部编译通过；WP-COVERAGE 后 `desktop` 已加入，
  **31 个** golden 全部编译通过，仅 `entities` 按设计排除）。
- 生成的 `src/app_config.rs` 加 `#![allow(dead_code)]`（新项目 `cargo build` 零告警）；模板里的
  占位 `let _ = (…)` 已删除。
- WP-5.10：TSX 示例三件套（`zant-tsx-counter` / `zant-tsx-demo` / `zant-tsx-bench-app`）编译运行、
  截图与布局 key 断言通过；TSX 编译基准 30 个共同语料平均 **1.08–1.10× Zig（PASS）**。
- 最终校验：71 个测试套件全绿、clippy 0、fmt 干净、`build-template-check.sh` / `build-check.sh` /
  `examples-check.sh` 全通过、`zant-demo` parity 6/6、bench 闸门 7/7 PASS（两种模式）。

## CLI 端到端（本轮验证）

- `zant create` → `cargo build`：新项目直接可编译（模板 `scaffold/template-ts`）。
- `zant package`：release 编译 → 生成 AppIcon.icns → ad-hoc 签名 → `target/<标题>.app`；打出的 .app
  内二进制用 `ZANT_SNAPSHOT` 运行渲染正常（截图 `/tmp/pkg-shot/pkg.png`）。
- `zant run`：已修掉原来用 bash 进程替换过滤 macOS 输入法日志的做法（沙箱/受限环境会
  `Operation not permitted`），改为 Rust 侧管道过滤（`is_benign_log` + 后台线程），现在在沙箱里也能
  编译并启动窗口；`ZANT_RAW_LOGS=1` 仍可看原始输出。

## WP-COVERAGE：`desktop` 黄金与测试覆盖对照（2026-11-14）

分支 `rust/wp-coverage`（worktree `.worktrees/wp-coverage`）。两块独立工作：**A. 解除 `desktop` 黄金阻塞**
（`Uint8Array` → `&'static [u8]`）；**B. M8 §7.1 的测试覆盖对照**（Zig 基线 → Rust 套件，缺口与补齐）。

### A. `Uint8Array` → `&'static [u8]`（desktop 黄金）

- **类型映射**（`crates/zant-tsx/src/codegen/types.rs`）：`Uint8Array` 不再映射为 `tsx::List<u8>`，改为
  `types::bytes_type` = `rust_imports::bytes` 的 `&'static [u8]`。这与资源导入（`include_bytes!`）、
  `tsx::platform::asset_image`、`tsx::desktop::TrayOptions::{icon, icon_light}` 是**同一个 interned 类型**，
  所以「导入的 .png」「字节字面量」「`.rs` 原生模块返回的字节」互相可赋值。
- **表达式生成**（`crates/zant-tsx/src/codegen/expr.rs`）：期望类型是 `Uint8Array`（含 `Uint8Array | null`、
  声明/返回/实参、`useTray` 内联值）时，整数字面量数组发成可 const-promote 的切片
  `&[137u8, 80u8, 78u8, 71u8]`（Zig 参考输出是 `&[_]u8{ 137, 80, 78, 71 }`）；空字面量 → `&[]`。
  只接受整数字面量（spread / 运行时值给诊断：`&'static` 无法从运行时 `Vec` 得到）；越界字节由
  `types::check_literal` 报 `integer literal does not fit u8`。
- **安全性**（`model.rs`）：`model::array` 只复用 `tsx::List<…>` 条目，避免 `u8[]` 与 `tsx::Bytes` /
  `&'static [u8]` 因 intern 顺序串型（旧实现里的潜在隐患）；未标注类型的字节字面量 → 明确诊断
  `annotate byte literals with const name: Uint8Array = [..]`。
- **框架 API 未改**：`tsx::desktop::TrayOptions::icon` 原本就是 `&'static [u8]`；没有新增字节 API。
  运行时字节（剪贴板图片）仍是 `tsx::Bytes` = `Rc<[u8]>`（`clipboard.image()` 的返回类型）；
  `setImage` 收 `impl AsRef<[u8]>`，静态切片和 `Bytes` 都能传。`Uint8Array` 注解不能承接
  `clipboard.image()` 这类 `tsx::Bytes` 值（`&'static` 不能借用 `Rc`）——已在下方待决问题登记文档偏差。
- **黄金**：新生成并人工审阅 `crates/zant-tsx/testdata/desktop.rs`（与 `tools/tsx/testdata/desktop.zig`
  逐段对照：`icon` 常量、`useTray` 三个回调、`useHotkey` 字段、`on_app_event`）；`tests/golden.rs` 的
  `NO_GOLDEN` 只剩 `entities`，31 个黄金一起写入临时 crate 后 `cargo check` 通过（**31/31**，
  `src/desktop.rs` 参与编译，仅剩生成代码的 clippy 风格 warning）。`entities` 保持排除（UTF-8 不能表示
  未配对代理项，见待决问题 4）。

**desktop 黄金证据**（`./scripts/cargo.sh test -p zant-tsx --test golden` → 5 passed）：

```text
temp crate: target/zant-tsx-goldens/src/main.rs  ->  31 个 #[path = "….rs"] 模块（含 desktop.rs）
$ cargo check   # workspace-external crate，zant = { path = crates/zant }
warning: src/desktop.rs:13:16: constant `_icon_VALUE` should have an upper case name
warning: src/desktop.rs:15:35: using `.clone()` on a double reference …
    Finished `dev` profile …
```

生成的关键代码（`crates/zant-tsx/testdata/desktop.rs`）：

```rust
fn icon() -> &'static [u8] {
    thread_local! {
        static _icon_VALUE: &'static [u8] = &[137u8, 80u8, 78u8, 71u8];
    }
    _icon_VALUE.with(|value| value.clone())
}
…
let shown: bool = tsx::desktop::update(tsx::desktop::TrayOptions { icon: icon(), … });
```

### B. 测试覆盖对照（M8 §7.1 闸门）

口径：Zig 侧在**冻结的 Zig 树**上实测 `grep -rn 'test "'`（`src/**/*.zig`、`tools/**`、`tests/**`；
不含 M8 要删除的 legacy `scaffold/template/`）；Rust 侧是 `cargo test --workspace` 的
`test result` 行汇总；夹具（remend、mhchem）按「一个测试遍历全部案例」计，**不是**每个案例一个测试。

| 项 | 数量 |
|---|---|
| Zig `test` 块（`src` 88 + `tests` 151 + `tools/tsx` 113 + `tools/zant` 7） | **359** |
| M0 记录 Zig 基线（6 个测试二进制：tests 124 / framework 99 / CLI 8 / TSX 149） | 380 |
| Zig 数据驱动夹具 | remend **522** 条、mhchem **823** 条（各由 1–2 个测试逐条断言） |
| Rust（本 worktree） | **74 个测试套件 / 1076 passed / 0 failed**（35 条 doc 示例 `ignore`） |
| **闸门** | **PASS**：1076 ≥ 359（也 ≥ M0 的 380）；522 / 823 条夹具都在测试里逐条断言 |

> M0 的 380 是按测试**二进制**记的，和今天按文件数 `test` 块的分组（tests 124→151、TSX 149→113）口径不同
> （迁移期间测试在 WP 之间移动/新增）；两种口径 Rust 都远高于基线，闸门不受影响。

#### `src/**` 单元测试（Zig 88 块 → Rust `zant` lib 525 条 + TSX 运行期契约 13 条）

| Zig 区域 | Zig 块 | Rust 对应 | Rust 条数 |
|---|---|---|---|
| `src/core/**`（11 个文件） | 11 | `zant` lib `core::*` | 60 |
| `src/layout/flex.zig` | 5 | `zant` lib `layout`（24 条 `tests/flex_test.rs` 见下表，同一份 Zig 用例） | 5 |
| `src/markdown/**`（13 个文件） | 31 | `zant` lib `markdown::*` | 45 |
| `src/math/**` | 5 | `zant` lib `math::*` | 21 |
| `src/db/**` | 2 | `zant` lib `db::*` | 2 |
| `src/components/widgets.zig` | 1 | `zant` lib `components::*` | 103 |
| `src/desktop.zig` | 1 | `zant` lib `desktop::*` | 24 |
| `src/tsx*.zig`（13 个文件） | 32 | `zant` lib `tsx::*` + `tests/tsx_runtime_test.rs` | 63 + 13 = 76（含运行期契约） |
| `src/renderer` / `platform` / `ui` / `runtime` / `theme` / `store` / `image` / `str` / `error`（Zig 无单测块） | 0 | `zant` lib 其余模块 | 202 |
| **合计** | **88** | | **525**（lib）+ `tsx_runtime_test` 13 |

#### `tests/**` 集成测试（Zig 151 块 → Rust `zant` 集成 147 条 + TSX 端到端/运行期覆盖）

| Zig 文件 | Zig 块 | Rust 套件 | Rust 条数 | 备注 |
|---|---|---|---|---|
| `tests/flex_test.zig` | 24 | `tests/flex_test.rs` | 24 | 一对一 |
| `tests/antd_components_test.zig` | 12 | `tests/antd_components_test.rs` | 12 | 一对一 |
| `tests/markdown_test.zig` | 12 | `tests/markdown_test.rs`（+ `markdown_render_test.rs` 5） | 12 (+5) | 表格/任务列表断言内联，无独立夹具文件 |
| `tests/motion_test.zig` | 10 | `tests/motion_test.rs` | 13 | 10 条同名移植 + 3 条新场景（WP-2.7） |
| `tests/view_test.zig` | 7 | `tests/view_test.rs` | 7 | |
| `tests/selection_test.zig` | 6 | `tests/selection_test.rs` | 6 | |
| `tests/math_test.zig` | 6 | `tests/math_test.rs` | 6 | |
| `tests/lifecycle_test.zig` | 6 | `tests/lifecycle_test.rs` | 6 | |
| `tests/ui_test.zig` | 5 | `tests/ui_test.rs` | 5 | |
| `tests/component_test.zig` | 5 | `tests/component_test.rs` | 4 | 1 条画廊命令式 App 用 `gallery_smoke.rs` 覆盖（WP-2.10 已登记） |
| `tests/db_test.zig` | 4 | `tests/db_test.rs` | 6 | |
| `tests/text_test.zig` | 3 | `tests/text_test.rs` | 3 | |
| `tests/pointer_test.zig` | 3 | `tests/pointer_test.rs` | 3 | |
| `tests/pdf_viewer_test.zig` | 3 | `tests/pdf_viewer_test.rs` | 7 | |
| `tests/list_test.zig` | 3 | `tests/list_test.rs` | 3 | |
| `tests/local_state_test.zig` | 2 | `tests/local_state_test.rs` | 2 | |
| `tests/all.zig` | 2 | `tests/all_test.rs` | 2 | |
| `tests/remend_test.zig` | 2 | `tests/remend_test.rs` | 2 | 其中 1 条遍历 522 条夹具 |
| `tests/text_align_test.zig` | 1 | `tests/text_align_test.rs` | 1 | |
| `tests/store_test.zig` | 1 | `tests/store_test.rs` | 1 | |
| `tests/pdf_test.zig` | 1 | `tests/pdf_test.rs` | 6 | |
| `tests/mhchem_render_test.zig` | 1 | `tests/mhchem_render_test.rs` | 1 | 遍历 823 条夹具做排版检查 |
| `tests/tsx_*_test.zig`（28 个文件） | 32 | 见下方缺口 2 | — | 编译/运行期分开覆盖 |
| **合计** | **151** | | **147 + 编译/运行期覆盖（缺口 2）** | |

#### `tools/**` 与 Zig 没有的 Rust 侧

| Zig 来源 | Zig 块 | Rust 对应 | Rust 条数 |
|---|---|---|---|
| `tools/tsx/**`（31 个文件） | 113 | `zant-tsx`：lib 137 + `zant-tsx-tags` 7 + 集成 143（`codegen_app` 29 / `codegen_bytes` 9 / `codegen_components` 8 / `codegen_core` 28 / `codegen_jsx` 39 / `codegen_strings` 2 / `codegen_react_compat` 5 / `generated_tables` 7 / `golden` 5 / `native_specs` 6 / `tsx_end_to_end` 5） | **287** |
| `tools/zant/**`（7 个文件） | 7 | `zant-cli` 36（29 单元 + 7 集成）；`zant-build` 23（10 单元 + 13 集成，Rust 新增职责） | 36 + 23 |
| `scaffold/template/src/**`（legacy Zig 模板） | 2 | M8 删除；`scripts/tsx/build-template-check.sh` / `create-smoke.sh`（不在 cargo test） | — |
| （Zig 无对应）`zant-sys` | 0 | `tests/prebuilt.rs` 11 + `tests/smoke.rs` 2 | 13 |
| （Zig 无对应）`zant-macros`（`derive(TsxProps)` / `derive(Table)`） | 0 | lib 单元测试 | 8 |
| （Zig 无对应）`zant-examples` | 0 | bench/demo/tsx-bench bin 11 + 6 个 smoke 集成套件 23 | 34 |
| （Zig 无对应）3 个 TSX 示例 crate | 0 | 每个 crate 1 条 | 3 |
| doc 示例 | — | 全部 `ignore`（35 条，不计入 passed） | — |

#### 夹具（数据驱动用例）

| 夹具 | 案例数 | Zig 断言 | Rust 断言 |
|---|---|---|---|
| `tests/fixtures/remend_cases.json`（Streamdown remend） | **522** | `tests/remend_test.zig` 1 个测试逐条比较 522 条 | `crates/zant/tests/remend_test.rs::remend_conformance_with_streamdown_test_suite` 逐条比较 522 条（0 失败），另有 1 条数学括号回归；`include_str!` 共用同一 JSON |
| `src/math/mhchem_cases.json`（KaTeX mhchem.js） | **823** | `src/math/mhchem.zig` 1 个测试逐条比较 TeX；`tests/mhchem_render_test.zig` 再逐条排版 | `crates/zant/src/math/mhchem/tests.rs::mhchem_fixture_generated_by_the_real_mhchem_js` 逐条比较 823 条；`crates/zant/tests/mhchem_render_test.rs` 逐条排版检查；`FIXTURE_JSON` 共用同一 JSON |
| 表格 / 任务列表 / setext / 引用等 | 无独立 JSON | `tests/markdown_test.zig` 的 12 个块内联 | `crates/zant/tests/markdown_test.rs` 的 12 条内联断言（`markdown_table_and_task_list` 等） |

### 缺口（Zig 块没有逐条 Rust 对应，含原因）

1. **`entities` 黄金**（`tools/tsx/testdata/entities.tsx`）——`&#xD800;` 等未配对代理项；Rust 的 `zant::Str` 是
   UTF-8，编译器按设计报 `string literal is not valid UTF-8; Rust cannot represent an unpaired surrogate`。
   `tests/golden.rs::entities_reports_its_documented_limitation` 锁定该诊断；改成 U+FFFD 会改数据，需先改设计文档 §2。
2. **`tests/tsx_*_test.zig` 32 块（28 个文件）运行期驱动**——这些 Zig 测试在测试里调用 Zig 编译器把 TSX 编成
   Zig 并跑帧。Rust 侧等价物只能是**编译期**的（黄金 byte-compare + 批量 `cargo check`）或**独立二进制**的运行期：
   - 编译：31 个黄金（`tests/golden.rs`，desktop 已加入）+ `tests/tsx_end_to_end.rs` 5 条（`tests/tsx/**` 的
     hooks/crossfile/rust_types/media fixture 移植）；
   - 运行期契约：`crates/zant/tests/tsx_runtime_test.rs` 13 条（生成代码依赖的运行时 API）；
   - 运行期整应用：`scripts/tsx/examples-check.sh` 无窗口跑 `zant-tsx-counter` / `zant-tsx-demo` /
     `zant-tsx-bench-app`，断言 PNG 非空白/尺寸/确定性/`ZANT_LAYOUT_FILE` keyed 控件（**不在 cargo test 内**）。
   不在 `crates/zant-tsx/tests` 里 1:1 移植的原因：生成的 Rust 只能在测试**外**由 rustc 编译进二进制，
   而生成应用的 crate（`crates/zant-examples/tsx/**`）属于 WP-5.10，不在本 WP 的写权限内。
3. **`tests/component_test.zig:34`**「gallery button click, input editing, composition and focus」——
   驱动 Zig 命令式 `App`（Zig 专用基础设施），已由 `crates/zant-examples/tests/gallery_smoke.rs` 覆盖；
   WP-2.10 已登记为不移植。
4. **`src/tsx.zig` 的状态 arena 7 条**（`string state owns its values`、composite / optional / all-null /
   empty / null-only state）——**已过时**：Rust 生成的状态是组件结构体的普通字段（`Option<T>` / `Vec<T>` /
   `zant::Str`），没有 arena / retired snapshot / sweep 机制；typed callback 落为
   `tsx::hooks::tests::typed_callbacks_pass_their_arguments_through`。
5. **`tools/tsx/codegen/*_test.zig` 的诊断块**：Rust 侧按区域**合并**在 `tests/codegen_core.rs` /
   `codegen_jsx.rs` / `codegen_app.rs` / `codegen_components.rs`，不是一块对一条（见各文件头注释）。
   本 WP 补了 `stores_test.zig` → `codegen_components.rs::store_diagnostics`、`strings_test.zig` →
   `tests/codegen_strings.rs`、`react_compat_test.zig` → `tests/codegen_react_compat.rs`；
   `emission_test.zig` / `errors_test.zig` / `widgets_test.zig` / `zant_api_test.zig` 的个别断言仍只由合并
   测试覆盖（功能代码有测试，但不保证每条 Zig 断言都有同名 Rust 测试）。若 lead 要求严格逐块，需要按
   Zig 用例逐条补，属后续工作。
6. **`scaffold/template/src/**` 2 条**——legacy Zig 模板，M8 删除。
7. **文档偏差（不在本 WP 文件范围）**：`docs/tsx.md`「字节」一节仍写 `Uint8Array` 对应 `tsx::Bytes`，
   注解类型现在是 `&'static [u8]`；同一句「资源导入」写成 `const tray_icon: &[u8]`。
   建议 lead 改成：`Uint8Array` 注解/字面量/资源导入 = `&'static [u8]`；运行时字节（剪贴板图片）仍是
   `tsx::Bytes`，两者都满足 `setImage(impl AsRef<[u8]>)`。

### 本 WP 新增 / 移植的测试（file:test name）

- `crates/zant-tsx/testdata/desktop.rs`（新黄金；`tests/golden.rs` 加入 31 个编译集）+
  `tests/golden.rs::golden_outputs_match_the_compiler` / `generated_goldens_compile_against_the_runtime`
  覆盖 `desktop`；`tests/golden.rs::entities_reports_its_documented_limitation`（原 desktop+entities 测试改名）。
- `crates/zant-tsx/tests/codegen_bytes.rs`（9 条，新文件）：
  `a_uint8array_literal_is_a_promoted_static_slice`、`a_uint8array_literal_is_accepted_inline`、
  `an_empty_uint8array_literal_is_an_empty_slice`、`an_imported_asset_is_a_uint8array`、
  `a_rust_module_byte_function_is_a_uint8array`、`an_untyped_byte_literal_points_at_the_annotation`、
  `an_out_of_range_byte_literal_is_rejected`、`a_uint8array_literal_cannot_hold_other_values`、
  `a_uint8array_literal_cannot_hold_a_spread`。
- `crates/zant-tsx/tests/codegen_components.rs::store_diagnostics`（移植 `stores_test.zig`「store diagnostics」）。
- `crates/zant-tsx/tests/codegen_strings.rs`（新文件，移植 `strings_test.zig` 2 块）：
  `string_method_diagnostics_stop_unsupported_calls`、
  `string_helper_temporaries_preserve_source_evaluation_order`。
- `crates/zant-tsx/tests/codegen_react_compat.rs`（新文件，移植 `react_compat_test.zig` 5 块）：
  `react_namespaces_lower_like_plain_hooks_and_fragments`、`named_react_imports_share_the_zant_output`、
  `root_shorthands_and_react_fragments_emit_the_same_native_root`、
  `children_typed_one_element_or_several_take_one_child_or_many`、`react_import_diagnostics`。
- `crates/zant-tsx/tests/codegen_app.rs`：`KNOWN_MISSING` 去掉 `desktop`（现在能编译），`platform` 的原因改成
  stub JSX 的 `padding={{…}}` 对象属性（真实 WP-5.4 生成器下 `platform` 黄金可编译），不再是 bytes 冲突。

### 验证（本 WP 实跑）

| 命令 | 结果 |
|---|---|
| `./scripts/cargo.sh test --workspace` | **74 个测试套件 / 1076 passed / 0 failed**（35 ignored 全是 doc 示例） |
| `./scripts/cargo.sh clippy --workspace --all-targets -- -D warnings` | 0 告警 |
| `./scripts/cargo.sh fmt --all -- --check` | 干净 |
| `ZANT_ZIG_BIN=/Users/xiaoyi/dev/XyUI/zig-out-baseline/bin ./scripts/parity/compare.sh zant-demo` | **6/6 PASS**（layout 8 keys ±0.01，0/3 840 000 像素差） |
| `./scripts/tsx/build-template-check.sh` | `scaffold/template-ts` end-to-end: **OK**（debug build，含无窗口快照） |
| Zig 改动 | 无：`git status --porcelain -- '*.zig'` 为空（未跑 `zig build test` 也不需要，Zig 树未动） |

## M5 验收达成（本轮）

- `zant-tsx-counter / -gallery / -todos / -reader / -app / -notes` 六个 TSX 示例均真实构建、运行并与
  Zig 基线对照：layout 完全一致，**gallery/todos/reader/notes 0 像素**（gallery 的 21px 尾随 `.` 由
  WP-TSX-BUGS 修复）；`scripts/mac/tsx-counter.steps` 全步骤通过。
- `bench-tsx`：TSX 生成代码 vs 手写 Rust 对照组，**全部 ≤1.11×（闸门 1.2×）**，6 应用 / 11 场景 PASS。
- WP-TSX-BUGS 修复：子组件回调 prop 现在**同步调用**（不再经 `ui::effect` 丢失）、`Option<Node>` 裸元素、
  rich text 尾随文本、非根组件 `<List>`（含子组件行内 action 捕获自身状态）。
- 迁移后测试：Rust 1092 passed / 0 failed；Zig 基线 359 个 test 块 + remend 522 / mhchem 823 夹具全部覆盖
  （`WP-COVERAGE` 对照表）。

## Windows 真机验证（2026-11-14，测试机恢复）

机器：`192.168.31.197`（WIN-AUA9GNCCOI7，`E:/dev/XyUI`，Rust 1.96.1 MSVC、Zig 0.17.1、Skia/PDFium 预编译齐全）。
`scripts/win/{remote,sync}.sh` 已支持 `ZANT_WIN_SSH_OPTS`（沙箱 known_hosts/进程替换限制）。

已修复的 Windows 专属缺陷：
1. `crates/zant-sys/build.rs`：`canonicalize()` 的 `\\?\` 谓词前缀导致 cl.exe `C1083`——加 `plain_path()` 剥离。
2. `crates/zant-sys/build.rs`：生成的 `static_data.rs` 用 `//!` 内层文档注释但被 `include!` → `E0753`——改 `//`。
3. **窗口应用崩溃**（`SkIcuLoader: datafile missing icudtl.dat`）：Windows 窗口化路径没调用
   `zant_sys::init()`（只有光栅测试路径调了）——在 `platform.rs` 的 Windows 分支补上；`zant-demo.exe` 正常出窗+截图。
4. 测试平台差异：`shortcut_list` 宽度断言按平台（macOS 250 / 其他 310）。

## 待决问题

### WP-5.9 提出（2026-11-14）

**黄金用例与编译验证**（分支 `rust/wp-5.9`）：

- `tools/tsx/testdata/*.tsx` 32 个里 **30 个**有 `crates/zant-tsx/testdata/<name>.rs`（由新的 Rust 编译器生成后逐个对照
  `testdata/<name>.zig` 行为与 `docs/rust-port/tsx-codegen.md` §2/§3/§5/§6 审阅）；`desktop`、`entities` 无法生成 `.rs`
  （见下），`widgets` 有黄金但由于框架 API 缺失不参与 `cargo check`。
- 编译检查：30 个黄金中 29 个写进一个临时 crate（`zant = { path = <repo>/crates/zant }`，单个 `[workspace]`，复用
  `target/zant-tsx-goldens/target` 缓存）后 `cargo check` 通过；首次约 11s，复跑约 0.2s。
- 这是生成代码第一次真正通过 rustc：期间修复的编译器 bug 记录在下面第 2 条，**其中与 WP-5.4/5.5/5.6 联调相关的
  接线缺口全部在编译器一侧修好**（未改 `crates/zant`）。
- 端到端：`tests/tsx/**` 的 `hooks`、`crossfile`、`zig_types`（移植为 `rust_types`）、`media` 复制到
  `crates/zant-tsx/tests/fixtures/`，Zig 模块换成等价的 `.rs` 镜像（`data.rs`/`thumbs.rs`/`devices.rs`/`jobs.rs`），
  用 `compile_project_root_with` 编译并断言跨文件组件/类型/store/常量/资源/`useWake` 的输出。

**偏差表（Zig 参考 → Rust 生成，位置 `crates/zant-tsx/testdata/`）**：

| Zig 输出 | Rust 输出 | 原因 |
|---|---|---|
| `[]const u8`、`[]const []const u8` | `zant::Str`、`tsx::List<zant::Str>` | plan §4 / codegen §2 类型映射 |
| `"x"`（comptime 字面量） | `zant::Str::from("x")`（Zig 是裸字面量） | `.into()` 在无期望类型处（`if`/`match` 分支、宏参数）无法推断；`Str::from` 无歧义且仍是 `Static` |
| 字符串拼接 `a + b` | `zant::Str::copy(&format!(..))` | 同上 |
| 非 Copy 变量每次读取 | `.clone()`（§3 所有权规则） | 同上 |
| `ui.action(self, Class.method)` | `ui::action(|s: &mut App| s.method())`；子组件用 `Action::new` + `Weak` upgrade；读写 render 局部变量的 handler 用 `_CaptureN { .. }` + `|s| s.method(s, &_capture)` | Rust 闭包/所有权；设计文档 §5 |
| `_set_name(self, []const u8)` | `fn _set_name(&mut self, value: &str)`（方法体 `zant::Str::copy(value)`） | `ui::TextAction` 传 `&str` |
| `switch` | 带标签的 `if/else if` 链 | codegen §7（case 求值顺序） |
| C 风格 `for` | `loop { update; if !cond { break } body }` | 保证 `continue` 也执行 update |
| `for (slice) |x|` | `for x in list.as_slice().iter().cloned()` / 迭代器适配器 `.map/.filter_map/.fold` + 显式类型闭包参数 | Rust 无切片循环；`forEach` 等在列表上就地重建 |
| Zig comptime `const`（`&[_]_`/`comptimePrint`） | `fn name() -> T { thread_local!{static _NAME_VALUE: T = …}; …clone() }`，读取处 `name()` | Rust `const` 不能分配；`Str`/`List` 非 `Sync` 不能用 `static` |
| 组件文件里 `const Props = XProps` | 模块级 `pub type <Component>Props_ = XProps;`（无属性类型名则 `#[derive(Default)] pub struct <Component>Props_;`） | Rust 无稳定 inherent associated type；模块级 `Props` 会重名 |
| Zig `?T` / `orelse` | `Option<T>` / `unwrap_or`/`unwrap_or_else`；可选成员 `?.` → `.map(..)`；`!` → `.unwrap()` | codegen §2 |
| Zig enum（`@tagName`） | `#[derive(..)] pub enum` + `as_str()`；外部 `.rs` 模块的 enum 必须 PascalCase 变体（TSX 字面量同名） | Rust 命名与 `as_str` 约定 |
| `tsx.Callback(fn (..) void)`（同步回调） | `std::rc::Rc<dyn Fn(&mut ui::ActionCx<'_>, ..)>`；在 handler 里调用会经 `ui::effect` **延后一帧**（与 ui event 形态的 callback prop 同一处理） | 设计缺口：Rust 闭包不能捕获 `&mut App`；WP-5.5 报告已提出，**需 lead 裁定**（见待决问题 5） |
| `tsx.Store(T)`（`&self` get/read/set/update） | `thread_local! { static X: RefCell<zant::store::Store<T>> }` + `x_get/x_read/x_set/x_update` | 框架只有 `zant::store::Store`（`&mut` 方法），编译器侧用 `RefCell` 适配，未改框架 |
| `ui.markdown`/`ui.PdfViewer` 等 | `ui::markdown`/`ui::pdf_viewer`（黄金输出一致） | 框架尚未从 `ui` 导出，见待决问题 1（`widgets` 因此不编译） |
| `tsx.timers.set(ms, ctx, Class.method)` | `tsx::timers::set(ms, ui::action(|s| s._timeoutN(s)))`（capture 以 `_TimeoutCaptureN` 传入） | 运行时 API 已是 `set(ms, Action)`；Zig 的 `timers.zig` 无 Rust 模块归属，暂落在 `events.rs` |
| `tsx.callback(fn(..), self, Class.method)` | 同上 `Rc<dyn Fn(&mut ActionCx,..)>` + `ui::effect` 排队 | 同上 |
| `console.log(..)` → `std.debug.print(pattern, .{args})` | `tsx::builtins::log(&[tsx::Any::from(zant::Str::copy(&format!(pattern, args)))])` | 运行时 `log(&[Any])`（自行补换行；生成代码去掉末尾 `\n`） |
| `Math.max()/min()`（无参） | `tsx::builtins::extrema::<T>(is_min, &[])`（显式类型参数） | 空切片无法推断 `T` |
| `tsx.concat` / `&.{}` | `nodes![..]`（`Vec<Node>`；`children: ReactNode[]` 的 `tsx::List<Node>` 用 `.as_slice().to_vec()`/`flatten()`） | 运行时 `nodes!` + `IntoNodes` |
| `ui.withKey` + `tsx.childId(parent, name, key)` | `ui::with_key(..)` + `tsx::child_id(parent, name, (key).as_str())`（字符串 key）/数字 key 直接传（`ChildKey`） | 运行时 `ChildKey` 只实现 `&str`/`String`/数字，非 `Str` |
| `useTheme()` 字段 `motionDurationMid` | `ui::current_theme()` → `zant::theme::tokens::Theme`，字段 snake_case（`motion_duration_mid`） | 运行时的类型路径与字段拼写；`zant::theme` 未 re-export `Theme`（待决 3） |
| `PrettyPlease` 格式 | `nodes![…]` 等宏参数由 prettyplease 原样打印 token，可能出现 `crate ::row` 这类空格；语义/可解析性不变 | prettyplease 宏内容不做格式化 |

**WP-5.9 修复的编译器 bug（都补了/更新了测试）**：

1. **WP-5.4↔5.5 接线缺口**（黄金一跑就暴露）：
   (a) `jsx::emit` 不派发子组件（children/`jsx_children` 直接调 `jsx::emit`，绕过 5.5 的 `emit_jsx` 包装）→ 组件标签全部
   “unknown tag”；(b) `captures_read` 钩子从未安装、`captures::read` 签名与 `Hooks` 不一致；(c) `events::event`/
   `event_value` 未调用 `components::forwarded_prop`（`onClick={props.onSave}` 全部失败）；(d) 事件身份按 AST 地址
   记录，但 `spread`/对象字面量会 clone AST，地址失配 → 改成按源码位置；(e) `reset_component` 不清 `g.events`（跨组件
   残留）；(f) `generate::action_value` 子组件分支 `(, value)` 多逗号、captures 发现后 receiver 仍无 `cap`；(g)
   `add_click` 等数组回调的参数顺序/`map` 双参数/`reduce` fold 形态/闭包显式类型。
2. **类型解析缓存**：`resolved_types` 以 `Box<TypeDetail>` 地址为键，但 `model::named` 解析的是 type decl 的 clone，
   地址释放后复用 → `arrays.tsx` 的 `Box[]` 解析成 `List<f64>` 等错类型；改为按 `TypeDetail` 结构作键，并把类型声明
   原地取出解析后放回（同 Zig 的指针稳定语义）。
3. **Rust 关键字转义**：`box`（以及 `gen`/`abstract`/`do`/`final`/…）没有转义，参数名 `box` 使生成代码不可解析。
4. **字符串方法**：标签块少 `'`（`label: {..}` → `'{label}: {..}`）、运行时函数名 `to_lower_case`/`to_upper_case`、
   `slice/substring` 的 `end: Option<f64>`、receiver 多余 `.clone()` 造成的类型歧义、`join` 分隔符 `&str`；
   `utf16_length`/`text::span`/`text::links`/`list_index`/`titlebarInsets`/`theme` 等运行期签名对齐。
5. **常量**：Rust `const` 不能分配/`format!`，`Str`/`List` 常量改为 `thread_local!` accessor 函数（见偏差表）；
   跨文件导入无类型注解的常量按字面量推断类型（`export const rowPrefix = "#"`）。
6. **子组件形态**：模块级 `pub type Props` 冲突 → 每组件 `<Name>Props_`；builder 的 `_props` 未赋值；props 读取未
   `.clone()`；省略的可选 props 加 `..Default::default()`；ui-action 回调字段的 `Default` 用了 `Rc` 闭包；裸可选
   布尔属性生成 `Some(true)`；`List` 的 `item_height/height` 可选字段。
7. **事件/回调**：非 ui-event 签名的 callback prop 需要运行时上下文 → 生成
   `Rc<dyn Fn(&mut ui::ActionCx<'_>, …)>`，handler 内调用经 `ui::effect` 排队（`children`/`named_handlers`/`crossfile`
   因此可编译；**设计偏差，待 lead 确认**）；callback 调用参数在 `'static` 闭包外先求值；嵌套 component function
   调用补 `self`/`&capture`；text action 参数按 `&str` 生成再 `Str::copy`；零参 handler 的占位参数类型。
8. **闭包/捕获**：嵌套闭包继承外层可见 locals（`items.map(item => …` 里的 handler 能读到 render 局部变量）；被
   narrowing 的局部按 `unwrap()` 后的类型捕获；capture 结构体与 `cap.field` 读取对非 Copy 值 clone；timer 回调
   在 effect 里捕获时用 effect 的 `cap.field` 作为 source。
9. **子节点**：`nodes![..]` 里 `Option<Node>` 的 guard 要 `Some(node)`；`tsx::List<Node>`/`List<Option<Node>>` →
   `Vec<Node>`；`components` 的 `children: List<Node>` 包 `tsx::List::from_vec(nodes![..])`；结构化 prop 列表只在有
   omitted-default 字段时加 `..Default::default()`（`ui::TabItem` 没有 `Default`）。
10. **其它**：生成文件头补 `use zant::nodes;`；`console.log` 对齐 `log(&[Any])`；`parseInt/parseFloat` 的 `&str` 与
    radix `f64`；`toFixed` 位数 `f64`；`extrema` 显式类型参数；store 生成器在 `RefCell<zant::store::Store<T>>` 上适配
    且 `x_set(v)` 少了调用的 `()`；effect 的 cleanup 字段在 `captures::prepare` 前设置（否则 `return () => …` 报“void
    functions cannot return a value”）；依赖数组单元素生成 `(x,)`；`ref.current++` 对 f64 用 `1.0`；窗口组件
    `on_close_request` 转发的方法名（`_window_on_close_request`）；`useWake` 的 `on_wake`；`setTimeout`/`clearTimeout`
    扫描与降级（见偏差表）。

**待决问题 / 缺失框架 API（给 WP-5.10 / lead）**：

1. **`zant::ui` 缺少组合组件导出**：`ui::markdown` / `ui::MarkdownOptions`（实际在
   `zant::components::widgets::{markdown, MarkdownOptions}`），`ui::pdf_viewer` / `ui::PdfViewerOptions`（**框架里根本没有**，
   目前只在 `crates/zant-examples/src/components_app/pdf_viewer.rs`）。黄金 `widgets.rs` 因此不参与 `cargo check`；
   补上导出/组件后把 `tests/golden.rs` 的 `NOT_COMPILED` 去掉。这与 status 里 WP-2.5「组合组件未从 ui::* 导出」是同一件事。
2. **`Uint8Array` ↔ `&'static [u8]`**：**已解决（WP-COVERAGE）**——裁定采用 `Uint8Array` = `&'static [u8]`
   （与资源导入 / `TrayOptions::icon` / `assetImage` 同一个 interned 类型），字节字面量生成 const-promote 的 `&[..]`；
   `desktop` 黄金已生成并进入编译检查。运行时剪贴板字节仍是 `tsx::Bytes`（`Rc<[u8]>`），`Uint8Array` 注解不承接
   `clipboard.image()` 的返回值（文档偏差登记在 WP-COVERAGE 缺口 7）。
3. **`zant::theme::Theme` 未 re-export**：编译器暂时生成内部路径 `zant::theme::tokens::Theme`（可编译）。若希望保持
   `zant::theme::Theme` 稳定路径，请在 `crates/zant/src/theme/mod.rs` 加 `pub use tokens::Theme;`，然后
   `codegen/model.rs::theme_type` 可改回。
4. **`entities.rs` 无法生成**：`tools/tsx/testdata/entities.tsx` 有 `&#xD800;` 等 lone surrogate，Zig 版用 WTF-8 原样
   输出；Rust 的 `zant::Str` 是 UTF-8，编译器按设计报 `string literal is not valid UTF-8; Rust cannot represent an
   unpaired surrogate`。`tests/golden.rs` 把这条诊断作为预期行为锁定
   （`entities_reports_its_documented_limitation`；`desktop` 已由 WP-COVERAGE 解除排除，`NO_GOLDEN` 只剩 `entities`）；
   若决定改成 U+FFFD 替换需先改设计文档 §2。
5. **非 ui-event callback prop 的 `ActionCx` 形态（需裁定）**：生成 `Rc<dyn Fn(&mut ui::ActionCx<'_>, ..)>` 并在 handler
   里经 `ui::effect` 延迟一帧调用（Zig 的 `tsx.Callback` 是同步的、带 self 指针）。这是 WP-5.5 报告的待决设计缺口；
   本 WP 为了让 `children`/`named_handlers`/`crossfile` 编译并端到端跑通先这样落地，**没有改框架**。若 lead 选择别的
   方案（例如 `ui` 增加“handler 内调用 Action”的 API），需同步改 `components.rs::callback_type`、
   `generate::callback_value`、`components::try_call` 三处，并重新生成黄金。
6. **`timers.zig` 无 Rust 模块归属**：本 WP 把 `setTimeout`/`clearTimeout` 的扫描与降级放在
   `codegen/events.rs`（`scan_timer`/`timer_call`），用运行时已有的 `tsx::timers::{set,clear}`。建议后续 WP 拆到
   `codegen/timers.rs`（需要 `lib.rs`/`codegen/mod.rs` 加模块声明，属 lead 文件）。
7. **更新了其它 WP 的测试期望**（编译器输出按上面的设计变化）：`tests/codegen_core.rs`/`codegen_jsx.rs`/
   `codegen_components.rs`/`codegen_app.rs` 与 `tests/snapshots/*.rs`（`Str::from`、`{Component}Props_`、常量函数、
   依赖元组、数组闭包类型、span `&str`、`list_index` 不再 `Some`、`extrema::<T>`、store accessor 等）。快照用
   `ZANT_TSX_WRITE_SNAPSHOTS=1` 重新生成并人工核对。

### WP-5.10 提出（2026-11-14）

**1. 跨文件窗口组件缺 `impl tsx::window::Component`（`crates/zant-tsx/src/codegen/windows.rs`）**
`docs/tsx.md`「多窗口」和 Zig `examples/tsx/app` 的写法是 `App.tsx` 里 `createWindow(Panel, {…})`、`Panel`
从 `./components/Panel` 导入。Rust 生成器对 imported 组件走 `component.external` 分支，直接用
`crate::panel::Panel` 做 `Window` 的参数；`impl tsx::window::Component for Panel` 只在**定义组件的文件**里
生成（`collect()` 只找本文件的 `createWindow`），于是：

```
error[E0277]: the trait bound `panel::Panel: WindowComponent` is not satisfied
  --> $OUT_DIR/App.rs:35: static PANEL : …tsx::window::Window<crate::panel::Panel>> = …
error[E0599]: the method `toggle` exists for mutable reference
  `&mut zant::tsx::window::Window<panel::Panel>`, but its trait bounds were not satisfied
```

Repro：把 `crates/zant-examples/tsx/tsx-demo` 的 `export const panel = createWindow(Panel, …)` 从
`src/components/Panel.tsx` 移回 `src/App.tsx`（`Panel` 仍 `import { Panel } from "./components/Panel"`）。
建议修法：`collect()` 的 external 分支在调用方文件里生成 wrapper（`pub struct PanelWindow { _s:
Rc<RefCell<crate::panel::Panel>> }` + `impl tsx::window::Component`，转发 `render`/`_window_on_key` 等；
trait impl 可以与被实现类型不同模块），或让组件文件无条件生成 `Component` impl。

**2. 资源常量与生成代码不满足 `clippy -D warnings`（`codegen/assets` 等）**
`import icon from "…png"` 生成 `const statusbar: &'static [u8] = include_bytes!(…);`：

```
error: constants have by default a `'static` lifetime   (clippy::redundant_static_lifetimes)
error: constant `statusbar` should have an upper case name   (non_upper_case_globals)
```

生成的组件树还会触发 `clippy::needless_return`、`clippy::redundant_field_names`、`clippy::derivable_impls`
等（模板项目不在主 workspace 里，所以之前没有暴露）。建议生成文件头的 `#![allow(…)]` 补齐这些
（资源常量上加 `#[allow(non_upper_case_globals)]`）；在那之前示例 crate 用 `#![allow(clippy::all)]` 兜底
（`crates/zant-examples/tsx/*/src/main.rs`，注释写明原因）。

**3. 生成的 `native/spec.rs` 不是 rustfmt-clean**
`zant_build::tsx_app()` 写出的 `native/spec.rs` 末尾多一个空行，workspace 成员的 `pub mod spec;` 会被
`cargo fmt --all -- --check` 检查：

```
Diff in …/native/spec.rs:42:
     /// The registry key of this spec …
     pub const NAME: &str = "system";
 }
-
```

示例用 `#[rustfmt::skip] pub mod spec;` 规避（`native/lib.rs`）。建议生成物末尾只留一个换行（
`native_specs::generate` / `write_spec_module`），或由 `zant-build` 在文件头写 `#[rustfmt::skip]`。

**4. `desktop` / `entities` 仍不能编译（承接 WP-5.9 待决 2/4）**
**`desktop` 已解决（WP-COVERAGE）**：`Uint8Array` → `&'static [u8]` 后 `desktop.rs` 黄金生成且编译，
32 个语料里 31 个有黄金、全部 `cargo check` 通过；`entities` 仍按 WP-5.9 待决 4 排除（UTF-8 lone surrogate）。
以下为当时的记录：

```
tools/tsx/testdata/desktop.tsx:13:11: error: cannot convert tsx::List<u8> to &'static [u8]; use an explicit matching type
tools/tsx/testdata/entities.tsx:1:1: error: string literal is not valid UTF-8; Rust cannot represent an unpaired surrogate
```

基准输出 `failed: 2 (desktop, entities)`，比值取两侧共同编译的 30 个；`Uint8Array` 映射裁定后 `desktop`
应进入共同集。

**5. `zant-build` 的 `src/app_config.rs` 警告时序（无害）**
workspace 成员没有 `zant sync` 时，`zant_build::try_tsx_app()` 在打印
`cargo:warning=zant-build: src/app_config.rs is missing … run zant sync` 之后应用 `build.rs` 才写出该文件，
`cargo clean` 后第一次构建会看到一次警告，之后正常。要彻底消掉可以让 `try_tsx_app` 的指令打印延后到
调用方写文件之后（接口变化），或示例 build.rs 先写文件再调用 `zant_build::tsx_app()`。

### WP-5.10b 提出（2026-11-14）

**1. 子组件的回调 prop 在事件里被 `ui::effect` 吞掉（`crates/zant-tsx/src/codegen/components.rs` + `crates/zant/src/ui/context.rs`）**——**已修**：WP-TSX-BUGS 按建议的直接调用修法实现，见下方「WP-TSX-BUGS 完成情况」1。
codegen 把子组件里的 `onX(arg)` 生成成 `ui::effect(ui::Action::new(move |cx| (callback)(cx, arg)))`
（`components.rs:426`、`:440`，注释假设“>= 1 frame later”运行）。但 `ui::effect` 只是把 action
push 进当前 `BuildCx.effects`，而下一帧 `Runtime::rebuild()` 先 `build_tree()` → `ui::begin_frame()`
（`crates/zant/src/ui/context.rs:198-205`，**`build.effects.clear()`**）再 `run_effects()`：
事件处理里排队的 effect 在运行前就被清掉，回调永远不执行。Zig 版 codegen 是直接调用
（`tools/tsx/codegen/components.zig:tryCall`：`{callee}.func({callee}.ctx, …)` / `.call(.{ … })`）。

Repro（Rust vs Zig 基线同一步骤）：

```
# 终端里先起应用：ZANT_LAYOUT_FILE=/tmp/layout.txt target/debug/zant-tsx-notes
# 步骤文件：click new → type Milk → click add → click delete-1 → (等 0.5s)
# Zig 基线：zig-out-baseline/bin/zant-tsx-notes 同样步骤
Rust： 01-after-delete.layout 仍有 summary + open-1/title-1/pin-1/delete-1（笔记没删）
Zig ： 01-after-delete.layout 回到 empty 状态（笔记删除成功）
```
`examples/tsx/notes` 的 `NoteCard` 的 `onPin`/`onDelete` 都中招；`open`（本地状态）正常。
建议修法：子组件的事件方法生成成 `fn _on_click3(&mut self, cx: &mut ui::ActionCx<'_>)`，直接
`(self.props.onDelete)(cx, arg)`（事件 action 本来就有 cx）；或 `begin_frame` 不丢事件期入队的
effect（先 drain 再清）；直接调用更贴近 Zig。

**2. `Option<Node>` 属性收到裸元素时生成 `Node` 而不是 `Some(..)`（`crates/zant-tsx/src/codegen/jsx_values.rs:322-324`）**——**已修**：分支 base 的 M7.1 staged fix 已按此修法修改，WP-TSX-BUGS 补回归测试并复验（见下方「WP-TSX-BUGS 完成情况」2）。
`node()` 先 `if value.ty == Type::Node { return Ok(value.code); }`，然后才走 `optional` 的
`types::coerce(.., OptionalNode)`（会生成 `Some(..)`），所以 `<Modal footer={<Row>…</Row>}>` 报：

```
error[E0308]: mismatched types
  --> $OUT_DIR/App.rs:257: expected `Option<Node>`, found `Node`
```

Repro：`examples/tsx/gallery.tsx` 的 Modal `footer={<Row …>…</Row>}`（`tools/tsx/testdata/components.tsx`
只有 `footer={enabled ? … : null}` / `footer={null}`，语料没覆盖裸元素）。Workaround：示例改成
`footer={open ? (<Row>…) : null}`。修法：`optional && value.ty == Type::Node` 时也走
`coerce(.., OptionalNode)`。

**3. 富文本里紧跟表达式子节点的字面量文本被丢掉**——**已修**：分支 base 的 M7.1 `Format::text` fix 已修，WP-TSX-BUGS 补回归测试并复验 gallery **0 px**（见下方 3）。
`examples/tsx/gallery.tsx` 的 `<Text key="rich">…<Link>link</Link>{docs > 0 && ` (opened …)`}.</Text>`
生成的 `spans` 里最后一个 span 是条件表达式，**末尾的 `.` 没有生成**（`crates/zant-tsx/src/codegen/rich_text.rs`
的 JSX 文本子节点处理）。Rust/Zig gallery 快照恰好差 21 个像素（逻辑坐标约 (448–450, 532–534)，
0.0007%，仍在 0.1% 闸门内）：Zig 显示 `inline link.`，Rust 显示 `inline link`。修好后 gallery 与 Zig
应为 0 px。

**4. `screencapture -l` 本机不可用（非仓库 bug，记录环境）**
`ui-scenario` 的 `shot` 全部输出 `could not create image from window`（终端无 Screen Recording
权限），所以本 WP 用 `ZANT_LAYOUT_FILE` + `CGWindowList` 窗口框做交互断言；`scenario` 的 click/hover/
type/key/resize 步骤本身全部成功（`scenario ok`）。

### WP-5.10c 提出（2026-11-14）

**1. 非根组件渲染 `<List>` 生成代码不编译（`crates/zant-tsx/src/codegen/list.rs`）**——**已修**：WP-TSX-BUGS 按建议 hoist `_list`/`_state`，并顺带修掉 base workaround 的**运行期 panic**（根 action 形态），见下方「WP-TSX-BUGS 完成情况」4。

带 props 的组件（非根）里写 `<List renderItem={…}>`，只要 item 闭包引用组件自身的状态/回调
（例如 `const [picked, setPicked] = useState(0)` 后 `onClick={() => setPicked(row.id)}`），生成的
Rust 不编译：

```
error[E0505]: cannot move out of `self` because it is borrowed
  --> …/ListView.rs:71
   |                      let _state = self._state.clone();
   |                                   ------ move occurs due to use in closure
error: lifetime may not live long enough (coercion requires that `'1` must outlive `'static`)
```

原因：`list.rs::emit` 生成 `ui::list(&self._listN, ListOptions { item: Rc::new(move |_index| { …
let _state = self._state.clone(); … }) })`——item 闭包为了拿到组件实例捕获了 `self`（`'static`
要求），同时 `&self._listN` 还在借 `self`。根组件没有 `_state` 字段，所以 `list_bench.tsx` 原样作根
可以编译（本 WP 的 list 基准因此用根页面）。

Repro（TSX，`zant-tsx` 直接编译即可；转成 `src/ListView.tsx` 后 `zant_build::tsx_app_in` 报上面的错）：

```tsx
import { useState, List, View, Row, Text, Button } from "zant";
import type { u32 } from "zant";

interface RowData { id: u32; title: string; }

export function ListView({ rows }: { rows: RowData[] }) {
  const [picked, setPicked] = useState<u32>(0);
  return (
    <View>
      <Text>Picked {picked}</Text>
      <List data={rows} itemHeight={40} keyExtractor={(row) => row.id}
        renderItem={(row) => (
          <Row>
            <Text>{row.title}</Text>
            <Button onClick={() => setPicked(row.id)}>Pick</Button>
          </Row>
        )} />
    </View>
  );
}
```

建议修法：`list.rs::emit` 在构造 `ListOptions` 前把 `self._listN` / `self._state`（以及闭包需要的
`self.props` 字段）克隆进局部变量，闭包只捕获这些局部量，`ui::list` 改传 `&_list`：

```rust
let _list = self._listN.clone();
let _state = self._state.clone();
let _data = …;
ui::list(&_list, ui::ListOptions { item: Rc::new(move |_index| { … _state … }), .. })
```

**2. 生成代码的 `dead_code`（根组件保留但不驱动时）**

本 WP 的 markdown 页保留 `markdown_bench.tsx` 作根（证明输入可编译），实际驱动的是
`MarkdownView` 组件，于是 rustc 报 `MarkdownBench is never constructed` 等；bench 在生成模块上用
`#[allow(clippy::all, dead_code)]` 限定（与 WP-5.10 示例级 `#![allow(clippy::all)]` 同因，等
`zant-tsx` 生成代码 lint 兜底补齐后可去掉）。

**3. 记录（非 bug）：`useState` 初始化器只接受字面量**

Zig 的 `tsx.State` 可以写根状态，所以 `bench_list` 直接把 10 000 行 set 进去；Rust 侧
`useState<T[]>([...])` 只接受字面量、`useMemo` 的依赖数组才允许函数调用。list 基准因此用
`useMemo(() => makeRows(), [])` 在 warm-up 帧建一次行数据（见完成情况「偏差」1）。M8 的 TSX 指南
可以记一句。

### WP-5.6 提出（2026-11-14）

1. **跨文件 `use` 路径与模块树（给 WP-6.2）**：Rust 没有 Zig 的 `@import("X.zig")`，相对导入
   生成 `crate::<模块名>::…` 路径（模块名 = 文件 stem 的 snake_case，`TodoRow` → `todo_row`），
   生成的 `main.rs` 用 `#[path = "TodoRow.rs"] pub mod todo_row;` 声明它们；`#[path]` 相对
   `main.rs` 自己所在目录解析（实测 `include!` 后仍如此），所以 `zant_build::tsx_app` 把模块写进
   OUT_DIR、应用 `src/main.rs` `include!(concat!(env!("OUT_DIR"), "/main.rs"))` 即可。**手写 `.rs`
   模块（`./format.rs`）不在生成的 `main.rs` 里声明**：应用 crate 根自己 `mod format;`，再声明一次
   会重复定义。6.2 需要按这个约定接线。
2. **`Uint8Array` 的类型映射冲突（需 5.3/5.7 负责人裁定）**：`types.rs` 把 `Uint8Array` 映射成
   `tsx::List<u8>`（WP-5.3），而 WP-5.7 的运行时把字节缓冲叫 `tsx::Bytes`（`Rc<[u8]>`）、
   静态资源/托盘图标是 `&'static [u8]`。本 WP 给**资源导入**与 tray icon 定了 `&'static [u8]`
   类型（`include_bytes!` 需要），于是 `const icon: Uint8Array = [...]`（`testdata/desktop.tsx`）
   传给 `useTray({ icon })` 会报 `cannot convert tsx::List<u8> to &'static [u8]`。建议：5.3 把
   `Uint8Array` 改成字节类型（`tsx::Bytes` 或静态切片），或给 `TrayOptions.icon` 加接受 `Bytes`
   的入口。在裁定前 `testdata/desktop.tsx` / `platform.tsx` 不进入语料通过集（见报告）。
3. **必需的跨文件小改（本 WP 已做，均为 1–3 行、不影响既有行为）**：
   (a) `import_parser.rs` 的 `is_asset` 排除 `.rs`（否则 `./format.rs` 被当成资源文件，rust 导入无法解析）；
   (b) `stores.rs` 的 `Store` 加 `declared` 字段（`import { prefs as shared }` 时访问器名必须用
   声明文件的名字，否则生成 `crate::store::shared_read()` 找不到函数）；
   (c) `stores::imported` 不再转义 alias（alias 已是 `crate::store` 这样的 Rust 路径，转义会把 `::`
   变成 `__`）。
4. **`generate.rs` 的接线超出 `install()`**：除 `install()` 的单行赋值外，还需要
   (a) `generate_inner` 把 `globals::assets` 换成 `assets::process`（assets 现在是真实现，
   `globals.rs` 的占位会报错）；(b) `collect()` 里两处 WP-5.6 占位（app hooks collect、desktop
   hooks collect）换成真调用，并删掉已被 `app_hooks::{statement,declaration}` 取代的
   `statement_hook`/`declaration_hook`。均已在 `generate.rs` 内，diff 约 40 行。
5. **`--dump-ast` 没有做**：计划 M4 验收里提到「WP-4.1 给 Zig 版加只读 `--dump-ast`」，但
   `tools/tsx/main.zig` 并没有这个开关（`dump.rs` 注释也写明了），按任务要求不自行发明；
   Rust 侧只保留 `dump::dump_program` 库函数。`--os` 标志也没移植：生成的 entry 直接内联
   `zant::tsx::app::Config`，不再生成 `app_config.zig`（平台相关字段由 6.2 决定）。
6. **窗口组件的 `tsx::window::Component` 形态**：设计文档 §6.2 的 `Local` 类型（`PanelLocal`）与
   WP-5.5 生成的子组件形态（状态是 `Panel` 的扁平字段 + `Weak<RefCell<Panel>>`）不一致。本 WP
   为窗口组件生成了一个持有 `Rc<RefCell<Panel>>` 的包装 `PanelWindow`（`type Local = Panel`），
   render / hooks 都转发给内部实例；行为一致，但 5.9 审阅生成代码时要知道这个包装。
7. **语料通过集**：`tools/tsx/testdata` 32 个文件里，本 worktree 通过 7 个
   （`app_hooks`、`builtins`、`entities`、`functions`、`loops`、`text`、`windows`），
   已知缺失 25 个：21 个要 WP-5.4 的 JSX prop/事件/children lowering（`object literal needs a
   type annotation`、`conditions must be boolean`、`event parameter inference is WP-5.4` 等），
   `timers` 要无归属的 `codegen/timers.zig`，`desktop`/`platform` 是 `Uint8Array`↔bytes 类型冲突
   （见上一条），`strings` 是 **WP-5.3 的 bug**：`string_methods`/`aggregate` 生成
   `label: { … }.clone()`（带标签块当方法接收者，不是合法 Rust，生成器 panic）。
   清单在 `tests/codegen_app.rs` 的 `KNOWN_MISSING`，测试断言「已知缺失集合 == 语料里的失败集合」；
   `tests/tsx/**` 的 `.zig` 导入项目（`zig_types`/`crossfile`）要等 `.rs` 迁移后再测。
8. **`zant-tsx --help`**：Zig CLI 没有 `--help`（未知参数按用法退出 2）；Rust 侧加了
   `--help`/`-h`（打印用法、退出 0），其余参数与退出码照搬。

### WP-5.5 提出（2026-11-14）

1. **WP-5.4/5.6/5.8 钩子待接线**：`codegen/generate.rs` 的 `install()` 里已按行列出
   `g.extensions.hooks.emit_jsx/scan_body/scan_expr/emit_setter/method_param/forward_arrow/parameter_type`、
   `extensions.process_imports/collect_windows/app_lifecycle/is_window_const`、`native` 的装配位置；
   `mod.rs` 已加对应扩展字段。合并各 WP 时把这些注释行改成赋值即可（都是单行）。
   在 5.4 落地前，任何渲染 JSX 的模块都会报 `JSX code generation is implemented by WP-5.4`。
2. **子组件回调 prop 的调用需要运行时 `ActionCx`**（设计缺口，需负责人裁定）：Rust 的 `ui::Action`
   只能由运行时用 `ActionCx` 调用；Zig 的 `tsx.Callback` 带根状态指针，允许在 handler 里直接
   `callback.call(...)`。当前实现：签名为 ui 事件（`()/bool/string/usize`）的回调 prop 用
   `ui::Action` 族，子 handler 调用时经 `ui::effect(...)` 延迟一帧执行（能编译、语义等价但晚一帧）；
   非 ui 事件签名（如 `(id: number) => void`）目前报诊断。要彻底解决需要在 `ui` 里增加
   “在 handler 中调用 Action”的能力（或给 handler 传 `ActionCx`）。
   **已解决**（WP-TSX-BUGS）：生成的 handler 需要时带 `_cx: &mut ui::ActionCx<'_>` 参数，回调 prop
   直接同步调用（`Action` 族 `.call(_cx, …)`、其余 `Rc<dyn Fn(&mut ActionCx, …)>` 直接调用）；
   根组件 handler 不受影响，也不需要改 `ui` 的 Action API。
3. **`tsx::Store<T>` 的契约**（给 WP-5.7）：生成代码假设 `tsx::Store::new(initial)` 与 `&self` 方法
   `get/read/set/update(impl FnOnce(&mut T))`；每个 `createStore` 生成
   `thread_local!` 静态 + `settings_{get,read,set,update}` 四个 `pub fn`（Rust 无可变 `static`）。
4. **`tsx::strings` 的方法名**（给 WP-5.7）：`codegen/string_methods.rs::rust_name` 固定为
   `starts_with/ends_with/index_of/last_index_of/to_lowercase/to_uppercase/pad_start/pad_end/char_at/char_code_at/code_point_at/replace_all` 等，
   接收 `&str`/`f64`，字符串结果返回 `zant::Str`，`split` 返回 `tsx::List<zant::Str>`。
5. **`scope.CaptureSource` 缺类型与 narrowing**：5.3 的实现只存可见名字，`captures.zig` 需要的
   “外层 context 的类型 + narrowing 事实” 无法取到。WP-5.5 用 `Generator::capture_scope`
   快照（`captures::prepare` 时记录）绕过；代价：narrowing 后的 optional 以声明类型捕获、
   箭头套箭头的捕获不再向上递归。要补需改 `codegen/scope.rs`（5.3 文件）。
6. **`<List>` 的 `ui::ListState` 字段**：`Generator::lists` 目前是 `Vec<String>`（字段名），
   WP-5.4 的 `list.rs` 需要 `jsx`/`item_method`/`id_method`，合并时把该字段扩展成真正的 `ListInfo`。
7. **子组件本地状态的实现形态**：设计文档 §6.2 写 `ChildState` + 回调捕获 `Rc<RefCell<_>>`；Rust 里
   handler 要保持 `&mut self` 形态，所以子组件本地状态槽里放的是**整个子组件实例**（props+state），
   并通过 `_state: Weak<RefCell<Child>>` 让 action 在回调时 upgrade（方法签名不变）。属实现偏差，
   行为一致；如坚持 §6.2 字面形态需改 `scope::state_field` 的前缀规则（5.3 文件）。

> 执行者遇到计划没写清的设计问题，登记在这里并停下来等负责人裁定，不要自行发明方案。

1. **`zant::Error` 尚未定义**（WP-3.5 提出）。裁定：错误类型在 WP-1.2 随 §4 契约落地；
   在那之前各模块可用自己的 `thiserror` 错误类型（如 `math::parser::ParseError`、`db::DbError`），
   合入 `feat/rust-port` 后由 lead 统一收编到 `zant::Error`，不阻塞各 WP。
2. **`tests/flex_test.zig` 的归属**（WP-1.1a 提出）。裁定：归 WP-2.10（见上），1.1a 不实现 Tree。
3. **clippy `module_inception`**：`platform/mod.rs`、`renderer/mod.rs`、`motion/mod.rs` 的模块名与父模块同名
   （对应 Zig 的 `platform/platform.zig` 等）。裁定：保留同名，加 `#[allow(clippy::module_inception)]`，
   让 `-D warnings` 通过。
4. **WP-2.8 的 3 处跨文件小改**（计划里没有预置，缺了桌面集成无法实现；均不改变既有行为）：
   (a) `platform/platform.rs` 新增安全 `AppEventHandler` 蹦床（`install_app_event_handler` /
   `clear_app_event_handler`），因为 `zant_app_set_handler` 是 `unsafe`，而 `desktop` 不允许 `unsafe`
   （plan §3 rule 5）；(b) `image.rs` 新增 `Image::read_rgba`（托盘图标要把解码后的像素交给
   `platform::set_tray`，该 C 函数在 `zant-sys` 里只有 `unsafe` 声明，`desktop` 不能直接调）；
   (c) `runtime/debug.rs` 新增 `FrameStats::for_window`（第二个窗口的 `ZANT_FRAME_STATS` 标签；
   `FrameStats` 字段私有）。三处都加了对应测试。
5. **`Shortcut` 有两份**（WP-2.5 提出，WP-2.8 已实现完整版）：WP-2.8 把 `Shortcut` 放在
   `crate::desktop`（`label` / `format` / `parse` / `from_key_event` / `eql` / `register_hotkey`，
   Zig 同名方法齐备）。WP-2.5 在 `components/hotkey_input.rs` 里先移植了 recorder 需要的子集；
   两者合并后让 `hotkey_input` 改为 `use crate::desktop::Shortcut`（字段与语义一致，desktop 版多
   `format` / `parse` 与注册函数）。
6. **多窗口的关闭是延迟的**（WP-2.8 deviation）：Zig `Window.close` 在回调里直接销毁运行时；
   Rust 版先向原生窗口发关闭请求，`WindowClose` 事件入队，主窗口的 wake 回调里再
   `process_pending_closes`（避免在窗口自己的事件回调里销毁原生窗口）。`close()` 立即生效的时机
   是"当前事件回调返回后的下一个事件循环轮次"；`open_window` 在没有 `ui::app::run` 主窗口时返回
   `PlatformError::UnsupportedPlatform`（Zig 是编译期就必须在 run 之后调用）。
7. ~~**`ui::key_bounds` 在 `on_resize` 里返回 `None`**~~（WP-2.10 提出，已修复 `cbab28c`）：Zig 的
   `ui.keyBounds` 读活树，所以 `BoxOptions::on_resize` 回调里能拿到控件的新 bounds；Rust
   `runtime::build_tree` 原本在 build 末尾 `set_key_bounds_hook(None)`，而 `notify_resizes`（触发
   `on_resize`）在随后的 `render_frame` 里才跑，hook 已清空 → `None`。lead 采用方案 (a)：build
   结束不再清 hook，快照在每次 layout 后由 `refresh_key_bounds` 刷新、runtime drop 时才清，整帧内
   `ui::key_bounds` 都可用。WP-2.10 的 `pointer_test.rs` 已补上原先被阻塞的 pointer_area 用例并通过。

4. **组合组件未从 `ui::*` 导出**（WP-2.5 提出）：8 个组件按计划落在 `crates/zant/src/components/*.rs`，
   `components/mod.rs` 已预先声明模块；但 `crates/zant/src/ui/mod.rs` 属于 WP-1.x 的文件，WP-2.5 不改。
   裁定建议：lead 在 `ui/mod.rs` 的 `pub use` 区块追加
   `pub use crate::components::menu::{dropdown_menu, MenuItem, MenuItemKind, MenuOptions};`、
   `modal::{modal, ModalOptions}`、`segmented::{segmented, SegmentedOption, SegmentedOptions}`、
   `message::{message, MessageKind, MessageOptions}`、
   `shortcut_list::{shortcut_list, Shortcut, ShortcutGroup, ShortcutListOptions}`、
   `hotkey_input::{hotkey_action, hotkey_input, HotkeyAction, HotkeyInputOptions, Shortcut}`、
   `view::{view, ViewOptions}`、`window_controls::{window_controls, WindowControlsOptions, CAPTION_MAXIMIZE_KEY}`，
   使 Rust 侧与 Zig 的 `ui.modal` / `ui.dropdownMenu` / … 同名可用（当前可经 `components::…` 使用）。
5. **`desktop.Shortcut` 的临时位置**（WP-2.5 提出）：`hotkey_input.zig` 依赖 `desktop.zig` 的 `Shortcut`
   （WP-2.8 才移植）。WP-2.5 在 `components/hotkey_input.rs` 内先移植了 recorder 需要的子集
   （`fromKeyEvent` / `isValid` / `eql` / `label`，含平台 key 白名单与 macOS / Windows 标签）；
   `format` / `parse` 留给 WP-2.8。裁定建议：WP-2.8 把 `Shortcut` 放到 `crate::desktop`，并让
   `hotkey_input` 改为 `use crate::desktop::Shortcut;`（或在 desktop 里 `pub use` 本模块的类型），
   字段名与语义不变。

7. **`math/mod.zig` 的 UI 层未移植**（WP-3.11 提出）：Zig `math/mod.zig` 的 `block` / `render` /
   `inlineText` / `inlineDrawing` 属 UI 层，WP-3.6 只移植了 `layout` 引擎（`math::layout`）。
   WP-3.11 在 `crates/zant-examples/src/math_app.rs` 里按 Zig 逐行实现 `math_block`（display、
   上下各半 em 边距、100% 宽居中 row，见 `zant-math` 测试），`inlineDrawing` 已在 WP-3.3 的
   `markdown::render::spans::inline_drawing`。裁定建议：把 `block` / `render` 移进 `crate::math`
   （约 40 行，example 改调用 `zant::math::block`）；`zant-math` parity 已证明绘制结果与 Zig 逐像素一致。
8. **`ui.pdfViewer` 组件与 PDF 工具条 segmented**（WP-3.11 提出）：`src/components/widgets.zig` 的
   PDF 半（`PdfViewerOptions` / `PdfViewerState` / `pdfViewer`，含 `password identity` 测试）没有任何
   WP 认领（WP-3.3 声明留给 3.9，3.9 只做了 viewer）。WP-3.11 在
   `crates/zant-examples/src/components_app/pdf_viewer.rs` 里逐行实现同名组件（local slot + 惰性 open
   effect + empty/loading/failed 表面 + `pdf::view` 透传），`zant-components` parity 6/6，并且
   `pdf-2002-zin` 的 on_change 事件恢复（`components-check.ps1` 的第 162–163 行可用）。
   同时 3.9 的 `pdf/viewer/annotate.rs::tool_switch` 用图标按钮替代了 Zig 的 `ui.segmented`
   （当时的 base 没有 2.5 的 `ui::segmented`；本 base 已有）。后果：`zant-pdf` layout 差 25 项
   （Zig `pdf-tools*` 11 个 key vs Rust `pdf-tool0..4`，另 8 个 x 偏移）、像素差 0.7–2.0%，差异全部
   在工具条（y ≤ 70）；`pdf-p0*` 页面瓦片坐标与像素完全一致。裁定建议：(a) 把 `pdf_viewer` 移入
   `crate::components::widgets`（+ 其 1 个测试）；(b) `tool_switch` 改成 `ui::segmented`
   （Zig 原样：key `{key}-tools`、`height: 24`、`font_size: 12`，5 个 label/icon，`on_change` → `tb_tool`），
   之后 `zant-pdf` 应能过 parity。
9. **`zant-math` / `zant-pdf` 的 parity 调用方式**（WP-3.11 记录，非待决）：`zant-math` 的 Zig 示例
   没有任何 key，`ZANT_LAYOUT_FILE` 是空文件，`capture.sh` 的 `-s layout.txt` 检查会失败 → 用同一套
   `ZANT_SNAPSHOT*` / `ZANT_WINDOW_SIZE` 环境手动截图并只跑 `imgdiff.py`（本次 6/6、0 像素）；
   `zant-pdf` 的 Zig 示例必须带 argv（`zig-out-baseline/bin/zant-pdf tests/fixtures/widgets.pdf`），
   `compare.sh` 不支持传参 → 手动跑 `capture.sh` 的同一套环境变量并分别 `layoutdiff.py` / `imgdiff.py`。
   `zant-gallery` 的 Zig 基线不写 `layout.txt`（imperative 树没有 key），`compare.sh zant-gallery`
   在 capture 阶段就失败；与 3.11 无关，留给 lead 决定是否给 gallery 加 key 或换对照方式。

## WP-E2E 记录（2026-11-14，`rust/wp-e2e-tsx`）

模板应用（`scaffold/template-ts`）现在端到端可编译并运行：
`./scripts/tsx/build-template-check.sh` 通过（`cargo build` + `ZANT_SNAPSHOT` 快照），
`./scripts/tsx/build-check.sh`（WP-6.2 fixture）保持通过。本 WP 的设计决定（按 Zig 行为取最接近者）：

1. **`native/spec.rs` 的落点**：`zant-tsx` 的 `GeneratedProject::spec_root` 产出合并后的 spec 根
   （`native_specs::spec_root`；`compile_project` 在解析完 spec 后生成）。`zant-build` 把它写进
   **`<app>/native/spec.rs`**，模板的 `native/lib.rs` 声明 `pub mod spec;`，模板 `.gitignore` 忽略它。
   写文件是幂等的（内容相同不重写），并加进 `rerun-if-changed`，这样删掉生成文件也会重建而不是
   `pub mod spec;` 报错；`$OUT_DIR` 里不再写每 spec 一个 `_spec_<名字>.rs`。`zant-tsx --out` 把同一份
   内容写成 `<out>/spec.rs`（Zig 的 `spec.zig` 行为）。选择写进应用而不是 `OUT_DIR` + `include!`：
   与模板/文档里的 `native/spec.rs` 名称、以及 IDE 里的模块跳转一致；代价是 build.rs 会写源码树
   （与 `zant sync` 生成 `src/app_config.rs` 同类）。
2. **跨文件组件调用**：调用点是 `crate::<模块>::<组件>::build(<props 结构体>)`，props 结构体用解析后的
   类型路径（`crate::<模块>::<Name>Props`；无 props 时传 `Default::default()`），不再用组件结构体
   `Component { … }` 当字面量。每模块仍生成 `pub type Props = <Name>Props;`（`build` 的签名用它），
   但跨文件调用不依赖这个别名，同一文件多个组件时不会撞名。
3. **子组件回调 prop**：props 字段类型是 `ui::*Action` 族；默认值从 `std::rc::Rc::new(|| ())` 改成匹配的
   构造器（`ui::Action::new(|_| ())` / `ui::TextAction::new(|_, _| ())` …，见 `model::callback_default`）。
   根组件给子组件回调 prop 的内联箭头仍用 `ui::action(|s: &mut App| …)`（`generate::action_value` 的
   根状态形态）；转发自己的回调 prop 时直接 clone（`self.props.x.clone()`）。子组件为自己的局部状态
   生成的内联箭头仍走 `ActionCx` + `Weak<RefCell<Child>>` 形态（`ActionCx` 不暴露局部状态，
   WP-5.5 待决问题 2 的语义缺口仍在，模板未用到）。
4. **`zant::theme::Theme`**：`crates/zant/src/theme/mod.rs` 增加 `pub use tokens::Theme;`
   （`useTheme()` 生成的 `let theme: zant::theme::Theme` 需要它）。
5. **生成代码的 lint**：`HEADER_ALLOWS` 增加 `#![allow(non_snake_case)]`（TSX 的 camelCase props /
   handler 名本来就不合法）；`native_specs` 只在确实生成 `tsx::List` 等类型时才写 `use zant::tsx;`；
   根状态回调的 `ui::action` 不再套多余的 `{ … }`（去掉 `unused_braces`）。
6. **剩余告警**：模板构建只剩 `src/app_config.rs` 里 `version` / `sandbox` / `signing_identity`
   三个 `dead_code` 告警。该文件由 `zant-cli` 的 `app_config::source` 生成，正确修法是给这几个常量加
   `#[allow(dead_code)]`（属 `crates/zant-cli`，不在本 WP 范围）；模板 `src/main.rs` 暂以
   `let _ = (…)` 引用它们保持新项目零告警。
7. **Windows**：本机不可达，未验证；新增代码只有平台无关的路径/文件写入，
   `#[cfg(windows)]` 分支未改动。

## WP-TSX-BUGS：`zant-tsx` 已验证编译器 bug 修复（2026-11-14）

分支 `rust/wp-tsx-bugs`。修「WP-5.10b 提出」1–3 与「WP-5.10c 提出」1 四个**已验证**的代码生成
bug；提交信息 `WP-TSX-BUGS: synchronous child callbacks, optional node props, rich-text tails, List in children`。

### 1. 子组件回调 prop 同步执行（最高优先级）

**根因**：子组件 handler 里的 `onX(arg)` 生成 `ui::effect(ui::Action::new(move |cx| (callback)(cx, arg)))`。
`ui::effect` 只把 action push 进 `BuildCx.effects`，而 `Runtime::rebuild()` 先 `build_tree()` →
`ui::begin_frame()`（`crates/zant/src/ui/context.rs`，`build.effects.clear()`）再 `run_effects()`：
事件期排队的回调在运行前被清掉，永远不执行（Zig 是同步 `{callee}.func({callee}.ctx, …)`）。

**修法**（不改运行时 API，只改生成代码）：
- 新增 `crates/zant-tsx/src/codegen/action_cx.rs`：生成 handler 前扫描它的 body，决定它是否需要运行时
  上下文——直接调用本组件回调 prop、经 `props.onX`，或调用另一个（传递地）需要它的生成函数
  （`function add(delta) { …; onChange(delta); }` 这种真实写法）；事件/回调箭头是独立方法（跳过），
  数组回调闭包按内联代码继续走。带调用图记忆、递归防护与局部变量遮蔽分析。
- 需要时 handler 签名变成 `fn _on_click3(&mut self, _cx: &mut ui::ActionCx<'_>)`；
  `components::try_call` 生成 `(self.props.onDelete.clone())(_cx, arg)`（`ui::*Action` 族用
  `.call(_cx, arg)`，`string` 参数用 `.as_str()`）。`generate::action_value` / `callback_value`
  把 action 闭包里的 `_cx` 传进去；直接调用（`self.add(_cx, 1)`）由 `expr::call_value` 注入；
  数组回调闭包继承 `cx_var`。`scope::Context.cx_var` 同时保证 TSX 局部变量不能遮蔽 `_cx`。
- 根组件 handler 保持原样（根组件没有 callback props → 不需要 cx；输出与之前逐字节一致）。

**测试**：
- `crates/zant-tsx/tests/compile_regressions.rs`：`child_callback_props_run_synchronously`（Action /
  `Rc<dyn Fn>` / TextAction / BoolAction 四种签名 + 经 helper 的传递调用）、
  `a_props_object_callback_prop_also_runs_synchronously`，断言不再出现 `ui::effect`、签名带 `_cx`，
  并把模块写进 workspace 外临时 crate `cargo check` 通过。
- `crates/zant/tests/tsx_callbacks_test.rs`：手写生成代码的形态（`local_state` + `Weak` 升级 +
  `_on_click(&mut self, cx)` → 父回调）并真正 `Runtime::dispatch` 一次点击，断言父状态在**同一次
  dispatch** 内改变、下一帧文本更新（旧的 `ui::effect` 形态在这里必然失败）。

### 2. `Option<Node>` prop 收到裸元素

**状态**：分支 base（提交 `060601e` 的 M7.1 staged fix）已按报告修法在 `jsx_values::node` 里
`value.ty == Type::Node && optional` 时包 `Some(..)`。本 WP 复验并补回归测试：
`<Modal footer={<Row>…</Row>}>` 生成 `footer: Some(ui::row(…))`、能编译。
`crates/zant-examples/tsx/tsx-gallery/src/App.tsx` 里的三元 workaround 保留（不在本 WP 范围；
渲染树与 Zig 相同）。

### 3. 富文本末尾字面量

**状态**：base 的 M7.1 staged fix（`rust_text::Format::text` 不再保留 `only_string`）已修：
`<Link>link</Link>{cond}.` 生成 `format!("{}.", if …)`。本 WP 补回归测试
（`rich_text_keeps_a_literal_after_an_expression`）；`examples-check.sh` 的 Zig 基线对照
gallery **0/2918400 像素**（WP-5.10b 的 21 px 差异消失），todos/reader/notes 同为 0 px。

### 4. 非根组件里的 `<List>`

**根因（比报告更深一层）**：base 的 workaround 在 `item_closure`/`id_closure` 里把
`g.current_component` 置空，行闭包不再捕获 `self`，于是**能编译**；但闭包里生成的 action 走的是
**根组件**形态 `ui::action(|s: &mut Child| …)`，运行期 `ActionCx::state::<Child>()` 会在根状态是
`App` 时 panic——报告里的 E0505 只是表象。

**修法**：按报告建议 hoist——

```rust
let _list = self._list1.clone();
let _state = self._state.clone();   // 仅非根组件
ui::list(&_list, ui::ListOptions { item: std::rc::Rc::new(move |_index| { … }), .. })
```

- `list.rs::emit` 把 `_list`（以及子组件的 `_state`）提到 options 前，`ui::list(&_list, …)`；
- `Generator::handler_state` 让行闭包里生成的 action 用 hoisted `_state.clone()` 升级**本组件**的
  `local_state`（仍是子组件形态，不再借 `self`），语义正确；
- `item_closure` 保留 `current_component`（不再置空），`collect_captures` 用 `components::read_prop`
  捕获解构的 props（`onDelete`）与非解构 props 对象，并把 JSX 收集递归到嵌套元素。

**测试**：`list_in_a_child_component_clones_its_state_slot`（断言 `let _list` / `let _state` /
`ui::list(&_list,` / 闭包内 `_state.clone()` / `(cap.onPick.clone())(_cx,`，并编译）；
`codegen_jsx` 的 list 用例同步更新。`local_state`/`list`/`list_scroll` golden 更新。

### 重新生成的 golden

`crates/zant-tsx/testdata/{children,named_handlers,list,list_scroll,local_state}.rs` 5 个（其余 26 个
golden 逐字节不变；`tests/snapshots/` 不变）。逐行审阅：`children`/`named_handlers` 是 handler 签名 +
同步调用（`ui::effect` 消失）；`list`/`list_scroll`/`local_state` 是 `_list`/`_state` hoist。

### 验证（全部通过）

| 命令 | 结果 |
|---|---|
| `./scripts/cargo.sh test --workspace` | 81 个测试结果（含 doc-tests）/**1092 passed / 0 failed**；新增 7 条（compile_regressions 6 + tsx_callbacks_test 1） |
| `./scripts/cargo.sh clippy --workspace --all-targets -- -D warnings` | 0 告警 |
| `./scripts/cargo.sh fmt --all -- --check` | 干净 |
| `./scripts/tsx/build-template-check.sh` | OK（scaffold 模板 build + 快照） |
| `ZANT_ZIG_BIN=<baseline> ./scripts/tsx/examples-check.sh` | OK：7 个应用两次运行字节一致；4 个 Zig 对照 layout 全等且像素全 **0 px**（gallery 0/2918400、todos 0/1433600、reader 0/3404800、notes 0/1164800）；编译基准 **1.10×** Zig PASS |
| `./scripts/tsx/bench-tsx.sh` | **PASS 6/6 apps、11/11 scenarios ≤1.20×**（最高 `todos-add` 1.11×、`local-mount` 1.10×） |
| `ZANT_ZIG_BIN=<baseline> ./scripts/parity/compare.sh zant-demo` | **6/6 组合 layout 相同、0 px**（light/dark × 3 尺寸） |

### 剩余 / 注意

1. 本 WP 未改 `crates/zant/src/ui/**` 或 `runtime/**`：不需要运行时 API 变更，`ui::effect` 仍是
   `useEffect` 的排队机制。
2. base 的两个 staged fix（bug 2/3）原先没有回归测试；现由 `compile_regressions.rs` 覆盖。
3. 回调 prop 的调用只支持「本组件 props 里的回调」——`const f = onDelete; f(1)`（别名）仍是
   “local value, not a component function” 诊断（Zig 同样不支持）。
4. `<List>` 行闭包里回调 prop 的捕获依赖 `collect_captures` 的解构 props 捕获；嵌套 `<List>`
   仍不在支持范围。
5. Windows 测试机不可达，未验证；本 WP 只改平台无关的代码生成器与测试。

## Zig 变更日志

> Zig 版冻结功能开发，只修 bug；每一条都要同步到已移植的 Rust 模块。

1. `src/platform/platform.zig`：`createWindow` 新增 `ZANT_WINDOW_SIZE=WxH` 自动化钩子
   （只覆盖初始窗口尺寸），供 `scripts/parity/capture.sh` 在固定尺寸下截图对照。
   不影响正常使用；Rust 版 `platform/createWindow` 要照搬同一行为（WP-1.1d）。
2. `src/renderer/skia/skia_bridge_win.cpp` + `skia_bridge.h`：新增
   `zant_set_icu_data` / `zant_set_math_font`，用 `#ifdef ZANT_STATIC_DATA` 包住；
   Zig 的 DLL 构建不定义该宏，行为不变（WP-0.3）。
