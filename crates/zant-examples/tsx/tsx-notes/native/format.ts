// native/format.rs 的规范：TSX 按这里的类型调用，编译时检查 Rust 实现是否一致。
// Port of `examples/tsx/notes/format.zig` (`plural`) as a native module spec
// (WP-5.10b): the Zig app imported a `.zig` module; the Rust compiler rejects
// Zig imports, so the same helper is a `native/format.ts` spec + `.rs` impl and
// the TSX imports it from `"native/format"`.
import type { usize } from "zant";

// `plural(1, "note")` -> "1 note", `plural(2, "note")` -> "2 notes"
export declare function plural(n: usize, word: string): string;
