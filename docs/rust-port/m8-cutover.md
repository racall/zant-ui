# M8 切换（Zig → Rust 收尾）

本文件是 M8 的执行手册：把仓库从「Zig 版与 Rust 版并存」切成**纯 Rust**。
执行者是 lead，**必须先得到用户明确同意**才能跑真正的 `--apply`；M8 的删除与发布
（plan.md §5 M8 第 5 条）都需要用户点头。

- 脚本：`scripts/migrate/cutover-m8.sh`
- 迁移记录（历史，保留不动）：`docs/rust-port/plan.md` §5 M8 / §7、`docs/rust-port/status.md`
- 演练：在 `/tmp` 的一次性副本里跑完整流程，见文末「演练记录」

## 为什么需要这个脚本

M8 不是 `rm -rf` 就能完成的事情：原生源码要移动、`build.rs` / bindgen / 预编译库 / 发布脚本
的路径要跟着改，`tools/` 里还有 Rust 侧运行时需要的资产（TSX 语料、`zant build` 的打包脚本、
DMG 背景图），删纯 Zig 模板还要同步删 CLI 的 `--lang zig`。手工做容易漏一处，而漏掉的引用
要等到构建或发布时才暴露。所以把全部改动写成脚本：**默认 dry-run、逐条断言目标文本仍存在、
先打备份 tag、最后跑完整验收**。

## 脚本做什么

`scripts/migrate/cutover-m8.sh` 分三个阶段。

### 1. 前置检查（`--apply` 时执行；`--skip-gates` 只给演练副本用）

- 当前分支是 `feat/rust-port`，工作区干净；
- 预 M8 结构还在（`build.zig`、`src/renderer/skia/`、`src/platform/`、`tools/tsx/testdata/`、
  `scaffold/template/`）；
- Zig 行为基线存在：`zig-out-baseline/bin/{zant-demo,zant-components,zant-gallery,zant-markdown,zant-math,zant-pdf,zant-bench}`
  （WP-0.1 的 ReleaseFast 基线；worktree 里会自动找到主 checkout 的那份，也可用 `ZANT_ZIG_BIN` 指定）；
