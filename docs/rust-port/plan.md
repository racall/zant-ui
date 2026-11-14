# Zant UI：Zig → Rust 迁移计划

> 本计划会交给多个模型分工执行。每个工作包（WP）自带输入、输出、要点和验收命令。
> 执行者只做自己领取的 WP，遇到本计划没写清的设计问题，**停下来在 `docs/rust-port/status.md` 的「待决问题」里登记**，不要自行发明方案。

## 1. Context（为什么做、做成什么样）

Zant UI 现在是 Zig 0.17.1 + Skia 的原生桌面 UI 框架。要整体换成 Rust，动机四条：
crates 生态、语言与工具链稳定（Zig 未到 1.0）、AI 与协作者写 Rust 更可靠、借用检查带来的内存与并发安全。

最终目标：**像写 React 一样用 TSX 写 Zant 桌面应用**。TSX 编译成 Rust（编译期完成，无 JS 运行时），
应用的原生逻辑写在 `native/*.rs`。手写 Rust 的 `ui::*` API 只求框架内部和代码生成够用，不追求人体工学。

### 已确定的决策（不要再讨论）

| 主题 | 决定 |
|---|---|
| 路线 | 同仓库并行重写。Zig 版保持可构建，作为行为对照；全部对齐后才删除 |
| 起点 | 从 `feat/tsx-compiler`（HEAD `905955f`）拉出 `feat/rust-port` |
| 应用层 | TSX 为主；Rust API 是内部 API 和 codegen 目标 |
| Skia / 平台 / PDF | **原生 C / C++ / ObjC 代码原样保留**（约 7,400 行），C ABI 不变，Rust 侧只重写 extern 包装 |
| Windows | `x86_64-pc-windows-msvc`，原生代码静态链接进应用；删除 `zant_skia.dll`、`.def`、`gen_def.sh`、dlltool |
| TSX 前端 | lexer / parser / AST / hook 展开按文件 1:1 直译成 Rust，不引入 oxc / swc |
| SQLite | 用 `rusqlite`（`bundled` 特性），删除手写 C 声明和 `winsqlite3.def` |

### 现状规模（HEAD `905955f`）

| 部分 | 规模 | 处理 |
|---|---|---|
| `src/`（Zig 框架） | 36,621 行 / 125 文件 | 移植 |
| `tools/tsx/`（TSX→Zig 编译器） | 手写 15,146 行 + 测试 2,482 行 | 前端直译，`codegen/` 9,612 行重写为生成 Rust |
| `tools/zant/`（CLI） | 3,005 行 | 移植，`zig build` 改 `cargo` |
| `tests/` | 6,154 行；全仓库约 382 个 test 块 | 同名移植 |
| `examples/` | 2,575 行 Zig + TSX 示例 | 移植 |
| `build.zig` | 871 行 | 拆成 `zant-sys/build.rs` + `zant-build` crate + CLI |
| 原生 C/C++/ObjC | 约 7,400 行 + 头文件 700 行 | **保留** |
| 仓库外下游 | `../cliphop-ts`（1,580 行 TSX + 3,400 行 Zig native）、`../stillmd`（2,020 行 Zig）、`../cliphop`（3,990 行 Zig） | 见 M7 |

三个 C ABI 是整个迁移的稳定地基：`src/renderer/skia/skia_bridge.h`（54 个函数）、
`src/platform/platform.h`（50 个函数 + `ZantHost` 回调表）、`src/platform/pdf.h`（12 个函数）。

---

## 2. 目标结构

```
Cargo.toml                 # workspace，[workspace.dependencies] 统一版本
rust-toolchain.toml        # 固定 stable 版本（M0 时取当时最新 stable），edition 2024
crates/
  zant-sys/                # FFI：build.rs 编译原生代码并链接 Skia；src/ 放签入的 bindgen 结果
  zant-macros/             # proc-macro：#[derive(TsxProps)]、#[derive(Table)]
  zant/                    # 框架本体（单 crate，因为 core ↔ components 互相依赖）
    src/{lib.rs, ui/, runtime/, core/, layout/, components/, renderer/, platform/,
         motion/, theme/, markdown/, math/, pdf/, db/, desktop/, tsx/, store.rs, image.rs}
    tests/                 # 对应现在的 tests/*.zig
  zant-tsx/                # TSX→Rust 编译器（lib + bin `zant-tsx`），testdata/ 放黄金用例
  zant-build/              # 应用 build.rs 用的辅助库（替代 addTsxApp / configureApp）
  zant-cli/                # bin `zant`
  zant-examples/           # src/bin/*.rs，二进制名与现在一致：zant-demo、zant-components …
docs/rust-port/            # plan.md（本文件副本）、conventions.md、api.md、status.md
scripts/parity/            # Zig 版与 Rust 版对照脚本
```

迁移期间原生源码**留在原位**（`src/platform/**`、`src/renderer/skia/**`），两套构建共用；
M8 删除 Zig 时再移到 `crates/zant-sys/native/`。`third_party/` 和 `scripts/fetch_skia*.{sh,ps1}` 不变。

`crates/zant/src/` 的目录与 Zig 的 `src/` 一一对应，方便对照。
Zig 的大文件必须拆分（项目规定单文件约 500 行以内）：

| Zig 文件 | Rust 目标 |
|---|---|
| `src/ui.zig`（1,946） | `ui/{mod, element, options, builders, actions, styled, context, memo, commands, portals}.rs` |
| `src/ui_runtime.zig`（2,721） | `runtime/{mod, build, reconcile, patch, dispatch, pointer, keyboard, ime, focus, selection, overlay, animate, frame, host, debug}.rs` |
| `src/core/tree.zig`（1,041） | `core/tree/{mod, alloc, paint, hit, scroll, input_ops, focus}.rs` |
| `src/core/node.zig`（813） | `core/node/{mod, text, rich_text, data}.rs` |
| `src/core/layout.zig`（697） | `core/layout/{mod, intrinsic, container, row, column, wrap}.rs` |
| `src/math/mhchem.zig`（2,565） | `math/mhchem/` 下按状态机 / 模式表 / 输出拆分 |
| `src/pdf/viewer.zig`（1,666） | `pdf/viewer/` 下按滚动 / 缩放 / 选择 / 搜索 / 批注拆分 |

