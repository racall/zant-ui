# Rust 迁移约定（conventions）

本文件是 [plan.md](plan.md) §3 的展开版。**每个参与迁移的执行者动手前必读**；
与 plan.md 冲突时以 plan.md 为准（但本文件的「本仓库环境」一节是当前机器上实测的事实，优先遵守）。

## 全局规则

1. **直译优先**。移植 = 把 Zig 文件的行为原样搬到 Rust，函数名转 snake_case，逻辑顺序、常量、
   边界条件一律不改。看到「可以顺手优化」的地方，不做，在报告里提一句即可。
2. **不改原生代码**。`*.c / *.m / *.mm / *.cpp / *.h` 只读。唯一例外是 WP-0.3 里用
   `#ifdef ZANT_STATIC_DATA` 守卫的那一处增补。
3. **不改 Zig 代码的行为**。Zig 只接受 bug 修复；修了就要在 `status.md` 的「Zig 变更日志」登记，
   已移植的对应模块要同步。
4. **不私自加依赖**。允许的 crate 只有根 `Cargo.toml` 的 `[workspace.dependencies]` 里列出的。
   需要新依赖 → 登记 `status.md` 的「待决问题」。
5. **`unsafe` 只允许出现在**：`crates/zant-sys`、`crates/zant/src/renderer/`、
   `crates/zant/src/platform/`、`crates/zant/src/image.rs`、`crates/zant/src/pdf/native.rs`。
   其余模块由 `crates/zant/src/lib.rs` 的 `#![forbid(unsafe_code)]` 兜底，不要 `#[allow(unsafe_code)]`。
6. **测试同名移植**。每个 Zig `test "名字"` 对应一个 Rust `#[test] fn 名字_snake_case()`，
   断言逐条对应。夹具 JSON（`tests/fixtures/remend_cases.json`、`src/math/mhchem_cases.json`）
   原文件共用，用 `include_str!` 引用，不复制。
7. **每个 WP 的统一验收**：
   ```bash
   ./scripts/cargo.sh fmt --all -- --check      # 由 lead 在合并前统一跑
   ./scripts/cargo.sh clippy --workspace --all-targets -- -D warnings
   ./scripts/cargo.sh test -p <crate> <模块过滤>
   ZIG_GLOBAL_CACHE_DIR=$PWD/.zig-global-cache zig build test   # 确认没碰坏 Zig 版
   ```
   涉及平台或渲染的 WP，macOS 和 Windows 都要过。
8. **界面相关 WP 遵循 `DESIGN.md`**，完成后按其第 6 条检查不同窗口尺寸、浅 / 深色主题、各交互状态；
   这里的做法是跑对照脚本（plan.md §6），Rust 版必须与 Zig 版一致。
9. **提交**：提交信息英文。不提交 `target/`、`zig-out*`、`third_party`、`.cargo-home`、密钥、本机绝对路径。
   并行期间所有 WP 都在 `feat/rust-port` 工作区里提交（见下文「本仓库环境」）。
10. **进度登记**：领取、完成、阻塞都更新 `status.md` 里自己那一行。

## 本仓库环境（当前机器实测）

| 事实 | 处理方式 |
|---|---|
| DSH 沙箱只允许写工作区目录；`~/.cargo` 不可写 | **一律用 `./scripts/cargo.sh` 代替 `cargo`**（它把 `CARGO_HOME` 指到 `.cargo-home/`，已 gitignore） |
| `~/.cache/zig` 不可写 | Zig 命令加 `ZIG_GLOBAL_CACHE_DIR=$PWD/.zig-global-cache` |
| 已装的 Rust 是 `stable` = 1.94.1；沙箱内 `rustup` 不能安装精确版本 | `rust-toolchain.toml` 用 `channel = "stable"`，`rust-version = "1.94"` 兜底最低版本 |
| 多个执行者共用同一工作区与 `target/` | 不同时提交；cargo 会用文件锁串行化构建，等待即可 |
| `cargo fmt --all` / `cargo clippy` 会扫到别人正在写的文件 | 迭代期间只格式化**自己的**文件：`rustfmt --edition 2024 <file.rs>` |
| 同一 crate 里多人并行改不同模块 | 编译错误出现在**不属于你**的文件时不要修，等几十秒重试；保证你自己的文件先编译通过 |
| Windows 测试机 192.168.31.24 当前不可达 | 所有需要 Windows 的验收先记为「待验证」，在 status.md 注明 |

