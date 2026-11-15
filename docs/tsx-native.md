# TS 应用的原生模块（native/）

推荐用 TSX 编写应用。TS + TSX 应用的 `src/` 只放 TS/TSX；剪贴板引擎、数据库、网络这类底层代码写成 **Rust 原生模块**，
放在 `native/`。写法参考 React Native 新架构的 TurboModule：先用 TS 写一份**规范**（spec），
它是接口的唯一来源；Rust 按规范实现，**编译期由 rustc 检查两边是否一致**。没有桥接层，也没有序列化，
TSX 里的调用编译成对 Rust 函数的直接调用。

```
my-app/
  src/                 只有 TS/TSX
    App.tsx            import { copy, entries } from "native/clips";
  native/
    main.rs            一行 zant::app!();
    build.rs           一行 fn main() { zant_build::tsx_app(); }
    clips.ts           规范：类型 + export declare function
    clips.rs           实现：impl ClipsSpec for Native
    engine/…           内部代码（子目录不会被当成规范）
  Cargo.toml  rust-toolchain.toml  tsconfig.json
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
| `string` | `zant::Str`（参数和返回值均相同；Rust 字符串需显式转换） |
| `number` | `f64` |
| `boolean` | `bool` |
| `i64` / `u32` 等（从 `"zant"` 导入） | 同名整数 |
| `T[]` | `zant::tsx::List<T>`（用 `List::from_vec(vec)` 包装 Rust 集合） |
| `T \| null` | `Option<T>` |
| `"a" \| "b"` | `enum { A, B }`（实现 `as_str` / `from_str`） |
| `interface` | `#[derive(Clone, PartialEq, Default)] pub struct` |
| `void` | `()` |

## 实现：`native/<名字>.rs`

生成器为规范输出 Rust 类型；声明了函数的规范还会输出一个 trait（名字取规范文件名，PascalCase + `Spec`）：

```rust
// native/clips.rs（应用自己写）
use crate::native::spec::clips::{CopyResult, ListEntry, ClipsSpec};

pub struct Native;

impl zant::tsx::native::NativeModule for Native {
    const NAME: &'static str = "clips";
}

// 最小实现；实际应用在这里调用自己的数据层。
impl ClipsSpec for Native {
    fn entries(_query: zant::Str) -> zant::tsx::List<ListEntry> {
        zant::tsx::List::new()
    }
    fn copy(_row: i64) -> CopyResult {
        CopyResult::NotFound
    }
}
```

模块声明与 `register()` 都由构建器生成，无需维护清单。顶层 `.rs` 自动声明；
有同名 `.ts` 规范时还会通过 `zant::tsx::native::register` 注册 `Native`，
因此实现还要提供上例的 `NativeModule::NAME`（规范文件名）。无规范的内部辅助模块不注册，子目录通过普通 `mod` 引入。
`main.rs` 和 `build.rs` 不参与模块扫描。

原生模块可以导入：

- `zant`：框架（`zant::ui`、`zant::db`、`zant::desktop`、`zant::image`、`zant::pdf` …）；
- 生成的规范模块（`crate::native::spec::clips`）：类型与 trait，和 TSX 用的是同一份 `.ts`；
- `crate::app_config`：由 `app.config.json` 生成的配置（`title`、`bundle_id`、`window`）。

接口中的字符串和集合使用 `Str` / `tsx::List`，可以在当前应用版本内跨帧保存，不需要帧分配器。
Rust 内部的 `String` 可用 `.into()` 转为 `Str`，`Vec<T>` 用 `List::from_vec` 转换。
仅共享类型的 `.ts` 规范不生成 trait，也不需要配套的 `.rs` 实现。

## 编译期检查

Rust 的类型系统就是检查器：规范生成的 trait 决定了方法签名，`impl` 里缺函数、
签名不一致都会在**编译**时报错（trait 方法的实现不写 `pub`），错误直接指向 `native/clips.rs` 的 `impl`：

```
error[E0046]: not all trait items implemented, missing: `copy`
  --> native/clips.rs:…
```

规范之外的辅助方法写在单独的 `impl Native { … }` 或模块函数中，只给 Rust 使用，TSX 看不到。

## TSX 里调用

```tsx
import { Column, Button } from "zant";
import { entries, copy, type ListEntry } from "native/clips";

export default function App() {
  const list: ListEntry[] = entries("");
  return (
    <Column>
      {list.map(entry => (
        <Button key={entry.row} onClick={() => { copy(entry.row); }}>{entry.preview}</Button>
      ))}
    </Column>
  );
}
```

导入了规范里没有的名字时，会在 `.tsx` 的这一行报错。编辑器通过 `tsconfig.json` 的
`"native/*": ["./native/*"]` 找到规范，补全和类型检查都按规范进行。

## 构建与测试

应用的 `native/build.rs` 调用 `zant_build::tsx_app()`，`native/main.rs` 只写 `zant::app!();`。
配置、规范类型、trait、模块声明与注册都生成在 `$OUT_DIR/zant_app.rs`，启动入口在 `zant_main.rs`；
热重载库用 `zant::hot_app!()` 引入 `zant_hot.rs`。`crate::native::spec::<名字>` 路径保持稳定。
`rerun-if-changed` 覆盖 `src/`、`native/`、配置和资源，改规范或实现都会重编。
原生模块用 `cargo test` 测试（本仓库沙箱中用 `./scripts/cargo.sh test`）。
工作区的 `rust-toolchain.toml` 固定 Rust 1.94.1；新应用模板使用 stable。

默认 `zant run` 使用宿主持有窗口，修改 `src/` 中的 `.tsx` / `.ts`、`native/` 中的 `.rs` / `.ts`
或 TSX 导入的资源会原地热刷新。框架通过 `crates/zant-dylib` 在进程内加载一次；应用动态库被替换，
旧应用库会卸载，不能依赖旧库的全局变量继续存活。
应用自行创建的后台线程、回调和外部资源不会由框架自动停止，必须自行管理其生命周期，避免卸载后仍执行旧库代码。
`useAppExit` 只在应用退出时执行，原地热刷新不会调用它；不能只靠退出钩子清理热刷新中的原生工作线程。
`Cargo.toml` / `Cargo.lock` / `app.config.json` / `native/build.rs` / `rust-toolchain.toml`
及 `assets/icons/` 的改动会重启 `zant run`；`docs/`、`README.md`、`tsconfig.json`、隐藏文件、
编辑器临时文件及未导入资源等改动会忽略。兼容的组件状态和按稳定 key 标识的交互状态尽量恢复；
全局 Store 和 Rust 外部资源不自动持久化。
`zant run --watch` 为旧的重编译重启模式，`zant run --no-hot` 只运行一次。详见 [脚手架](scaffold.md#热更新)。

`src/` 中出现任何 `.rs` 都会报构建错误；请把 Rust 移到 `native/` 并为 TSX 调用提供 `.ts` 规范。
底层编译器的独立 `.rs` 导入能力仍保留，应用构建器统一使用以上目录约定。