---

## 3. 全局规则（每个执行者必读）

1. **直译优先**。移植 = 把 Zig 文件的行为原样搬到 Rust，函数名转 snake_case，逻辑顺序、常量、边界条件一律不改。
   看到「可以顺手优化」的地方，不做，在 PR 描述里提一句即可。
2. **不改原生代码**。`*.c / *.m / *.mm / *.cpp / *.h` 只读。唯一例外是 WP-0.3 里用 `#ifdef ZANT_STATIC_DATA` 守卫的那一处增补。
3. **不改 Zig 代码的行为**。Zig 只接受 bug 修复；修了就要在 `status.md` 的「Zig 变更日志」登记，已移植的对应模块要同步。
4. **不私自加依赖**。允许的 crate 只有 `Cargo.toml` 的 `[workspace.dependencies]` 里列出的。需要新依赖 → 登记待决问题。
5. **`unsafe` 只允许出现在**：`zant-sys`、`zant/src/renderer/`、`zant/src/platform/`、`zant/src/image.rs`、`zant/src/pdf/native.rs`。
   其余模块文件头加 `#![forbid(unsafe_code)]`（模块级用 `#[forbid(unsafe_code)] mod x;`）。
6. **测试同名移植**。每个 Zig `test "名字"` 对应一个 Rust `#[test] fn 名字_snake_case()`，断言逐条对应。
   夹具 JSON（`tests/fixtures/remend_cases.json`、`src/math/mhchem_cases.json`）原文件共用，用 `include_str!` 引用，不复制。
7. **每个 WP 的统一验收**（下文各 WP 只列额外项）：
   ```
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test -p <crate> <模块过滤>
   zig build test          # 确认没碰坏 Zig 版
   ```
   涉及平台或渲染的 WP，macOS 和 Windows 都要过（Windows 用 `scripts/win/sync.sh` + `remote.sh`）。
8. **界面相关 WP 遵循 `DESIGN.md`**，完成后按其第 6 条检查不同窗口尺寸、浅 / 深色主题、各交互状态；
   这里的做法是跑对照脚本（§6），Rust 版必须与 Zig 版一致。
9. **提交**：一个 WP 一个分支 `rust/<WP 编号>`，合入 `feat/rust-port`。提交信息英文。不提交 `target/`、`zig-out`、`third_party`、密钥、本机绝对路径。
10. **进度登记**：领取、完成、阻塞都更新 `docs/rust-port/status.md` 里自己那一行。

### Zig → Rust 翻译对照表（写入 `docs/rust-port/conventions.md`）

| Zig | Rust |
|---|---|
| `[]const u8` 参数 | `&str`；需要保存时用 `zant::Str`（见 §4） |
| `?T` / `orelse` | `Option<T>` / `.unwrap_or(..)` |
| `!T` 且唯一失败原因是 OOM | 去掉错误，直接返回 `T`（Rust 分配失败即 abort） |
| `!T` 有真实失败（IO、解析、SQLite） | `Result<T, zant::Error>`（`thiserror` 定义） |
| `catch {}` | `let _ = ..;` 或 `.ok();` |
| `catch @panic(..)` / `catch unreachable` | `.expect("同一句话")` |
| `std.mem.Allocator` 参数 | 删除 |
| `ArenaAllocator`（帧 arena、节点 arena） | 用自有类型（`Vec` / `String` / `Rc`），不引入 arena |
| `ArrayListUnmanaged(T)` | `Vec<T>` |
| `StringHashMap(V)` | `HashMap<String, V>` |
| `union(enum)` | `enum` |
| `extern struct` / `extern fn` | 用 `zant-sys` 的 bindgen 结果，不手写 |
| `*anyopaque` + 函数指针回调 | `Rc<dyn Fn(..)>`（见 §4 Action） |
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
| `std.fmt.allocPrint(arena, "{s}-thumb", .{base})` | `format!("{base}-thumb")` |
| `defer x.deinit()` | 实现 `Drop` |
| `@intCast(x)` | `x.try_into().expect("int cast")`；确定不溢出的用 `as` |
| `@divTrunc` / `@rem` | `/` / `%`（整数语义相同） |
| `std.Thread.Mutex` / 条件变量（`core/sync.zig`） | `std::sync::{Mutex, Condvar}`，`sync.zig` 不移植 |
| `std.compress.flate` + `core/png.zig` | `png` crate |

`Cargo.toml` 的 `[profile.release]` 设 `overflow-checks = true`，保持 Zig 整数溢出即崩溃的语义。

---

## 4. 核心 API 契约（WP-1.x 实现，之后冻结；写入 `docs/rust-port/api.md`）

所有后续 WP 都依赖这一节。实现时发现行不通，登记待决问题，由负责人修订后统一通知。

**字符串** `zant::Str`：廉价克隆的不可变字符串。内部 `enum { Static(&'static str), Shared(Rc<str>) }`，
实现 `Deref<Target = str>`、`From<&'static str>`、`From<String>`、`Clone`、`PartialEq`、`Hash`、`Default`。
非静态 `&str` 用 `Str::copy(s)`。

**声明节点**（对应 `src/ui.zig:14-87`）：
```rust
#[derive(Clone)] pub struct Node(Rc<Element>);
#[derive(Clone, Default)] pub struct Element { pub kind: NodeKind, pub key: Option<Str>,
    pub children: Vec<Node>, /* 其余约 55 个字段与 Zig 的 Element 同名同序 */ }
```
节点自己拥有数据，没有帧 arena，也没有生命周期参数。`ui::make()` 的深拷贝逻辑不需要移植。