## 代码组织

- `crates/zant/src/` 的目录与 Zig 的 `src/` 一一对应，方便对照；模块文件尽量 ≤ 500 行，
  按 plan.md §2 的拆分表拆大文件。
- 每个 `mod.rs`（或 Rust 的 `mod x;` 声明）里写一行的模块职责注释，指向对应 Zig 文件路径。
- 错误类型集中在 `zant::Error`（`thiserror`），只在 Zig 有真实失败（IO、解析、SQLite）时使用；
  纯 OOM 的错误一律去掉（Rust 分配失败即 abort）。

## Zig → Rust 翻译对照表

| Zig | Rust |
|---|---|
| `[]const u8` 参数 | `&str`；需要保存时用 `zant::Str` |
| `?T` / `orelse` | `Option<T>` / `.unwrap_or(..)` |
| `!T` 且唯一失败原因是 OOM | 去掉错误，直接返回 `T` |
| `!T` 有真实失败（IO、解析、SQLite） | `Result<T, zant::Error>`（`thiserror`） |
| `catch {}` | `let _ = ..;` 或 `.ok();` |
| `catch @panic(..)` / `catch unreachable` | `.expect("同一句话")` |
| `std.mem.Allocator` 参数 | 删除 |
| `ArenaAllocator`（帧 / 节点 arena） | `Vec` / `String` / `Rc`，不引入 arena |
| `ArrayListUnmanaged(T)` | `Vec<T>` |
| `StringHashMap(V)` | `HashMap<String, V>` |
| `union(enum)` | `enum` |
| `extern struct` / `extern fn` | 用 `zant-sys` 的 bindgen 结果 |
| `*anyopaque` + 函数指针回调 | `Rc<dyn Fn(..)>` |
| `comptime func` 生成的 `Adapter` 蹦床 | 闭包 |
| `fn X(comptime T: type) type` | 泛型 `struct X<T>` |
| `anytype` | 泛型 + trait 约束 |
| `@hasDecl` 探测可选钩子 | trait 的默认方法 |
| `@typeInfo` 遍历字段（哈希、相等、拷贝） | `#[derive(Hash / PartialEq / Clone)]` |
| `std.meta.eql(a, b)` | `a == b`（类型 derive `PartialEq`） |
| `@typeName(T)` 做类型标识 | `std::any::TypeId::of::<T>()` |
| `threadlocal var` | `thread_local!` + `Cell` / `RefCell` |
| 模块级 `var` | `static` + `Mutex` / `AtomicX` / `OnceLock` |
| `@embedFile` | `include_bytes!` / `include_str!` |
| 带默认值的选项结构体 + `.{ .a = 1 }` | `impl Default` + `X { a: 1, ..Default::default() }` |
| `defer x.deinit()` | 实现 `Drop` |
| `@intCast(x)` | `x.try_into().expect("int cast")`；确定不溢出的用 `as` |
| `@divTrunc` / `@rem` | `/` / `%`（整数语义相同） |
| `std.Thread.Mutex` / 条件变量（`core/sync.zig`） | `std::sync::{Mutex, Condvar}`，`sync.zig` 不移植 |
| `std.compress.flate` + `core/png.zig` | `png` crate |

## 命名

- 函数 / 变量 / 模块文件：snake_case；类型 / 枚举：PascalCase；常量：SCREAMING_SNAKE_CASE。
- 枚举变体保留 Zig 名字的语义，但避免 Zig 的缩写习惯乱改：`textColor` → `text_color`，
  `maxLines` → `max_lines`。
- 选项结构体字段顺序与 Zig 一致（便于逐字段对照），首字段通常是 `key`。
- 与 Zig 同名的常量表（如 `keys.zig` 的标签表）保留原值，**逐字节可对照**。

## WP 完成检查单

- [ ] 所有目标 Zig 文件的每个 `pub fn` / 公开类型都有对应 Rust 项（漏掉的列出来）
- [ ] 每个 Zig `test` 都有同名 Rust 测试，且通过
- [ ] `./scripts/cargo.sh clippy -p <crate> --all-targets -- -D warnings` 无警告
- [ ] 自己文件已 `rustfmt --edition 2024`
- [ ] `ZIG_GLOBAL_CACHE_DIR=$PWD/.zig-global-cache zig build test` 仍通过
- [ ] 报告：文件清单、测试数量、验收命令、偏差与待决问题
