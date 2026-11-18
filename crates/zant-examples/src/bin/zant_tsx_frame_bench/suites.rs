//! The benchmark suites and measuring them (repeats, ratio limits, paired rounds).

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

/// Number of paired rounds per suite (default 5).
pub(super) fn repeats() -> usize {
    std::env::var("ZANT_BENCH_REPEAT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(5)
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

/// Return the app's pooled frames followed by its individual scenarios.
fn with_app_row(suite: &Suite, scenarios: Vec<Stats>) -> impl Iterator<Item = Stats> {
    assert_eq!(scenarios.len(), suite.scenarios.len(), "{}", suite.app);
    let mut app = Stats::new();
    for stats in &scenarios {
        app.extend(stats);
    }
    std::iter::once(app).chain(scenarios)
}

fn pair(
    left: impl FnOnce() -> Vec<Stats>,
    right: fn() -> Vec<Stats>,
    round: usize,
) -> (Vec<Stats>, Vec<Stats>) {
    if round.is_multiple_of(2) {
        (left(), right())
    } else {
        let second_side = right();
        (left(), second_side)
    }
}

/// Run both comparisons with fresh harnesses, alternating order in each pair.
pub(super) fn measure(suite: &Suite, rounds: usize, slow_tsx: f64) -> Vec<Row> {
    let mut rows: Vec<_> = std::iter::once(Row::new("app", suite.app))
        .chain(
            suite
                .scenarios
                .iter()
                .map(|name| Row::new("scenario", name)),
        )
        .collect();
    let tsx = || {
        FRAME_TIME_FACTOR.set(slow_tsx);
        let result = (suite.tsx)();
        FRAME_TIME_FACTOR.set(1.0);
        result
    };
    for round in 0..rounds {
        let (generated, manual) = pair(tsx, suite.manual, round);
        let (a, b) = pair(suite.manual, suite.manual, round);
        let comparison = with_app_row(suite, generated).zip(with_app_row(suite, manual));
        let control = with_app_row(suite, a).zip(with_app_row(suite, b));
        for ((row, ab), aa) in rows.iter_mut().zip(comparison).zip(control) {
            row.comparison.rounds.push(ab);
            row.control.rounds.push(aa);
        }
    }
    rows
}