**选项结构体**：`BoxOptions`、`TextOptions`、`ButtonOptions`、`InputOptions` … 字段与 Zig 同名。
全部加 `#[derive(Clone, zant_macros::TsxProps)]` 并**手写 `impl Default`，默认值逐字段对照 Zig**（Zig 里非零默认值很多，不能直接 derive）。
调用形态：`ui::button(ButtonOptions { text: "Save".into(), variant: ButtonVariant::Primary, ..Default::default() })`。

**子节点列表**：宏 `nodes![a, b, maybe, list]`，元素可以是 `Node`、`Option<Node>`、`Vec<Node>`，展平成 `Vec<Node>`（trait `IntoNodes`）。

**回调**（对应 `src/core/actions.zig`）：
```rust
#[derive(Clone)] pub struct Action(Rc<dyn Fn(&mut ActionCx)>);
// 同样形状：BoolAction(bool)、TextAction(&str)、IndexAction(usize)、RangeAction、
// PointerAction（返回 bool）、KeyAction、InputKeyAction —— 与 actions.zig 的类型一一对应
pub struct ActionCx<'a> { /* Runtime 分发事件时构造 */ }
impl ActionCx<'_> { pub fn state<S: 'static>(&mut self) -> &mut S; /* 当前窗口的根状态，类型不符则 panic */ }

pub fn action<S: 'static>(f: impl Fn(&mut S) + 'static) -> Action;          // 替代 ui.action(self, State.save)
pub fn text_action<S: 'static>(f: impl Fn(&mut S, &str) + 'static) -> TextAction;
// bool_action / index_action / … 同理；Action::new(|cx| ..) 是不绑定根状态的通用构造
```
Runtime 调用回调的固定顺序：**先把 `Action` 从树节点里 clone 出来，释放对树的借用，再构造 `ActionCx` 调用**。

**应用入口**（替代 `launch.zig` 的 `@hasDecl` 探测）：
```rust
pub trait App: 'static {
    fn render(&mut self) -> Node;
    fn theme(&self) -> Option<Theme> { None }
    fn on_key(&mut self, _e: &KeyEvent) -> bool { false }
    fn on_close_request(&mut self) -> bool { true }
    // on_quit_request / on_start / on_wake / on_app_event / on_exit：默认空实现
}
pub fn run(app: impl App, opts: RunOptions) -> Result<(), Error>;
```

**局部状态**（对应 `ui.localState` / `core/local_state.zig`）：
`ui::local_state::<T>(id: u64, init: impl FnOnce() -> T) -> Rc<RefCell<T>>`。
存储按 `id` + `TypeId` 保存 `Rc<dyn Any>`；同一 id 两种类型 → panic；本次 rebuild 没被请求的槽位被清除（drop 即析构）。
memo 跳过的子树要保活它上次用到的 id（照搬 `local_state.zig:87-102`）。
回调里直接捕获这个 `Rc`：`Action::new(move |_| slot.borrow_mut().count += 1)`。
`ui::child_id(parent, name_or_index) -> u64` 的哈希算法照搬（Wyhash，`local_state.zig:141-157`），用 `wyhash` crate 或直译，**结果必须与 Zig 相同**以便对照。

**构建上下文**：Zig `ui.zig:94-301` 的 18 个 threadlocal 合并为 `ui/context.rs` 里的一个
`thread_local! { static CX: RefCell<BuildCx> }`，对外仍是 `ui::current_theme()`、`ui::effect()`、`ui::request_rebuild()` 这些自由函数。
**硬规则：调用用户代码（`render`、memo 的构建闭包、任何 Action）之前必须先释放 `CX` 的借用**，否则重入会 panic。

**保留树**（对应 `core/tree.zig`、`core/node.zig`）：`Tree { nodes: Vec<RetainedNode>, free_ids, key_index, animating, .. }`，
`NodeId = u32` 是下标，槽位带 `alive` 标志并复用（墓碑机制照搬）。节点数据 `enum NodeData { Root, Container(..), Text(..), Button(..), .. }`。
**硬规则：不跨越树的修改持有 `&mut RetainedNode`；递归前先把子节点 id 列表 clone 出来**。布局函数签名照 Zig，传 `&mut [RetainedNode]` + id。

**Skia 句柄**：`TextLayout`、`Image`、`Renderer`、`Canvas` 包装不透明指针并实现 `Drop`；默认 `!Send`，只有 `Image`（原本就线程安全创建）`unsafe impl Send + Sync`。

**性能闸门**：去掉 arena 后每帧会有更多堆分配。M2 末用移植后的 `zant-bench` 与 Zig ReleaseFast 基线对比，
要求各场景单帧耗时不超过 Zig 的 1.25 倍且远低于 16.7 ms；不达标再考虑 `SmallVec` / bump 分配，届时由负责人决定。

---

## 5. 里程碑与工作包

难度标记：★ 机械直译（任何模型）／★★ 需要对照运行调试／★★★ 设计性，交给最强模型或负责人。
「行数」是 Zig 源规模，用来估工作量。

### M0 基线与地基（串行，负责人做）

