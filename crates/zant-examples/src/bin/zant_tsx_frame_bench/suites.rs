//! The benchmark suites and measuring them (repeats, ratio limits, best-of rounds).

use super::*;

/// One benchmark app: one harness run per side produces one [`Stats`] per
/// scenario (an app's phases run through a single harness).
pub struct Suite {
    /// App row of the comparison table (`counter`, `todos`, …).
    pub app: &'static str,
    /// Scenario names in the order `tsx()` / `manual()` return the stats.
    pub scenarios: &'static [&'static str],
    /// The TSX side: the generated app(s) and the timed frames.
    pub tsx: fn() -> Vec<Stats>,
    /// The hand-written control side, same input and frame counts.
    pub manual: fn() -> Vec<Stats>,
}

/// The benchmark apps with their scenarios. `ZANT_BENCH_LIST` /
/// `ZANT_BENCH_MESSAGES` of `zant-bench` do not apply here.
pub const SUITES: &[Suite] = &[
    Suite {
        app: "counter",
        scenarios: &["counter-clicks", "counter-typing"],
        tsx: counter::tsx,
        manual: counter::manual,
    },
    Suite {
        app: "todos",
        scenarios: &["todos-add", "todos-toggle", "todos-filter"],
        tsx: todos::tsx,
        manual: todos::manual,
    },
    Suite {
        app: "list",
        scenarios: &["list-scroll", "list-click"],
        tsx: list::tsx,
        manual: list::manual,
    },
    Suite {
        app: "local",
        scenarios: &["local-mount", "local-click"],
        tsx: local::tsx,
        manual: local::manual,
    },
    Suite {
        app: "markdown",
        scenarios: &["markdown-stream"],
        tsx: markdown::tsx,
        manual: markdown::manual,
    },
    Suite {
        app: "settings",
        scenarios: &["settings-click"],
        tsx: settings::tsx,
        manual: settings::manual,
    },
];

/// `ZANT_BENCH_REPEAT` rounds per suite, best round wins (`scripts/bench.sh`
/// uses the same variable; default 3).
pub(super) fn repeats() -> usize {
    std::env::var("ZANT_BENCH_REPEAT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(3)
}

/// `ZANT_BENCH_MAX_RATIO`, the TSX/manual ratio gate (default 1.2).
pub(super) fn max_ratio() -> f64 {
    std::env::var("ZANT_BENCH_MAX_RATIO")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| *value > 0.0)
        .unwrap_or(1.2)
}

/// Whether to run `app`: no argument runs everything; an argument may name
/// the app (`counter`), one of its scenarios (`counter-typing`), or a short
/// scenario name (`clicks` / `typing`, `scroll` / `click`, `mount`,
/// `stream`).
pub(super) fn want(args: &[String], app: &str, scenarios: &[&str]) -> bool {
    if args.is_empty() {
        return true;
    }
    args.iter().any(|arg| {
        if arg == app || scenarios.iter().any(|name| name == arg) {
            return true;
        }
        match arg.as_str() {
            "clicks" | "typing" => app == "counter",
            "scroll" | "click" => app == "list",
            "mount" => app == "local",
            "stream" => app == "markdown",
            _ => false,
        }
    })
}

/// Runs one suite for `rounds` rounds, alternating which side goes first, and
/// keeps the best round per side and scenario (lowest average frame time).
pub(super) fn measure(suite: &Suite, rounds: usize) -> (Vec<Stats>, Vec<Stats>) {
    let mut best_tsx: Vec<Option<Stats>> = vec![None; suite.scenarios.len()];
    let mut best_manual: Vec<Option<Stats>> = vec![None; suite.scenarios.len()];
    for round in 0..rounds {
        let (tsx, manual) = if round % 2 == 0 {
            ((suite.tsx)(), (suite.manual)())
        } else {
            let manual = (suite.manual)();
            let tsx = (suite.tsx)();
            (tsx, manual)
        };
        assert_eq!(
            tsx.len(),
            suite.scenarios.len(),
            "{}: the TSX side returned {} stats for {} scenarios",
            suite.app,
            tsx.len(),
            suite.scenarios.len()
        );
        assert_eq!(
            manual.len(),
            suite.scenarios.len(),
            "{}: the manual side returned {} stats for {} scenarios",
            suite.app,
            manual.len(),
            suite.scenarios.len()
        );
        for (index, stats) in tsx.into_iter().enumerate() {
            best_tsx[index] = Some(best_of(best_tsx[index].take(), stats));
        }
        for (index, stats) in manual.into_iter().enumerate() {
            best_manual[index] = Some(best_of(best_manual[index].take(), stats));
        }
    }
    (
        best_tsx
            .into_iter()
            .map(Option::unwrap_or_default)
            .collect(),
        best_manual
            .into_iter()
            .map(Option::unwrap_or_default)
            .collect(),
    )
}

/// The lower-average of two rounds (`None` before the first round).
pub(super) fn best_of(best: Option<Stats>, stats: Stats) -> Stats {
    match best {
        Some(best) if best.avg() <= stats.avg() => best,
        _ => stats,
    }
}
