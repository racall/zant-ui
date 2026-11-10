//! 原生模块清单：每个 `pub const` 对应一个 `native/<名字>.zig`，
//! TS 侧的接口写在同名的 `native/<名字>.ts`（规范，唯一来源）。
//! TSX 里 `import { info } from "native/system"` 直接调用 Zig，没有桥接开销。
pub const system = @import("system.zig");

test {
    _ = system;
}
