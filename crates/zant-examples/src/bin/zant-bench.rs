//! `zant-bench`: headless frame benchmark. The benchmark lives in the
//! bin-local [`bench_app`] module so this file stays a thin entry point.
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
