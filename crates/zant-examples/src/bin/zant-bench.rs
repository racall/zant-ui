//! `zant-bench`: headless frame benchmark, port of the Zig `zant-bench`
//! (`examples/bench.zig`, WP-2.11). The benchmark lives in the bin-local
//! [`bench_app`] module so the executable keeps the Zig binary name while the
//! app / scenario code stays readable.
//!
//! ```text
//! cargo build --release -p zant-examples --bin zant-bench
//! ./target/release/zant-bench                 # all scenarios
//! ./target/release/zant-bench open scroll     # some of them
//! ZANT_BENCH_LIST=1 ./target/release/zant-bench
//! ```
#![forbid(unsafe_code)]

#[path = "zant_bench/bench_app.rs"]
mod bench_app;

fn main() {
    bench_app::main();
}