| WP | 内容 | 验收 |
|---|---|---|
| 0.1 ★★ | 建分支 `feat/rust-port`。两平台跑 `zig build test`，把真实通过数记进 `status.md`（文档写的 164/164 未核实）。ReleaseFast 构建安装到 `zig-out-baseline`。确认 Windows 测试机装了 rustup、VS Build Tools、与 `rust-toolchain.toml` 相同的工具链 | 两平台数字入档 |
| 0.2 ★ | 根 `Cargo.toml`（workspace + `[workspace.dependencies]`：`cc`、`thiserror`、`png`、`rusqlite`(bundled)、`serde`/`serde_json`、`wyhash`、`syn`/`quote`/`proc-macro2`/`prettyplease`、`clap`、`notify`、`embed-resource`；版本取当时最新稳定版）、`rust-toolchain.toml`、空 crate 骨架、`.gitignore` 加 `target/`。CI 新增 `rust.yml`：macos-15 与 windows-2022 上跑 fetch 脚本 + §3 第 7 条 | `cargo build --workspace` 两平台通过 |
| 0.3 ★★★ | **`zant-sys`**。`scripts/gen_bindings.sh` 用 bindgen CLI 从三个头文件生成 `src/{skia,platform,pdf}.rs` 并签入（用户机器不需要 libclang）；CI 检查生成结果没过期。`build.rs` 照搬 `build.zig:12-53`（macOS）和 `scripts/win/build_bridge.ps1`（Windows）的源文件列表、编译参数、链接库，细节见下 | 冒烟测试：创建 raster renderer，画一个矩形，读回像素；创建一个 TextLayout 并量宽度。两平台通过 |
| 0.4 ★★ | **对照工具** `scripts/parity/`（见 §6） | 用 Zig 版自己和自己比，全部一致 |
| 0.5 ★ | `docs/rust-port/`：`plan.md`（本文件）、`conventions.md`（§3）、`api.md`（§4）、`status.md`（WP 表格：编号 / 负责人 / 状态 / 分支 / 备注，外加「待决问题」「Zig 变更日志」两节） | 文件存在 |

**WP-0.3 细节**
- macOS：`cc` 编两组。C++ 组：`skia_bridge.cpp`（`-std=c++20 -fno-exceptions -fno-rtti`）、`skia_bridge_mac.mm` 与 `platform/macos/pdf.mm`（加 `-fobjc-arc`）。
  C 组：`platform/macos/window.m`（`-fobjc-arc -fblocks`）。include 路径 `third_party/skia-headers`。
  静态链接 `third_party/skia/skia-binaries/` 下的 `skia`、`skparagraph`、`skshaper`、`skunicode_icu`、`skunicode_core`；
  链接 14 个 framework（AppKit、Metal、MetalKit、QuartzCore、CoreText、CoreFoundation、CoreGraphics、IOKit、Security、CoreVideo、UniformTypeIdentifiers、PDFKit、Carbon、ServiceManagement）、`z`、`c++`。不再链接系统 `sqlite3`。
- Windows：`cc`（MSVC）编 `skia_bridge.cpp`、`skia_bridge_win.cpp`、`platform/windows/pdf_win.cpp`（`/std:c++20 /O2 /MD /EHs-c- /GR-`）和 `platform/windows/window.c`（C11）。
  链接 `third_party/skia-win/` 的 5 个 lib，以及 `build_bridge.ps1` 和 `build.zig:73` 列出的全部系统库。
- **ICU 数据和数学字体**：现在是 `rc.exe` 嵌进 DLL 的 RCDATA（id 101 / 102）。静态链接后改为：`zant-sys` 在 Windows 上
  `include_bytes!` 这两个文件，启动时通过新增的 `zant_set_icu_data(ptr, len)` / `zant_set_math_font(ptr, len)` 传给桥。
  这两个函数加在 `skia_bridge_win.cpp`，用 `#ifdef ZANT_STATIC_DATA` 包住，Zig 的 DLL 构建不定义这个宏所以不受影响。
  **动手前先读 `skia_bridge_win.cpp` 第 30–60 行，弄清现在怎么取资源**；这是全计划唯一允许的原生代码改动。
- `pdfium.dll` 仍是运行时加载，由 `zant-build` 负责拷到可执行文件旁（WP-6.2）。

### M1 核心骨架（WP-1.1 可并行；1.2 起串行，★★★ 的交给最强模型）

| WP | Zig 源 → Rust 目标 | 行数 | 依赖 |
|---|---|---|---|
| 1.1a ★ | `core/geometry.zig`、`theme/*`、`motion/*`、`layout/flex.zig` → 同名模块，连同各自测试和 `tests/flex_test.zig`（24 个）、`tests/motion_test.zig`（10 个） | ~900 | 0.2 |
| 1.1b ★ | `core/{unicode, keys, paths, hash, text_select}.zig` → 同名模块。`keys.zig` 的 comptime 标签表改成 `const` 表；`hash.zig` 改为 derive `Hash`，f32 字段用 `to_bits()` | ~900 | 0.2 |
| 1.1c ★ | `renderer/{canvas, text, rich_text, renderer}.zig`、`image.zig` → `renderer/`、`image.rs`：在 `zant-sys` 之上的安全包装（带 `Drop`） | ~500 | 0.3 |
| 1.1d ★★ | `platform/{platform, dialog}.zig`、`core/events.zig` → `platform/`：`Host` 回调表做成 `extern "C"` 蹦床转发到 `Box<dyn HostHandler>`；C 事件结构体转 Rust `enum Event` | ~900 | 0.3 |
| 1.2 ★★★ | `core/actions.zig`、`ui.zig` 的 `Element` / 选项结构体 / `make` / `styled` / 上下文 → `ui/`（§4 契约）。本包只做 `column` / `row` / `text` / `button` / `styled` 的构建器 | ~1,200 | 1.1 |
| 1.3 ★★★ | `core/node.zig`、`core/tree.zig`、`core/layout.zig`、`core/vector.zig`、`components/button.zig` → 保留树、布局、绘制、命中测试 | ~3,300 | 1.2 |
| 1.4 ★★★ | `ui_runtime.zig` 的构建 / 协调 / patch / 指针分发 / 帧循环 / 调试环境变量（`ZANT_SNAPSHOT`、`ZANT_SNAPSHOT_EXIT`、`ZANT_LAYOUT_FILE`、`ZANT_FRAME_STATS`、`ZANT_NO_MOTION`）、`launch.zig` → `runtime/`、`App` trait | ~1,500 | 1.3 |
| 1.5 ★★ | `zant-examples` 的 `zant-demo`（对应 `src/main.zig`）：窗口打开，文字和按钮可点 | — | 1.4 |

