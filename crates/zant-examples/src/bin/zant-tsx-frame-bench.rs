//! `zant-tsx-frame-bench`: the TSX frame benchmark (WP-5.10c) — plan.md §5 M5
//! acceptance, "`bench-tsx` 中 TSX 生成代码不慢于手写 Rust 对照组 1.2 倍".
//!
//! Port of the Zig `bench-tsx` (`examples/tsx/bench.zig`): every TSX benchmark
//! page (`examples/tsx/counter.tsx`, `todos.tsx`, `list_bench.tsx`,
//! `local_bench.tsx`, `markdown_bench.tsx`, `settings.tsx`) compiled by
//! `zant-build` in this crate's `build.rs`, timed against a hand-written
//! `ui::App` control with the same UI tree through the same headless frame
//! loop. The driver, harness and scenarios live in the bin-local
//! [`bench`] module; `scripts/tsx/bench-tsx.sh` builds the Release binary, runs
//! the repeats and prints the comparison table plus the ≤1.2× gate verdict.
//!
//! ```text
//! cargo build --release -p zant-examples --bin zant-tsx-frame-bench
//! ./target/release/zant-tsx-frame-bench                 # all six apps
//! ./target/release/zant-tsx-frame-bench counter todos   # some of them
//! ZANT_BENCH_REPEAT=5 ./target/release/zant-tsx-frame-bench
//! ZANT_FRAME_STATS=1 ./target/release/zant-tsx-frame-bench list   # frame phases
//! ```
#![forbid(unsafe_code)]

#[path = "zant_tsx_frame_bench/mod.rs"]
mod bench;

fn main() {
    bench::main();
}
