//! The TSX compile benchmark (WP-5.10): compile every `tools/tsx/testdata/*.tsx`
//! through the in-process compiler, `warmup + iterations` times each, and report
//! the same statistics line as the Zig frame benchmark
//! (`examples/tsx/bench_harness.zig`).
//!
//! The shipped Zig binary (`zig build bench-tsx` → `zant-tsx-bench`) measures
//! *frames* (clicks, typing, …), not compile time, so the Zig side of this
//! comparison is `tools/tsx/compile_bench.zig` (the same `tools/tsx/compile.zig`
//! the Zig compiler uses, in process). `scripts/tsx/examples-check.sh` builds
//! both, runs both on the same corpus and gates the ratio at 1.25×.
//!
//!   cargo run --release -p zant-examples --bin zant-tsx-bench
//!   ./target/release/zant-tsx-bench <corpus dir> [warmup] [iterations]
//!
//! `frames=` is the number of timed compiles of that file (the bench row of the
//! Zig harness, whose unit is a frame); `total` aggregates the files that
//! compiled. Two corpus files are rejected by the Rust compiler today
//! (`desktop`/`entities`, status.md「待决问题」2/4); they are reported and
//! skipped, and the gate compares the files both sides compile.

use std::path::{Path, PathBuf};
use std::time::Instant;

/// The benchmark runs out of the checkout: the corpus lives next to the
/// workspace's `Cargo.toml` (`crates/zant-examples/../..`), overridable with
/// the first positional argument.
fn default_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/tsx/testdata")
}

/// Per-file compile samples in milliseconds; the report line of
/// `examples/tsx/bench_harness.zig` `Stats.report` (`{s: <20} frames=…`).
#[derive(Default)]
struct Stats {
    samples: Vec<f64>,
}

impl Stats {
    fn add(&mut self, ms: f64) {
        self.samples.push(ms);
    }

    fn report(&mut self, name: &str) {
        if self.samples.is_empty() {
            return;
        }
        self.samples.sort_by(f64::total_cmp);
        let len = self.samples.len();
        let sum: f64 = self.samples.iter().sum();
        let p95 = self.samples[p95_index(len)];
        let avg = sum / len as f64;
        println!(
            "{name:<20} frames={len:>5}  avg={avg:>6.3} ms  p95={p95:>6.3} ms  max={:>6.3} ms  ({:.0} fps at avg)",
            self.samples[len - 1],
            1000.0 / avg
        );
    }
}

/// The p95 sample index of `examples/tsx/bench_harness.zig`:
/// `(len * 95) / 100`, clamped to the last sample.
fn p95_index(len: usize) -> usize {
    (len * 95 / 100).min(len.saturating_sub(1))
}

/// Every `.tsx` file of `corpus`, sorted by name (the Zig harness sorts too).
fn corpus_files(corpus: &Path) -> Vec<PathBuf> {
    let entries = match std::fs::read_dir(corpus) {
        Ok(entries) => entries,
        Err(error) => {
            eprintln!("zant-tsx-bench: cannot read {}: {error}", corpus.display());
            std::process::exit(2);
        }
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file() && path.extension().is_some_and(|extension| extension == "tsx")
        })
        .collect();
    files.sort();
    files
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let corpus = args.first().map_or_else(default_corpus, PathBuf::from);
    let warmup: usize = args
        .get(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(10);
    let iterations: usize = args
        .get(2)
        .and_then(|value| value.parse().ok())
        .unwrap_or(100);

    let files = corpus_files(&corpus);
    if files.is_empty() {
        eprintln!("zant-tsx-bench: no .tsx files in {}", corpus.display());
        std::process::exit(2);
    }
    println!(
        "zant-tsx-bench: {} corpus files in {}, {} warm-up rounds, {} iterations",
        files.len(),
        corpus.display(),
        warmup,
        iterations
    );

    let mut total = Stats::default();
    let mut failed: Vec<String> = Vec::new();
    let runs = warmup + iterations;
    for path in &files {
        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("zant-tsx-bench: cannot read {}: {error}", path.display());
                std::process::exit(2);
            }
        };
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
        let name = file_name
            .strip_suffix(".tsx")
            .unwrap_or(&file_name)
            .to_owned();
        let mut stats = Stats::default();
        let mut rejected = false;
        for run in 0..runs {
            let started = Instant::now();
            let result = zant_tsx::compile::compile(&source, &name);
            let ms = started.elapsed().as_secs_f64() * 1000.0;
            match result {
                Ok(_) => {
                    if run >= warmup {
                        stats.add(ms);
                        total.add(ms);
                    }
                }
                Err(diag) => {
                    print!(
                        "{}",
                        zant_tsx::error::render_diag(&path.to_string_lossy(), &source, &diag)
                    );
                    failed.push(name.clone());
                    rejected = true;
                    break;
                }
            }
        }
        if !rejected {
            stats.report(&name);
        }
    }
    total.report("total");
    if !failed.is_empty() {
        println!("failed: {} ({})", failed.len(), failed.join(", "));
    }
    if total.samples.is_empty() {
        eprintln!("zant-tsx-bench: no corpus file compiled");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The corpus shipped in the repository: the plan's 32 `.tsx` files.
    #[test]
    fn corpus_has_the_32_testdata_files() {
        let files = corpus_files(&default_corpus());
        assert!(
            files.len() >= 32,
            "expected the 32 tools/tsx/testdata files, found {}",
            files.len()
        );
        let names: Vec<String> = files
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"counter.tsx".to_owned()));
        assert!(names.contains(&"todos.tsx".to_owned()));
    }

    /// `Stats::report` prints the Zig harness line (`{s: <20} frames=…`).
    #[test]
    fn report_matches_the_zig_stats_line() {
        let mut stats = Stats::default();
        for ms in [0.1, 0.2, 0.3, 0.4] {
            stats.add(ms);
        }
        assert_eq!(stats.samples.len(), 4);
        stats.samples.sort_by(f64::total_cmp);
        let avg = stats.samples.iter().sum::<f64>() / 4.0;
        assert!((avg - 0.25).abs() < 1e-9);
        assert_eq!(stats.samples[p95_index(4)], 0.4);
    }

    /// The p95 index of the Zig harness (`bench_harness.zig`):
    /// `(len * 95) / 100`, clamped to `len - 1`.
    #[test]
    fn p95_index_matches_the_zig_harness() {
        assert_eq!(p95_index(1), 0);
        assert_eq!(p95_index(4), 3);
        assert_eq!(p95_index(100), 95);
        assert_eq!(p95_index(3200), 3040);
    }
}