M1 验收：`zant-demo` 在两平台打开；`scripts/parity/compare.sh zant-demo` 布局完全一致、像素差在阈值内。§4 契约此时冻结。

### M2 组件与运行时补全（M1 之后，各 WP 可并行）

| WP | Zig 源 | 行数 | 说明 |
|---|---|---|---|
| 2.1 ★★ | `components/input.zig` + runtime 的键盘、IME、焦点、文本选择 | ~1,200 | 输入法在两平台真机验证；`scripts/win/smoke-stillmd.ps1` 的输入段是参考 |
| 2.2 ★ | `components/{toggle, select, image}.zig` + runtime 的 select 弹层、tooltip | ~800 | |
| 2.3 ★ | `ui.zig` 其余构建器（card、space、checkbox、switch、radioGroup、icon、pointerArea、clickable …）、`core/icons.zig` | ~1,500 | 逐个对照 `examples/components.zig` |
| 2.4 ★★ | `ui.memo`、`ui.localState` + `core/local_state.zig`、portals、effects、commands（focus / scrollIntoView / scrollToEnd）、`wakeAfter`、`keyBounds`、`postWake` | ~700 | memo 与局部状态保活逻辑照搬 |
| 2.5 ★ | 组合组件 `components/{menu, modal, segmented, message, shortcut_list, hotkey_input, view, window_controls}.zig` | ~1,000 | `view.zig` 用 `inline for` 拷字段的地方改为显式逐字段赋值 |
| 2.6 ★★ | `components/list.zig`（虚拟列表）+ `ui.ListState` | 584 | |
| 2.7 ★★ | 动效接入：presence、FLIP、`layout_transition`、runtime 的 `tick` | ~400 | 规范见 `docs/motion.md`；动画帧只重绘不重建 |
| 2.8 ★★ | `desktop.zig`、`store.zig`、多窗口（`open_runtimes`）→ `desktop/`、`store.rs`。`Store::update` 改成 `store.update(\|s\| ..)` | ~750 | 托盘、全局快捷键、剪贴板、单实例两平台手测 |
| 2.9 ★ | `examples/{declarative, components, gallery}.zig`、`src/gallery.zig`、`src/app.zig` → `zant-examples` | ~1,500 | `ZANT_AUTOPLAY=quiet` 保留 |
| 2.10 ★ | `tests/*_test.zig` 中属于以上模块的全部移植到 `crates/zant/tests/` | ~4,000 | 像素断言用 raster renderer + `pixel_at`，照搬 |
| 2.11 ★★ | `examples/bench.zig` → `zant-bench`；跑 §4 性能闸门，结果写进 `status.md` | 301 | |

M2 验收：`zant-components`、`zant-gallery`、declarative 示例在浅 / 深色、三种窗口尺寸下对照一致；
`scripts/win/components-check.ps1` 改指向 Rust 二进制后通过；性能闸门通过。

### M3 内容模块（纯逻辑部分 M0 之后即可开工，与 M1 / M2 并行）

| WP | Zig 源 | 行数 | 何时可开始 | 额外验收 |
|---|---|---|---|---|
| 3.1 ★ | `markdown/remend/` | 1,500 | M0 后 | `remend_cases.json` 522 条全过 |
| 3.2 ★ | `markdown/parser/`（block、inline …）、`markdown/{blocks, cache, html, safety, highlight}.zig` | ~3,500 | M0 后 | 对应测试（`src/markdown` 34 个 + `tests/markdown_test` 13 个）全过 |
| 3.3 ★★ | `markdown/render/`、`markdown/{editor, edit}.zig`、`components/widgets.zig` 的 markdown 部分 | ~2,000 | M2 后 | `zant-markdown` 对照一致 |
| 3.4 ★★ | `markdown/mermaid.zig` | 834 | 1.3 后（需要 `vector`） | 对照一致 |
| 3.5 ★ | `math/{parser, symbols, fonts}.zig` | ~1,800 | M0 后 | 现有测试全过 |
| 3.6 ★★ | `math/layout.zig` | 1,208 | 1.3 后 | `zant-math` 对照一致 |
| 3.7 ★ | `math/mhchem.zig`（KaTeX mhchem 的逐字节对齐移植，**必须保持逐字节对齐**） | 2,565 | M0 后 | `mhchem_cases.json` 823 条全过 |
| 3.8 ★★ | `pdf/{document, text, renderer, annotations, native}.zig`、`platform/pdf.zig` | ~1,200 | 1.1 后 | 用 `~/Downloads/pdf` 的真实文件手测（跳过简历，文件不提交） |
| 3.9 ★★ | `pdf/viewer.zig`、`ui.pdfPreview` | 1,666 | M2 后 | `zant-pdf` 对照一致；`scripts/win/pdf-check.ps1` 通过 |
| 3.10 ★★★ | `db/sqlite.zig` 的 comptime ORM → `rusqlite` + `#[derive(zant_macros::Table)]`（从结构体字段生成建表 SQL、绑定、读取）。**生成的 SQL 文本要与 Zig 版相同**，已有数据库文件才能直接打开 | 852 | M0 后 | 现有 db 测试全过；用 Zig 版建的库 Rust 版能读 |
| 3.11 ★★ | `examples/{markdown, math, markdown_stream, pdf}.zig` → `zant-examples` | ~660 | 3.3 / 3.6 / 3.9 后 | 流式性能：ReleaseFast 等价构建的 `zant-md-stream` 配 `ZANT_FRAME_STATS=1`，长回复 + 多条历史，单帧远低于 16.7 ms，`[slow-frame]` 的 text_layouts 不随内容增长 |

### M4 TSX 编译器前端（M0 之后即可开工，与 M1–M3 并行）

