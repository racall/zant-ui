# TS 应用的原生模块（native/）

TS + TSX 应用的 `src/` 只放 TS/TSX；剪贴板引擎、数据库、网络这类底层代码写成 Zig 原生模块，
放在 `native/`。写法参考 React Native 新架构的 TurboModule：先用 TS 写一份**规范**（spec），
它是接口的唯一来源；Zig 按规范实现，编译时自动检查两边是否一致。没有桥接层，也没有序列化，
TSX 里的调用编译成对 Zig 函数的直接调用。

```
my-app/
  src/                 只有 TS/TSX
    App.tsx            import { copy, entries } from "native/clips";
  native/
    lib.zig            模块清单：pub const clips = @import("clips.zig");
    clips.ts           规范：类型 + export declare function
    clips.zig          实现
    engine/…           内部代码（子目录不会被当成规范）
  app.config.json  build.zig  build.zig.zon  tsconfig.json
```

`zant create --lang ts` 生成的项目自带一个示例模块 `native/system`（平台、CPU 核数、Zig 版本）。

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
- Zig 端要用到的对象类型和字符串联合都要**命名**，不能写成内联类型；
  `T | null`、`T[]` 可以直接写。写成内联类型时，编译器会在这一行报错并提示怎么改。
- 参数和返回值是数据，不能是回调；耗时工作在 Zig 里放到后台线程，完成后用 `ui.postWake()`
  唤醒界面，TSX 在 `useWake` 里读取结果。
- 规范之间可以共享类型：`import type { Kind } from "./clips";`（只能导入类型）。

类型映射与 TSX 相同：

| 规范 | Zig |
|---|---|
| `string` | `[]const u8` |
| `number` | `f64` |
| `boolean` | `bool` |
| `i64` / `u32` 等（从 `"zant"` 导入） | 同名整数 |
| `T[]` | `[]const T` |
| `T \| null` | `?T` |
| `"a" \| "b"` | `enum { a, b }` |
| `interface` | `struct` |
| `void` | `void` |

## 实现：`native/<名字>.zig`

```zig
const spec = @import("spec").clips; // 规范生成的类型

pub fn entries(query: []const u8) []const spec.ListEntry { … }
pub fn copy(row: i64) spec.CopyResult { … }
```

再在 `native/lib.zig` 里登记：

```zig
pub const clips = @import("clips.zig");

test {
    _ = clips;
}
```

原生模块可以导入：

- `zant`：框架；
- `spec`：所有规范的类型（`@import("spec").clips.ListEntry`），和 TSX 用的是同一份；
- `app_config`：由 `app.config.json` 生成的配置，例如 `title`、`app_id`。

返回的字符串和切片，内存要在下一帧前一直有效：可以放在每帧的 arena 里
（`zant.ui.frameAllocator()`），也可以用模块自己持有的内存。

## 编译期检查

生成的入口会检查每个规范，出错时的提示直接说明要改哪个文件：

```
native/lib.zig: add `pub const clips = @import("clips.zig");` (the implementation of native/clips.ts)
native/clips.zig: missing `pub fn copy(row: i64) CopyResult` (declared in native/clips.ts)
native/clips.zig: `copy` is `fn (i64) bool`, but native/clips.ts declares `pub fn copy(row: i64) CopyResult`
```

分别对应：`native/lib.zig` 里没有登记模块；缺少函数或函数不是 `pub`；签名不一致。

实现里可以有规范之外的 `pub fn`，它们只给 Zig 自己用，TSX 看不到。

## TSX 里调用

```tsx
import { entries, copy, type ListEntry } from "native/clips";

const list: ListEntry[] = entries(query);
<Button onClick={() => copy(entry.row)}>复制</Button>
```

导入了规范里没有的名字时，会在 `.tsx` 的这一行报错。编辑器通过 `tsconfig.json` 的
`"native/*": ["./native/*"]` 找到规范，补全和类型检查都按规范进行。

## 构建与测试

`addTsxApp` 发现 `native/lib.zig` 时自动启用原生模块（目录可以用 `TsxAppOptions.native` 修改）。
它会建立 `spec`、`native`、`app_config` 三个模块，并加到应用模块里。
`zant_build.tsxNative(app)` 返回 `native` 模块，可以用来给它加单元测试或链接系统库：

```zig
if (zant_build.tsxNative(app)) |native| {
    const tests = b.addTest(.{ .root_module = native });
    test_step.dependOn(&b.addRunArtifact(tests).step);
    if (target.result.os.tag == .windows) native.linkSystemLibrary("iphlpapi", .{});
}
```

模板的 `zig build test` 会运行原生模块的测试。`zant run --watch` 也会监视 `native/` 下的
`.ts` 和 `.zig` 文件。

## 和直接导入 `.zig` 的区别

TSX 仍然可以 `import { f } from "./util.zig"`（见 [tsx.md](tsx.md)「从 Zig 导入函数」），
那种方式以 Zig 的签名为准，Zig 文件和 TSX 文件放在一起。新项目建议用原生模块：
接口以 TS 写的规范为准，`src/` 里只有 TS，Zig 代码集中放在 `native/`。
