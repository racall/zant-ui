# TS 应用的原生模块（native/）

TS + TSX 应用的 `src/` 只放 TS/TSX；剪贴板引擎、数据库、网络这类底层代码写成 **Rust 原生模块**，
放在 `native/`。写法参考 React Native 新架构的 TurboModule：先用 TS 写一份**规范**（spec），
它是接口的唯一来源；Rust 按规范实现，**编译期由 rustc 检查两边是否一致**。没有桥接层，也没有序列化，
TSX 里的调用编译成对 Rust 函数的直接调用。

```
my-app/
  src/                 只有 TS/TSX
    App.tsx            import { copy, entries } from "native/clips";
  native/
    lib.rs             模块清单：pub mod clips; pub use clips::Native; + register()
    clips.ts           规范：类型 + export declare function
    clips.rs           实现：impl clips::Spec for Native
    engine/…           内部代码（子目录不会被当成规范）
  Cargo.toml  build.rs  rust-toolchain.toml  tsconfig.json
```

`zant create` 生成的项目自带一个示例模块 `native/system`（平台、CPU 核数、Rust 版本）。

## 规范：`native/<名字>.ts`

```ts
import type { i64 } from "zant";

export type Kind = "text" | "image" | "files";
export interface ListEntry {
  row: i64;
  kind: Kind;
  preview: string;
  from: string | null;
}
export type CopyResult = "ok" | "not_found" | "missing_content";

export declare function entries(query: string): ListEntry[];
export declare function copy(row: i64): CopyResult;
```

- 只能有类型声明（`interface` / `type`）和 `export declare function`，不能有函数体或常量。
- Rust 端要用到的对象类型和字符串联合都要**命名**，不能写成内联类型；
  `T | null`、`T[]` 可以直接写。写成内联类型时，编译器会在这一行报错并提示怎么改。
- 参数和返回值是数据，不能是回调；耗时工作在 Rust 里放到后台线程，完成后用 `ui::post_wake()`
  唤醒界面，TSX 在 `useWake` 里读取结果。
- 规范之间可以共享类型：`import type { Kind } from "./clips";`（只能导入类型）。

类型映射与 TSX 相同：

| 规范 | Rust |
|---|---|
| `string` | `zant::Str`（返回 `&'static str` / `String` 也可以，生成代码会转成 `Str`） |
| `number` | `f64` |
| `boolean` | `bool` |
| `i64` / `u32` 等（从 `"zant"` 导入） | 同名整数 |
| `T[]` | `Vec<T>`（TSX 侧是 `tsx::List<T>`） |
| `T \| null` | `Option<T>` |
| `"a" \| "b"` | `enum { A, B }`（实现 `as_str` / `from_str`） |
| `interface` | `#[derive(Clone, PartialEq, Default)] pub struct` |
| `void` | `()` |

## 实现：`native/<名字>.rs`

生成器为每个规范输出 Rust 类型和一个 trait（名字取规范文件名，PascalCase + `Spec`）：

```rust
// native/clips.rs（应用自己写）
use crate::native::clips::{CopyResult, ListEntry, Spec};

pub struct Native;

impl Spec for Native {
    fn entries(query: &str) -> Vec<ListEntry> { … }
    fn copy(row: i64) -> CopyResult { … }
}
```

再在 `native/lib.rs` 里登记：

```rust
pub mod clips;
pub use clips::Native;

pub fn register() {
    zant::tsx::native::register(clips::Native);
}
```

原生模块可以导入：

- `zant`：框架（`zant::ui`、`zant::db`、`zant::desktop`、`zant::image`、`zant::pdf` …）；
- 生成的规范模块（`crate::native::clips`）：类型与 trait，和 TSX 用的是同一份 `.ts`；
- `crate::app_config`：由 `app.config.json` 生成的配置（`title`、`bundle_id`、`window`）。

返回的字符串和集合是 Rust 拥有的值（`String` / `Str` / `Vec`），可以跨帧保存，不需要帧分配器。

## 编译期检查

Rust 的类型系统就是检查器：规范生成的 trait 决定了方法签名，`impl` 里缺函数、函数不是 `pub`、
签名不一致都会在**编译**时报错，错误直接指向 `native/clips.rs` 的 `impl`：

```
error[E0046]: not all trait items implemented, missing: `copy`
  --> native/clips.rs:…
   = help: implement `fn copy(row: i64) -> CopyResult` (declared in native/clips.ts)
```

实现里可以有规范之外的 `pub fn`，它们只给 Rust 自己用，TSX 看不到。

## TSX 里调用

```tsx
import { entries, copy, type ListEntry } from "native/clips";

const list: ListEntry[] = entries(query);
<Button onClick={() => copy(entry.row)}>复制</Button>
```

导入了规范里没有的名字时，会在 `.tsx` 的这一行报错。编辑器通过 `tsconfig.json` 的
`"native/*": ["./native/*"]` 找到规范，补全和类型检查都按规范进行。

## 构建与测试

应用的 `build.rs` 调 `zant_build::tsx_app()`：它编译 `src/**/*.tsx`，扫描 `native/` 下的 `.ts`
规范生成 Rust 类型与 trait（写到 `$OUT_DIR`），并把 `native/` 当成应用的模块树（`src/main.rs` 里
`#[path = "../native/lib.rs"] mod native;`）。`rerun-if-changed` 覆盖 `native/**`，改规范或实现都会重编。

原生模块就是普通 Rust 代码，直接用 `cargo test` 测试（模板里 `native/system.rs` 带示例测试）。
`zant run --watch` 也会监视 `native/` 下的 `.ts` 与 `.rs` 文件。

多个原生模块在一个应用里互不影响：每个 `.ts` 规范一个模块、一个 trait、一个 `Native` 实现；
`native/lib.rs` 的 `register()` 只是给按名字解析用的运行时注册表（生成的静态调用路径不经过它），
`src/main.rs` 在第一帧前调用一次。

## 和直接导入 `.rs` 的区别

TSX 仍然可以 `import { f } from "./util.rs"`（见 [tsx.md](tsx.md)「从 Rust 导入函数」），
那种方式以 Rust 的签名为准，Rust 文件和 TSX 文件放在一起。新项目建议用原生模块：
接口以 TS 写的规范为准，`src/` 里只有 TS，Rust 代码集中放在 `native/`。