全部 ★，按文件 1:1 直译到 `crates/zant-tsx/src/`，AST 形状不变，测试同名移植。

| WP | Zig 源（`tools/tsx/`） | 行数 |
|---|---|---|
| 4.1 | `lexer.zig`、`ast.zig`、`jsx_text.zig`、`jsx_entities.zig` | ~1,000 |
| 4.2 | `parser.zig`、`expr_parser.zig`、`jsx_parser.zig`、`type_parser.zig`、`loop_parser.zig`、`import_parser.zig`、`react_compat.zig` | ~1,550 |
| 4.3 | `hooks/{expand, rewrite, resolve, leftover}.zig`（自定义 hook 的 AST 展开） | 1,326 |
| 4.4 | `tags.zig`、`tag_aliases.zig`、`string_method_names.zig`、诊断信息相关代码 | ~400 |

验收：前端单元测试全过；对 32 个 `testdata/*.tsx` 和 `examples/tsx/**/*.tsx`，把 AST 按固定格式 dump 出来，
Zig 版与 Rust 版逐字节相同（WP-4.1 负责给 Zig 版加一个只读的 `--dump-ast` 调试开关，这属于允许的 Zig 改动，记入变更日志）。
`scripts/tsx/swc-corpus.sh` 的 981 个用例不崩溃。

### M5 TSX 后端与运行时（依赖 M2 + M4；5.1 是 ★★★）

**WP-5.1 ★★★ 代码生成设计**（负责人），产出 `docs/rust-port/tsx-codegen.md`，后续 5.x 照做。已定的要点：

- 类型映射：`string` → `zant::Str`；`T[]` → `tsx::List<T>`（`Rc<Vec<T>>` 包装，写时复制）；`interface` → `#[derive(Clone, PartialEq)] struct`；
  联合字面量 → `enum`；`T | null` → `Option<T>`；数值类型 `u8`…`i64`、`usize`、`f32`、`f64` 原样。
- **所有权策略：TSX 的值要么 `Copy`，要么廉价 `Clone`；codegen 每次读取非 `Copy` 变量一律生成 `.clone()`，不做「最后一次使用」优化。**
  生成的文件头加 `#![allow(clippy::redundant_clone)]`。这条规则让 codegen 不需要做借用分析。
- `types.Value { ty, code, precedence }` 的结构保留，`code` 改成 Rust 源码，`zig_text.Precedence` 换成 Rust 运算符优先级表。
- 输出校验与格式化：`syn::parse_file` + `prettyplease`（对应现在的 `std.zig.Ast` 重解析）；解析失败即 panic 并打印生成的源码。
- 因为 Rust 有闭包和所有权，这些 Zig 运行时机制**不移植**：`tsx.State(T)`（改普通字段）、`tsx.capture`（改 `move` 闭包）、
  `tsx.deinitFields`（改 `Drop`）、`deepCopy`（改 `Clone`）、`tsx.concat`（改 `nodes!`）。
- 目标形态（对应 `testdata/counter.tsx`）：
  ```rust
  pub struct Counter { count: u32, name: Str, notify: bool }
  impl Default for Counter { fn default() -> Self { Self { count: 0, name: "Zant".into(), notify: false } } }
  impl Counter {
      pub fn render(&mut self) -> ui::Node {
          ui::column(ui::BoxOptions { children: nodes![
              ui::input(ui::InputOptions { key: Some("name".into()), value: self.name.clone(),
                  on_change: Some(ui::text_action(|s: &mut Counter, v: &str| s._set_name(v))), ..Default::default() }),
              ui::button(ui::ButtonOptions { key: Some("reset".into()), text: "Reset".into(), disabled: self.count == 0,
                  on_click: Some(ui::action(|s: &mut Counter| s._on_click1())), ..Default::default() }),
              if self.notify { Some(ui::text("Notifications are on", ui::TextOptions { key: Some("note".into()), ..Default::default() })) } else { None },
          ], ..Default::default() })
      }
      fn increment(&mut self) { let count: u32 = self.count; /* … */ self.count = count + step; }
      fn _on_click1(&mut self) { self.count = 0; }
  }
  ```
- 子组件：`pub fn build(props: Props, id: u64) -> ui::Node`，局部状态 `ui::local_state::<Local>(id, Local::default)`，回调捕获这个 `Rc`。
  调用点的 id 仍用 `tsx::child_id(parent, "Counter", key)` / `tsx::site_id(parent, "Counter", n)`。

| WP | 内容（对照 `tools/tsx/codegen/` 同名文件） | 行数 |
|---|---|---|
| 5.2 ★★ | `zant-macros` 的 `#[derive(TsxProps)]` + `zant-tsx-tags` 工具，生成 `crates/zant-tsx/src/ui_tags.rs`（替代 comptime 反射的 `manifest/`）；`dts.zig` → 生成 `zant.d.ts`；CI 检查两者没过期 | ~700 |
| 5.3 ★★ | codegen 核心：`types`、`scope`、`expr`、`stmt`、`zig_text`(→`rust_text`)、`builtins`、`narrowing`、`functions`、`loops`、`aggregate` | ~2,600 |
| 5.4 ★★ | JSX：`jsx`、`jsx_values`、`jsx_children`、`rich_text`、`list`、`events` | ~1,700 |
| 5.5 ★★ | 组件与状态：`codegen`、`components`、`model`、`stateful`、`hooks`、`stores`、`array_methods`、`local_state` | ~2,800 |
| 5.6 ★★ | 应用层：`imports`、`zig_imports`(→`rust_imports`，用 `syn` 读 `.rs` 的 `pub fn` 签名)、`windows`、`desktop_hooks`、`zant_api`、`assets`、`entry`、`compile`、`main` | ~1,800 |
| 5.7 ★★ | `src/tsx*.zig`（13 个文件，2,775 行）→ `zant/src/tsx/`，只移植 5.1 判定仍需要的部分（`Memo`、`Effect`、`Deps`、`Callback`、`Store`、`Window`、`timers`、`builtins` 的 JS 数值语义、`platform`） | ~2,000 |
| 5.8 ★★★ | **native 模块**：`native/<名字>.ts` 规范编译成 Rust trait（`pub trait ClipsSpec { fn list() -> List<Clip>; .. }`），应用在 `native/<名字>.rs` 里 `impl ClipsSpec for Native`，签名不符由 rustc 报错；`native/lib.rs` 注册。替代 `src/tsx_native.zig` + `codegen/native_specs.zig` | ~260 |
| 5.9 ★★ | 黄金用例：32 个 `testdata/*.tsx` 的期望输出换成 `.rs`（由新编译器生成后**人工逐个审阅**再签入）；`tests/tsx_all.zig`、`tests/tsx/` 移植：生成结果要能编译并在运行期驱动 | — |
| 5.10 ★ | `examples/tsx/**` 的入口包装改 Rust，`*_manual.zig` 对照组改手写 Rust，`bench-tsx` 移植 | ~800 |

