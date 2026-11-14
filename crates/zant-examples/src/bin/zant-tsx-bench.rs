//! The TSX compile benchmark: compile every corpus `.tsx`
//! (`crates/zant-tsx/testdata/tsx/`) through the in-process compiler,
//! `warmup + iterations` times each, and report the same statistics line as
//! the frame benchmarks. `scripts/tsx/examples-check.sh` runs it and checks
//! the corpus count.
//!
//!   cargo run --release -p zant-examples --bin zant-tsx-bench
//!   ./target/release/zant-tsx-bench <corpus dir> [warmup] [iterations]
//!
//! `frames=` is the number of timed compiles of that file (the column name is
//! shared with the frame benchmarks); `total` aggregates the files that
//! compiled. Corpus files the compiler rejects are reported and skipped.

use std::path::{Path, PathBuf};
use std::time::Instant;

/// The benchmark runs out of the checkout: the corpus lives next to the
/// workspace's `Cargo.toml` (`crates/zant-examples/../..`), overridable with
/// the first positional argument.
fn default_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../zant-tsx/testdata/tsx")
}

/// Per-file compile samples in milliseconds, reported as one
/// `<name> frames=… avg=… p95=… max=…` line.
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

/// The p95 sample index: `(len * 95) / 100`, clamped to the last sample.
fn p95_index(len: usize) -> usize {
    (len * 95 / 100).min(len.saturating_sub(1))
}

/// Every `.tsx` file of `corpus`, sorted by name.
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

    /// The corpus shipped in the repository: at least 32 `.tsx` files.
    #[test]
    fn corpus_has_the_32_testdata_files() {
        let files = corpus_files(&default_corpus());
        assert!(
            files.len() >= 32,
            "expected the 32 crates/zant-tsx/testdata/tsx files, found {}",
            files.len()
        );
        let names: Vec<String> = files
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"counter.tsx".to_owned()));
        assert!(names.contains(&"todos.tsx".to_owned()));
    }

    /// The statistics behind `Stats::report`: sample count, average and p95.
    #[test]
    fn report_statistics_use_the_sorted_samples() {
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

    /// The p95 index is `(len * 95) / 100`, clamped to `len - 1`.
    #[test]
    fn p95_index_is_95_percent_of_the_length_clamped() {
        assert_eq!(p95_index(1), 0);
        assert_eq!(p95_index(4), 3);
        assert_eq!(p95_index(100), 95);
        assert_eq!(p95_index(3200), 3040);
    }
}
