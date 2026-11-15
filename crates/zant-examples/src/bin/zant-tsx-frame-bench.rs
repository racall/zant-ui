//! `zant-tsx-frame-bench`: the TSX frame benchmark. Generated TSX code must
//! not be slower than 1.2× the hand-written Rust control.
//!
//! Every TSX benchmark page (`tsx/corpus/counter.tsx`, `todos.tsx`,
//! `list_bench.tsx`, `local_bench.tsx`, `markdown_bench.tsx`, `settings.tsx`)
//! is compiled by `zant-build` in this crate's `build.rs` and timed against a
//! hand-written `ui::App` control with the same UI tree through the same
//! headless frame loop. The driver, harness and scenarios live in the bin-local
//! [`bench`] module; `scripts/tsx/bench-tsx.sh` builds the Release binary, runs
//! the repeats and prints the comparison table plus the ≤1.2× gate verdict.
//!
//! ```text
//! cargo build --release -p zant-examples --bin zant-tsx-frame-bench
//! ./target/release/zant-tsx-frame-bench                 # all six apps
//! ./target/release/zant-tsx-frame-bench counter todos   # some of them
//! ZANT_BENCH_REPEAT=5 ./target/release/zant-tsx-frame-bench
//! ZANT_BENCH_SOAK=1 ./target/release/zant-tsx-frame-bench # memory soak only
//! ./target/release/zant-tsx-frame-bench soak             # same soak mode
//! ZANT_FRAME_STATS=1 ./target/release/zant-tsx-frame-bench list   # frame phases
//! ```
#![forbid(unsafe_code)]

#[path = "zant_tsx_frame_bench/mod.rs"]
mod bench;

fn main() {
    bench::main();
}