M5 验收：`zant-tsx-counter / -gallery / -todos / -reader / -app / -notes` 与 Zig 版对照一致；
`scripts/mac/ui-scenario.swift` + `tsx-counter.steps` 通过；`bench-tsx` 中 TSX 生成代码不慢于手写 Rust 对照组 1.2 倍。

### M6 CLI、构建、脚手架、发布（依赖 M5；6.1–6.4 可并行）

| WP | 内容 |
|---|---|
| 6.1 ★★ | `tools/zant/*.zig` → `zant-cli`。`run [--watch] [--release]`、`compile`、`package`、`build [配置]`、`create`、`sync`、`update` 命令和输出文字不变；`zig build` 换成 `cargo build`，产物路径 `zig-out/bin/app` 换成 `target/{debug,release}/<名字>`；`zigExe()` 的查找逻辑删除，改为检查 `cargo` 是否可用；`--watch` 继续监视 `src/` 并重启（扩展到 `.tsx` / `.rs`）；`ZANT_RAW_LOGS` 保留 |
| 6.2 ★★ | `zant-build`：应用 `build.rs` 调 `zant_build::tsx_app()`，在 `OUT_DIR` 里生成 Rust 并由 `main.rs` `include!`；`rerun-if-changed` 覆盖 `src/**/*.tsx`、`native/**`、`app.config.json`。图标用 `include_bytes!`；Windows 上用 `embed-resource` 嵌清单（`zant.manifest`）和 `.ico`，非 debug 构建加 `windows_subsystem = "windows"`，并把 `pdfium.dll` 拷到产物旁 |
| 6.3 ★ | `scaffold/template-ts/`：`build.zig` / `build.zig.zon` 换成 `Cargo.toml` + `build.rs` + `rust-toolchain.toml`，`native/{lib, system}.zig` 换成 `.rs`；模板里的 `AGENTS.md` / `CLAUDE.md` / `README.md` 改写。`scaffold/template/`（纯 Zig 模板）删除，`zant create` 只保留 TS 模板。`scripts/tsx/create-smoke.sh`、`scripts/win/scaffold-check.ps1` 改到通过 |
| 6.4 ★ | `tools/zant/package/{macos.sh, windows.ps1}`：只改产物路径；Windows 不再打包 `zant_skia.dll`，保留 `pdfium.dll` 和 MSVC 运行库 |
| 6.5 ★★ | 发布：`scripts/release/package.{sh,ps1}`、`install.{sh,ps1}`、`.github/workflows/release.yml`。安装脚本改为确保 rustup 与固定工具链（Windows 另查 VS Build Tools，macOS 仍查 Xcode CLT），不再下载 Zig。发布包 = `bin/zant` + `zant-ui/`（crates、原生源码、Skia 预编译库与头文件）。应用的 `Cargo.toml` 用 `zant = { path = "<ZANT_HOME>/zant-ui/crates/zant" }` |
| 6.6 ★★ | 可选优化：CI 预编译原生部分为 `libzant_native.a` / `zant_native.lib` 放进发布包，`zant-sys/build.rs` 发现它就跳过 C++ 编译（对应现在 `third_party/zant-skia-prebuilt` 的做法） |

M6 验收：干净机器上 `install.sh` / `install.ps1` → `zant create demo --yes` → `zant run` → `zant build`，两平台得到可运行的安装包。

### M7 下游应用与文档（依赖 M6）

| WP | 内容 |
|---|---|
| 7.1 ★★ | `../cliphop-ts`：`native/` 下约 3,400 行 Zig 引擎（sync、transfer、storage、history）移植成 Rust，TSX 不动。完成 `docs/tsx-roadmap.md` 的 E4（与 Zig 版 ClipHop 的像素 / 点击对照）。之后 `../cliphop`（纯 Zig 版）归档 |
| 7.2 ★★ | `../stillmd`（Zig Markdown 编辑器）移植到内部 Rust API；`scripts/win/smoke-stillmd.ps1`、`docs-check.ps1` 通过 |
| 7.3 ★ | 文档重写：`README.md`、`AGENTS.md`、`CLAUDE.md`、`docs/{development, scaffold, tsx, tsx-native, desktop, database, state, markdown}.md`、`docs/components/*.md`（36 个，示例改成 TSX）。基本不动的：`DESIGN.md`、`docs/{motion, flex, architecture, window-config, build, showcase, tsx-roadmap}.md`（只替换其中的 Zig 片段） |

### M8 切换（负责人做，需用户确认后执行）

