// native/format.rs 的规范：TSX 按这里的类型调用，编译时检查 Rust 实现是否一致。
// A native module spec: the helper (`plural`) is declared here and implemented
// in `native/format.rs`; the TSX imports it from `"native/format"`.
import type { usize } from "zant";

// `plural(1, "note")` -> "1 note", `plural(2, "note")` -> "2 notes"
export declare function plural(n: usize, word: string): string;
