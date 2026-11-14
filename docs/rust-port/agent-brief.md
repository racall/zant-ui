# 并行执行者工作说明（agent brief）

本文件给所有并行执行同一仓库迁移任务的 AI 执行者。**动手前按顺序读完**。

1. `docs/rust-port/plan.md`：总计划。重点 §3（全局规则）、§4（API 契约，已冻结）、
   §5 里你领取的 WP。
2. `docs/rust-port/conventions.md`：Zig→Rust 翻译对照表、本仓库的环境限制、完成检查单。
3. 你负责的 Zig 源文件：**逐个完整读完**，再开始写 Rust。

## 工作区与分支

- 仓库：`/Users/xiaoyi/dev/XyUI`，集成分支 `feat/rust-port`。
- 你在一份 **git worktree** 里工作，路径形如 `/Users/xiaoyi/dev/XyUI/.worktrees/wp-3.1`，
  对应分支 `rust/wp-3.1`。你只在这份 worktree 里读写、构建、提交。
- worktree 里已经软链好 `.cargo-home`、`.zig-global-cache`、`third_party`；
  **不要删除或重指它们**，也不要提交它们（已在 `.gitignore`）。
- 完成后在 worktree 里提交：`git add -A && git commit -m "WP-x.y: <一句话>"`（英文信息）。
  lead 负责把各分支合入 `feat/rust-port` 并统一跑 `cargo fmt` / 全量测试。
- 不要 `git push`，不要切分支，不要 rebase/merge。

## 构建与测试

```bash
cd /Users/xiaoyi/dev/XyUI/.worktrees/wp-3.1
./scripts/cargo.sh test -p zant remend        # 只看自己的模块
./scripts/cargo.sh clippy -p zant --all-targets
rustfmt --edition 2024 crates/zant/src/markdown/remend/*.rs   # 只格式化自己的文件
zig build test   # 不需要跑：lead 合并后统一跑
```

- 沙箱里 `~/.cargo` 不可写，所以**必须用 `./scripts/cargo.sh`**，不要直接 `cargo`。
- **不要跑 `cargo fmt --all` / `cargo fmt`**：它会重排别人分支上的文件（你是独立 worktree，
  但仍保持这个习惯，便于 merge）。
- 需要新 crate 依赖 → **不要自己加**，在报告里写清楚（登记到 `status.md` 的待决问题由 lead 做）。

## 翻译规则（摘要，全文见 conventions.md）

- **直译优先**：函数名转 snake_case，逻辑顺序、常量、默认值、边界条件一律不改；
  不做「顺手优化」。发现优化点写进报告。
- **不改 Zig 代码**（`.zig` 只读）。**不改 C/C++/ObjC 原生代码**。
- 每个 Zig `test "名字"` → 一个 `#[test] fn 名字_snake_case()`，断言逐条对应。
  夹具用 `include_str!` 引用仓库里已有的 JSON，不复制。
- `unsafe` 不许出现在你的模块（crate 根是 `#![deny(unsafe_code)]`）。
- 文件 ≤ 500 行左右；目标路径已按 `plan.md` §2 拆好，模块桩文件已存在，直接写内容即可。
- 只写你 WP 列出的路径。**不要**改 `lib.rs` / 各级 `mod.rs`（模块声明已预先写好）、
  根 `Cargo.toml`、其他 WP 的文件。确实需要加模块声明时，在报告里说明，由 lead 统一改。
- 如果 `cargo check` 报的错误出现在**不是你负责的文件**里（本 worktree 应该不会出现），
  报告即可，不要改。

## 完成时报告（必须）

1. 分支名与提交 hash；
2. 新建/修改的文件清单（对着 Zig 源逐个确认）；
3. 测试数量与结果（`./scripts/cargo.sh test -p <crate> <filter>` 的通过数），
   以及对应 Zig 测试数量（应当相等）；
4. 你跑过的验收命令；
5. 偏差、未移植项、发现的 Zig bug、需要 lead 决策的问题。

有「计划没写清」的设计问题 → **停下来**，在报告里给出 2–3 个候选方案+推荐，
不要自行发明并继续。