- 质量门全绿：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo test --workspace`；
- 打备份 tag：`git tag -f rust-migration-pre-m8`（当前 HEAD）。回滚：
  `git reset --hard rust-migration-pre-m8 && git clean -fd`。

任何一条不满足就拒绝执行，不写任何文件。

### 2. 切换（dry-run 只打印；`--apply` 才执行）

移动（`git mv`，保留历史）：

| 从 | 到 |
|---|---|
| `src/renderer/skia/` | `crates/zant-sys/native/skia/` |
| `src/platform/{platform.h,pdf.h}` | `crates/zant-sys/native/platform/` |
| `src/platform/macos/`、`src/platform/windows/` | `crates/zant-sys/native/platform/{macos,windows}/` |
| `tools/tsx/testdata/*.tsx`（32 个） | `crates/zant-tsx/testdata/tsx/` |
| `examples/tsx/**/*.tsx` | `crates/zant-examples/tsx/corpus/`（parser 测试语料） |
| `examples/assets/zant.png` | `crates/zant-examples/assets/zant.png`（示例运行时图片） |
| `src/math/mhchem_cases.json` | `crates/zant/src/math/mhchem_cases.json`（mhchem 823 条夹具） |
| `scaffold/template/DESIGN.md` | `scaffold/template-ts/DESIGN.md`（TS 模板继续用它） |
| `tools/zant/package/`（`zant build` 运行时脚本、DMG 背景、Inno Setup 模板） | `scripts/package/` |

删除（`git rm`）：

- 全部 `*.zig`（348 个：`src/**`、`tools/**`、`tests/*.zig`、`examples/*.zig`、`build.zig`）、
  `build.zig.zon`；
- `tools/`（TSX 编译器 / CLI 的 Zig 实现；需要的东西已移出）、`src/`、`examples/`、
  `tests/tsx/`（Rust 侧镜像在 `crates/zant-tsx/tests/fixtures/`，`tests/fixtures/` 保留）；
- `scaffold/template/`（纯 Zig 模板）；
- `scripts/win/build_bridge.ps1`、`scripts/win/gen_def.sh`（`zant_skia.dll` 时代的构建脚本）；
- `scripts/tsx/create-smoke.sh`、`scripts/tsx/swc-corpus.sh`（驱动 Zig 构建 / Zig 编译器，已被
  `build-template-check.sh` / Rust 测试取代）；
- 随原生源码一起移走的 `zant_skia.def`、`zant_skia_resources.h`、`windows/winsqlite3.def`。

改写（每条替换都断言原文仍存在，漂移就报错）：

- 原生代码：`#include "src/platform/platform.h"` → `"platform/platform.h"`、
  `#include "src/renderer/skia/skia_bridge.h"` → `"skia/skia_bridge.h"`；`build.rs` 的 include
  路径改为 `crates/zant-sys/native/`；
- Windows 桥：删除 `zant_skia.dll` 的 RCDATA 资源读取分支和 `ZANT_STATIC_DATA` 宏守卫
  （静态链接后 ICU 数据 / STIX Two Math 一律由 `zant-sys` 用 `include_bytes!` 嵌入并经
  `zant_set_icu_data` / `zant_set_math_font` 传入）；`build.rs` 不再定义 `ZANT_STATIC_DATA`，
  `gen_bindings.sh` 同步改 header 路径与 `-I`；
- 预编译库：`crates/zant-sys/prebuilt/sources-{macos,windows}.txt` 换成新路径（source_hash 随
  之更新，旧发布包里的预编译库会因 hash 不符自动回退到源码编译），`prebuilt/README.md` 同步；
- Rust 引用：`tests/golden.rs` / `tests/codegen_app.rs` / `parser.rs` / `zant-tsx-bench.rs` 的语料
  路径，`zant-demo` / `components_app` 的 `zant.png` 路径，`mhchem` 的夹具路径，
  `zant-build` 的 Windows manifest 路径，bindgen 生成文件头部的 header 注释；
- CLI：删除纯 Zig 模板与 `--lang zig`（`project.rs` 只剩 `is_tsx_app`、`create.rs` / `scaffold.rs` /
  `main.rs` / `tests/cli.rs` 同步），`scaffold` 改从 `crates/zant-tsx/zant.d.ts` 复制生成的类型声明，
  `zant build` 的打包脚本路径改 `scripts/package/`，DMG 背景图同步改；
- CI：`.github/workflows/rust.yml` 去掉 Zig toolchain 与 `zig build test`；`release.yml` 去掉
  Zig 时代注释；
- 脚本：`scripts/gen_bindings.sh`、`scripts/release/*`、`scripts/parity/*`、`scripts/tsx/*`、
  `scripts/win/*`、`scripts/worktree.sh`、`scripts/mac/ui-scenario.swift` 里的 Zig / `zig-out` 引用
  改为 Cargo / `target/`；
- 文档：`AGENTS.md`、`CLAUDE.md`、`README.md`、`docs/{architecture,development,scaffold,tsx,tsx-native,
  markdown}.md`、`docs/components/{pdf,markdown,pointer-area}.md`、`tests/fixtures/README.md`、
  `THIRD_PARTY_NOTICES.md`、`crates/zant-sys/prebuilt/README.md`、`crates/zant-examples/**/README.md`
  删除「迁移中」「只保留到 M8」「M8 起移入」等过程性文字，路径指向最终位置。
  `docs/rust-port/*` 是迁移记录，**刻意保留**（含 Zig 变更日志、WP 报告、验证数字）。

脚本最后断言：`git ls-files '*.zig'` 为空，`build.zig` / `build.zig.zon` / `tools/` / `src/` /
`examples/` / `scaffold/template/` 不存在，新路径全部就位。

### 3. 切换后验收（`--apply` 默认执行；也可 `--verify-only` 单独跑）

1. `cargo build --workspace`
2. `cargo test --workspace`
3. `cargo clippy --workspace --all-targets -- -D warnings`
4. `cargo fmt --all -- --check`
5. `scripts/tsx/build-template-check.sh`
6. `scripts/tsx/examples-check.sh`
7. `scripts/parity/bench.sh`
8. `git ls-files '*.zig'` 为空、Zig 目录不存在

（cargo 在沙箱里走 `scripts/cargo.sh`。）

## 切换后哪些对照能力消失 / 如何替代

Zig 工具链和 `tools/` 删除后：

- **布局 / 像素 parity**（`scripts/parity/compare.sh`、`crates/zant-examples/parity/*`）不再能自动跑：
  它们需要 Zig 基线二进制。`zig-out-baseline/` 是 gitignore 的本地产物，脚本**不删**；主力机器上
  它还在时这些工具仍可手工用（`ZANT_ZIG_BIN=<dir>`）。文档与 README 已把 parity 标为迁移期工具。
- **帧基准** `scripts/parity/bench.sh`：没有 Zig 基线时自动进入 **Rust-only 模式**——跑 Rust 侧全部
  场景，并和**记录在 `docs/rust-port/status.md` 的 Zig 基线数字**（脚本内 `recorded_default` /
  `recorded_list`）对照，1.25× 闸门与 16.7 ms 稳态预算照旧；表格会标注 baseline 是 recorded。
  `ZANT_BENCH_BASELINE=recorded` 可强制该模式；`ZANT_ZIG_BIN=<dir>` 可恢复直接对照。
  性能测量受机器负载影响（sub-ms 场景尤其明显）：cutover 的 `--apply` 验收用
  `ZANT_BENCH_REPEAT=12`（best-of-12）；已知机器很忙时可设 `ZANT_M8_BENCH_ADVISORY=1` 把 bench
  失败降为警告，发布前必须在安静机器上复跑 `scripts/parity/bench.sh`。
- **TSX 编译基准** `scripts/tsx/examples-check.sh`：Zig 编译器基线（`tools/tsx/compile_bench.zig`）
  删除后只跑 Rust 侧 `zant-tsx-bench`，断言 32 个语料中的 30 个共同语料全部编译（记录的 pre-M8
  比值是 1.08–1.10×，见 `status.md` WP-5.10）。编译耗时不是帧耗时，不设绝对时间闸门。

## 已知的迁移后遗留（不影响构建）

- `crates/zant-sys/prebuilt` 的发布包预编译库需要重新生成：`sources-*.txt` 路径变了，
  `source_hash` 变化，旧归档会被 `build.rs` 判为过期并回退源码编译。发布前跑
  `scripts/release/prebuilt.{sh,ps1}` 重新生成（release workflow 会做）。
- `tests/fixtures/`（remend 522 + mhchem 823 以外还有 `widgets.pdf`）保留；`tests/tsx/` 的 Zig
  项目 fixture 删除，Rust 镜像在 `crates/zant-tsx/tests/fixtures/`。
- `crates/zant-sys/src/{skia,platform,pdf}.rs` 的 header 注释随 `gen_bindings.sh` 更新；绑定内容
  本身不变（`ZANT_STATIC_DATA` 守卫删除只是把两个 setter 变成无条件声明）。CI 仍会重跑 bindgen 并
  `git diff --exit-code`，没装 libclang 的机器不必手动重生成。

## 用户需要决定 / 之后要做的事

1. **批准切换**（本脚本只准备和演练，不执行）：对 `feat/rust-port` 跑
   `scripts/migrate/cutover-m8.sh --apply`，然后 review `git diff --stat` 与生成的 diff。
2. **合并到 `main`**：M8 切换提交合并后，`feat/rust-port` → `main`。
3. **打 `v*` 标签发布**：推送标签触发 `.github/workflows/release.yml`（重新生成预编译原生库、
   构建两个平台发布包）。发布前必须得到用户明确同意。
4. **Windows 机器补验**：本仓库的 Windows 测试机全程不可达；M8 之后需要在 Windows 上补
   `cargo test --workspace`、`cargo build --workspace`、`scripts/win/{components,smoke-stillmd,docs,pdf,
   resize,stream-resize,scaffold}-check.ps1`、`install.ps1` → `zant create` → `zant run` → `zant build`
   的端到端。脚本改的 Windows 路径（`target\debug\...`）未在真机验证。
5. **`zig-out-baseline/` 与 `.zig-global-cache/`**：本地产物，不提交；是否删除由用户决定
   （删了就没有 parity 对照的基线，只剩 `status.md` 里的记录数字）。

## 演练方式（在一次性副本里）

```bash
# 1. 独立 clone（不要 cp 本 worktree：它的 .git 是 worktree 指针，会改到主仓库）
git clone --no-hardlinks --branch feat/rust-port /Users/xiaoyi/dev/XyUI /tmp/m8-rehearsal
cd /tmp/m8-rehearsal
# 2. 依赖软链（clone 不含 gitignore 的 third_party / cargo home）
ln -s /Users/xiaoyi/dev/XyUI/.cargo-home .cargo-home
ln -s /Users/xiaoyi/dev/XyUI/third_party third_party
# 3. 把本 WP 的脚本与文档覆盖进来（或等 lead 合并分支后直接 clone 该分支），提交成干净树
cp -R <worktree>/scripts/migrate scripts/
cp <worktree>/docs/rust-port/m8-cutover.md docs/rust-port/
git add -A && git commit -m "rehearsal: M8 cutover"
# 4. 切换（演练不需要前置质量门与 Zig 基线）
CARGO_HOME=/Users/xiaoyi/dev/XyUI/.cargo-home ./scripts/migrate/cutover-m8.sh --apply --skip-gates
# 5. 真实验收
./scripts/cargo.sh build --workspace
./scripts/cargo.sh test --workspace
./scripts/cargo.sh clippy --workspace --all-targets -- -D warnings
./scripts/cargo.sh fmt --all -- --check
./scripts/tsx/build-template-check.sh
./scripts/tsx/examples-check.sh
./scripts/parity/bench.sh
git ls-files '*.zig'   # 必须为空
```

## 演练记录（WP-M8-PREP，2026-11-14，本机 macOS arm64）

见 `docs/rust-port/status.md`「M8 切换脚本就绪」。演练在一次性副本里做，**没有动主 checkout 和
`feat/rust-port`**：

- 演练目录：`/tmp/m8-rehearsal` = `git clone --no-hardlinks --branch rust/wp-m8-prep`（本 WP 分支；
  等价于把脚本 / 文档覆盖到 `feat/rust-port` 后的树）。clone 不含 gitignore 的依赖，手动软链
  `.cargo-home` / `third_party`。`--skip-gates` 只跳过前置质量门与 Zig 基线检查（副本没有
  `zig-out-baseline/`），切换后的真实验收在副本里单独跑。
- **dry-run**：`bash -n scripts/migrate/cutover-m8.sh` 通过；在干净副本上 dry-run 退出 0，列出
  54 个 `git mv`、755 个 `git rm`、78 处文件改写、4 个整文件重写，`git status` 无变化。
- **切换**：`scripts/migrate/cutover-m8.sh --apply --skip-gates` 退出 0；脚本自身断言
  `git ls-files '*.zig'` 为空、`build.zig` / `build.zig.zon` / `tools/` / `src/` / `examples/` /
  `scaffold/template/` 不存在、新路径全部就位。`git status` 只有预期的 `R` / `D` / `M`
  （58 个 renamed、11 个 renamed+modified、388 deleted、70 modified）。之后脚本内
  `cargo fmt --all` 只收敛被改写行（例如 `mhchem/tests.rs`、`windows.rs` 的长行）。
- **真实验收（副本内；`--verify-only` 也会原样重跑这套）**：
  - `cargo build --workspace`：通过（只有既有的 pdf.mm clang 警告与 `app_config.rs` 提示）。
  - `cargo test --workspace`：**71 个测试套件 / 1057 passed / 0 failed**。与切换前
    `status.md` 记录的 71 / 1059 相比少 2 条，正是随纯 Zig 模板一起删除的
    `create_lang_zig_still_copies_the_legacy_template` 和 `project.rs` 的旧语言探测（`Language::Zig`）
    测试；其余（remend 522、mhchem 823 等夹具）全在。
  - `clippy --workspace --all-targets -- -D warnings`：0 告警；`fmt --all -- --check` 干净。
  - `scripts/tsx/build-template-check.sh`：通过（`scaffold/template-ts end-to-end: OK (debug build)`；
    模板现在自带 `DESIGN.md`，`zant.d.ts` 从 `crates/zant-tsx/zant.d.ts` 复制）。
  - `scripts/tsx/examples-check.sh`：通过（3 个 TSX 示例构建 + 无窗口截图 / 布局 key / 确定性断言；
    编译基准进入 Rust-only 模式，30 个共同语料全部编译）。
  - `scripts/parity/bench.sh`（Rust-only，对照 `status.md` 记录的 Zig 基线；`--apply` 验收默认
    best-of-12，与 WP-2.11 的 best-of 方法一致）：
    default **7/7 PASS**（open 1.02×、stream 0.96×、typing 1.18×、caret 0.97×、hover 1.00×、
    resize 0.95×、scroll 0.99×），virtual list **7/7 PASS**（open 1.04×、stream 1.04×、typing 1.03×、
    caret 1.02×、hover 1.00×、resize 1.01×、scroll 1.03×）。演练时机器 load average ~18（其他构建
    并行）：best-of-3 时 sub-ms 场景会抖到 1.26–2.2×，best-of-8 通过，best-of-12 稳定通过；被机器
    负载压到失败时可用 `ZANT_M8_BENCH_ADVISORY=1` 把 bench 降为警告（发布前在安静机器上复跑）。
    同一副本用 `ZANT_ZIG_BIN` 指向主 checkout 的 `zig-out-baseline` 直接对照也跑过：
    default 7/7、virtual list 6/7（`hover` 1.53×，sub-ms 噪声），说明剩余波动来自机器负载而不是
    Rust 侧回归。
  - `git ls-files '*.zig'`：空。
- **最终 `--verify-only`**（干净 `--apply --skip-gates` 之后、用最终脚本）：**exit 0**，上面 8 项
  全部通过（`✓ all post-M8 gates passed`）。

**结论**：切换脚本在一次性副本里端到端通过，可以等用户批准后在 `feat/rust-port` 上执行。