1. 全量验证（§7）两平台通过，结果写进 `status.md`。
2. 把原生源码移到 `crates/zant-sys/native/`，更新 `build.rs` 路径。
3. 删除：全部 `.zig`、`build.zig`、`build.zig.zon`、`tools/`、`scripts/win/{build_bridge.ps1, gen_def.sh}`、`src/renderer/skia/zant_skia.def`、`src/platform/windows/winsqlite3.def`、`skia_bridge_win.cpp` 里旧的资源加载分支和 `ZANT_STATIC_DATA` 宏守卫。
4. 更新 `THIRD_PARTY_NOTICES.md`（新增的 crate）、CI 去掉 Zig。
5. 合并到 `main`，打 `v*` 标签发布。**删除和发布前必须得到用户明确同意。**

迁移完成后回到「像 React 一样写」的目标，继续 `docs/tsx-roadmap.md` 的未完成项：A10（TS `enum`）、A12（泛型函数）、编译期自动 memo。

### 依赖与并行度

```
M0 ──┬─ M1(1.1 并行 → 1.2 → 1.3 → 1.4 → 1.5) ── M2(11 包并行) ──┬─ M5 ── M6 ── M7 ── M8
     ├─ M3 纯逻辑包(3.1 3.2 3.5 3.7 3.10) ─ 其余 3.x 等 1.3 / M2 ─┘
     └─ M4(4.1 → 4.2 → 4.3，4.4 并行) ─────────────────────────────┘
```
M0 完成后可同时开工的最多约 10 个包（1.1a–d、3.1、3.2、3.5、3.7、3.10、4.1）。

---

## 6. 对照验证工具（WP-0.4）

Zig 版是行为基准。两边使用同一套 Skia 桥，所以布局应完全相同、像素应几乎相同。

- `scripts/parity/capture.sh <zig|rust> <二进制名> <输出目录> [light|dark] [宽x高]`：
  设 `ZANT_NO_MOTION=1`、`ZANT_SNAPSHOT=<目录>/shot.png`、`ZANT_SNAPSHOT_EXIT=1`、`ZANT_LAYOUT_FILE=<目录>/layout.txt`、`ZANT_THEME` / `ZANT_DARK` 后运行。
  Zig 二进制取自 `zig-out-baseline/bin/`，Rust 取自 `target/release/`。
- `scripts/parity/compare.sh <二进制名>`：对浅 / 深色 × 三种窗口尺寸各抓一次并比较：
  - `layout.txt`（每行 `key x y width height`，见 `src/ui_runtime.zig:1390` 的 `writeLayoutFile`）：按 key 对齐，数值差 ≤ 0.01 才算一致，key 集合必须相同。
  - `shot.png`：通道差 > 2 的像素占比 ≤ 0.1% 算一致；不一致时输出差异图。
- 交互对照复用现有脚本：`scripts/mac/ui-scenario.swift`、`scripts/win/{ui-auto, components-check, pdf-check, resize-check, stream-resize-check}.ps1`，把可执行文件路径改成参数。
- Windows 的窗口必须在已登录桌面里运行（`scripts/win/run-desktop.ps1`），SSH 会话里看不到。

---

## 7. 最终验证（M8 之前逐项执行并记录）

1. `cargo test --workspace`，macOS 与 Windows 全过；测试数量不少于 M0 记录的 Zig 基线（remend 522 条、mhchem 823 条包含在内）。
2. `scripts/parity/compare.sh` 对全部示例通过：`zant-demo`、`zant-gallery`、`zant-components`、`zant-markdown`、`zant-math`、`zant-pdf`、六个 `zant-tsx-*`。
3. `zant-bench` 各场景（open / stream / typing / hover / resize / scroll / caret，另跑 `ZANT_BENCH_LIST=1`）不超过 Zig 基线 1.25 倍；`zant-md-stream` 流式帧率达标。
4. Windows 桌面脚本全过：`components-check`、`smoke-stillmd`、`docs-check`、`pdf-check`、`resize-check`、`stream-resize-check`、`scaffold-check`。
5. 端到端：全新安装 → `zant create` → `zant run --watch`（改 `.tsx` 后自动重启）→ `zant build` 出 DMG 和 Inno 安装包。
6. ClipHop（TS 版）和 stillmd 在两平台正常运行；ClipHop 能打开迁移前的 SQLite 数据。
7. 按 `DESIGN.md` 第 6 条人工过一遍：不同窗口尺寸、浅 / 深色主题、悬停 / 按下 / 禁用 / 焦点状态、动效过渡。

---

## 8. 风险与对策

| 风险 | 对策 |
|---|---|
| §4 契约在 M2 才发现不够用 | M1 由最强模型做并用 `zant-demo` 验证；M2 发现问题只登记，由负责人统一改契约 |
| 去掉 arena 后帧耗时上升 | M2 末的性能闸门（WP-2.11）；不达标再针对性优化 |
| TSX codegen 的所有权问题让执行者卡住 | 「非 Copy 一律 clone」规则 + 5.1 设计文档 + 黄金用例人工审阅 |
| `RefCell` 重入 panic（构建上下文、局部状态） | §4 的两条硬规则；runtime 测试里覆盖「回调里再触发 rebuild」「回调里读写局部状态」 |
| Windows 静态链接的 CRT / 资源问题 | WP-0.3 就在 Windows 上打通冒烟测试，不留到后面 |
| 并行期间 Zig 版继续变动导致漂移 | Zig 冻结功能开发，只修 bug 并记变更日志 |
| 执行者「顺手改进」造成行为差异 | §3 第 1 条 + 对照脚本是每个界面 WP 的硬验收 |
| 应用开发者的首次编译变慢、Windows 要装 VS Build Tools | WP-6.6 预编译原生部分；安装脚本明确提示 |

调查时未能核实、需要在 M0 确认的事项：`zig build test` 的真实通过数；macOS 上 `ZANT_SNAPSHOT_EXIT` 是否在 `window.m` 里处理；macOS 的 ICU 数据来源（推测已编进 `libskunicode_icu.a`）；哪些代码需要 `-lz`。
