//! `zant-tsx-frame-bench`: the TSX frame benchmark.
//!
//! It runs each TSX benchmark page — compiled to Rust by `zant-build` in this
//! crate's `build.rs` — against a hand-written `ui::App` control with the same
//! UI tree, through the same headless frame loop (raster canvas, fake clock),
//! and gates the TSX/manual frame-time ratio at 1.2×.
//!
//! ```text
//! cargo build --release -p zant-examples --bin zant-tsx-frame-bench
//! ./target/release/zant-tsx-frame-bench                 # all apps
//! ./target/release/zant-tsx-frame-bench counter list    # some of them
//! ZANT_BENCH_REPEAT=5 ./target/release/zant-tsx-frame-bench
//! ZANT_FRAME_STATS=1 ./target/release/zant-tsx-frame-bench counter
//! ```
//!
//! # How both sides run
//!
//! The two sides are ordinary `ui::App`s driven by the same [`Harness`]:
//!
//! - the **TSX side** is the real generated app: the case's `src/App.tsx` is
//!   the `tsx/corpus/*.tsx` input, compiled by `zant_build::tsx_app_in` in
//!   `build.rs`, and included here as `mod *_case`. For `list` / `markdown`
//!   the generated code is a component taking the data as a prop (see
//!   `build.rs`), because the codegen keeps root-state fields private; the
//!   JSX and widget calls are the ones of `list_bench.tsx` / `markdown_bench.tsx`.
//! - the **manual side** is a hand-written `ui::App` in `controls/` with the
//!   same UI tree, one control per page, so every reported scenario has a
//!   ratio.
//!
//! Both are measured per frame as `renderer.begin` → `Runtime::render_frame`
//! (rebuild + layout + paint) → `renderer.end`, exactly like `zant-bench`
//! (`src/bin/zant_bench/bench_app.rs`): 640×900, raster canvas, a fake clock
//! advanced 16 ms per frame, motion off, one untimed warm-up frame per
//! harness. Each side gets a fresh runtime (and canvas) per round.
//!
//! `ZANT_BENCH_REPEAT` (default 3) rounds are timed, alternating which side
//! runs first (load on a shared machine hits both sides evenly); the best
//! round per side — the lowest average frame time — is compared, like
//! `scripts/bench.sh`. The gate (`ZANT_BENCH_MAX_RATIO`, default 1.2)
//! applies to every scenario and every app aggregate. `scripts/tsx/bench-tsx.sh`
//! is the reproducible entry point; it formats the comparison table.
#![forbid(unsafe_code)]

use std::cell::RefMut;
use std::sync::atomic::{AtomicI64, Ordering};

use zant::platform::platform::{self, WindowOptions};
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant::ui::{App, RunOptions};

// The generated TSX cases. Each `build.rs`-generated `main.rs` is a list of
// `#[path] pub mod …;` declarations resolved relative to the generated file,
// so `include!` nests them under the case module. `clippy::all` is allowed
// for generated code (the codegen emits constructs clippy simplifies); it is
// scoped to the generated modules, not this crate.
#[allow(clippy::all, dead_code)]
mod counter_case {
    include!(concat!(
        env!("OUT_DIR"),
        "/tsx-frame-bench/cases/counter/main.rs"
    ));
}
#[allow(clippy::all, dead_code)]
mod todos_case {
    include!(concat!(
        env!("OUT_DIR"),
        "/tsx-frame-bench/cases/todos/main.rs"
    ));
}
#[allow(clippy::all, dead_code)]
mod list_case {
    include!(concat!(
        env!("OUT_DIR"),
        "/tsx-frame-bench/cases/list/main.rs"
    ));
}
#[allow(clippy::all, dead_code)]
mod local_case {
    include!(concat!(
        env!("OUT_DIR"),
        "/tsx-frame-bench/cases/local/main.rs"
    ));
}
#[allow(clippy::all, dead_code)]
mod markdown_case {
    include!(concat!(
        env!("OUT_DIR"),
        "/tsx-frame-bench/cases/markdown/main.rs"
    ));
}
#[allow(clippy::all, dead_code)]
mod settings_case {
    include!(concat!(
        env!("OUT_DIR"),
        "/tsx-frame-bench/cases/settings/main.rs"
    ));
}

mod controls;

mod counter;
mod list;
mod local;
mod markdown;
mod settings;
mod todos;

/// The benchmark window size.
pub const W: f32 = 640.0;
pub const H: f32 = 900.0;

/// The runtime clock injected into every scenario (motion is off; it drives
/// the caret and wake timers only).
static FAKE_NOW: AtomicI64 = AtomicI64::new(1_000_000);

fn fake_clock() -> i64 {
    FAKE_NOW.load(Ordering::Relaxed)
}

fn advance_clock(ms: i64) {
    FAKE_NOW.fetch_add(ms, Ordering::Relaxed);
}

/// One scenario's frame samples. `report` prints the average, p95 (index
/// `len * 95 / 100` of the sorted list) and max.
#[derive(Clone, Default)]
pub struct Stats {
    pub samples: Vec<f64>,
}

impl Stats {
    pub fn new() -> Stats {
        Stats::default()
    }

    pub fn add(&mut self, ms: f64) {
        self.samples.push(ms);
    }

    pub fn extend(&mut self, other: &Stats) {
        self.samples.extend_from_slice(&other.samples);
    }

    pub fn frames(&self) -> usize {
        self.samples.len()
    }

    pub fn avg(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    fn sorted(&self) -> Vec<f64> {
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        sorted
    }

    pub fn p95(&self) -> f64 {
        let sorted = self.sorted();
        match sorted.len() {
            0 => 0.0,
            len => sorted[usize::min(len - 1, (len * 95) / 100)],
        }
    }

    pub fn max(&self) -> f64 {
        self.sorted().last().copied().unwrap_or(0.0)
    }

    /// One machine-readable comparison line: the `zant-bench` fields (frames,
    /// avg / p95 / max per frame) for both sides plus the TSX/manual ratio.
    /// `scripts/tsx/bench-tsx.sh` formats the table from these.
    pub fn report(&self, kind: &str, name: &str, other: &Stats) {
        let tsx_avg = self.avg();
        let manual_avg = other.avg();
        let ratio = if manual_avg > 0.0 {
            tsx_avg / manual_avg
        } else {
            f64::INFINITY
        };
        println!(
            "{kind} {name} frames={} tsx_avg={tsx_avg:.4} manual_avg={manual_avg:.4} ratio={ratio:.4} tsx_p95={:.4} manual_p95={:.4} tsx_max={:.4} manual_max={:.4}",
            self.frames(),
            self.p95(),
            other.p95(),
            self.max(),
            other.max(),
        );
    }
}

/// One whole frame in milliseconds (`renderer.begin` → `render_frame` →
/// `renderer.end`).
pub fn frame(rt: &mut Runtime, renderer: &mut Renderer) -> f64 {
    advance_clock(16);
    let t0 = platform::monotonic_us();
    let canvas = renderer.begin(1.0).expect("a raster canvas");
    rt.render_frame(canvas, W, H, 1.0);
    renderer.end();
    (platform::monotonic_us() - t0) as f64 / 1000.0
}

/// A headless runtime on the fake clock with an untimed warm-up frame,
/// clicks by key and per-frame timing. The raster canvas lives here (one per
/// side per round).
pub struct Harness {
    pub rt: Runtime,
    renderer: Renderer,
}

impl Harness {
    /// Runtime over `app` at 640×900, fake clock, then one untimed warm-up
    /// frame.
    pub fn new(app: impl App) -> Harness {
        let mut rt = Runtime::init(
            app,
            RunOptions {
                title: String::from("TSX frame bench"),
                window: WindowOptions {
                    width: W,
                    height: H,
                    ..WindowOptions::default()
                },
                ..RunOptions::default()
            },
        );
        rt.clock = fake_clock;
        // `zant-bench` wires `ZANT_FRAME_STATS=1` here: `ui::run` (the windowed
        // entry) is not used headless, so the per-second `[frames]` /
        // `[slow-frame]` phase breakdown has to be enabled explicitly.
        if frame_stats_enabled() {
            rt.enable_frame_stats();
        }
        let mut harness = Harness {
            rt,
            renderer: Renderer::create_raster(W as u32, H as u32).expect("a raster renderer"),
        };
        let _ = harness.frame();
        harness
    }

    /// Times one frame.
    pub fn frame(&mut self) -> f64 {
        frame(&mut self.rt, &mut self.renderer)
    }

    /// The laid-out box of a keyed node, `Runtime::tree.bounds_of_key`.
    pub fn bounds(&self, key: &str) -> Option<zant::core::geometry::Rect> {
        self.rt.tree.bounds_of_key(key)
    }

    /// Clicks the control with `key` (move + down + up at its centre), then
    /// times the frame that shows the result. A missing key is an error (the
    /// scenario would silently measure nothing).
    pub fn click(&mut self, key: &str) -> Result<f64, String> {
        let rect = self
            .bounds(key)
            .ok_or_else(|| format!("missing key `{key}`"))?;
        let point = zant::core::events::PointerEvent {
            x: rect.x + rect.width * 0.5,
            y: rect.y + rect.height * 0.5,
            ..Default::default()
        };
        let _ = self
            .rt
            .dispatch(zant::core::events::Event::PointerMove(point));
        let _ = self
            .rt
            .dispatch(zant::core::events::Event::PointerDown(point));
        let _ = self
            .rt
            .dispatch(zant::core::events::Event::PointerUp(point));
        Ok(self.frame())
    }

    /// Types one character into the focused input, then times the frame.
    pub fn type_text(&mut self, text: &str) -> f64 {
        let _ = self
            .rt
            .dispatch(zant::core::events::Event::TextInput(String::from(text)));
        self.frame()
    }

    /// The benchmark app state (`markdown` needs to set its source between
    /// frames).
    pub fn app_mut<T: App + 'static>(&mut self) -> RefMut<'_, T> {
        RefMut::map(self.rt.app.borrow_mut(), |app| {
            let app: &mut dyn std::any::Any = &mut **app;
            app.downcast_mut::<T>()
                .expect("the benchmark runtime was built with this app type")
        })
    }

    /// `Runtime::invalidate` without a window: ask for a rebuild in the next
    /// timed frame.
    pub fn invalidate(&mut self) {
        self.rt.invalidate();
    }
}

/// `ZANT_FRAME_STATS` set to a value starting with `1`.
fn frame_stats_enabled() -> bool {
    std::env::var("ZANT_FRAME_STATS")
        .map(|value| value.starts_with('1'))
        .unwrap_or(false)
}

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
fn repeats() -> usize {
    std::env::var("ZANT_BENCH_REPEAT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(3)
}

/// `ZANT_BENCH_MAX_RATIO`, the TSX/manual ratio gate (default 1.2).
fn max_ratio() -> f64 {
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
fn want(args: &[String], app: &str, scenarios: &[&str]) -> bool {
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
fn measure(suite: &Suite, rounds: usize) -> (Vec<Stats>, Vec<Stats>) {
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
fn best_of(best: Option<Stats>, stats: Stats) -> Stats {
    match best {
        Some(best) if best.avg() <= stats.avg() => best,
        _ => stats,
    }
}

/// `main`: parse the scenario arguments, set motion off, run the suites.
pub fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    zant::motion::motion::set_enabled(false);
    let rounds = repeats();
    let gate = max_ratio();
    println!(
        "zant-tsx-frame-bench: raster canvas {W}x{H}, best of {rounds} round(s) per side, gate <= {gate:.2}x manual"
    );

    let selected: Vec<&Suite> = SUITES
        .iter()
        .filter(|suite| want(&args, suite.app, suite.scenarios))
        .collect();
    if selected.is_empty() {
        eprintln!(
            "zant-tsx-frame-bench: no scenario matches {:?} (apps: counter, todos, list, local, markdown, settings)",
            args
        );
        std::process::exit(2);
    }

    let mut rows: Vec<Row> = Vec::new();
    let mut scenarios = 0;
    for suite in &selected {
        let (tsx, manual) = measure(suite, rounds);
        let mut app_tsx = Stats::new();
        let mut app_manual = Stats::new();
        for (index, name) in suite.scenarios.iter().enumerate() {
            app_tsx.extend(&tsx[index]);
            app_manual.extend(&manual[index]);
            rows.push(Row {
                kind: "scenario",
                name,
                tsx: tsx[index].clone(),
                manual: manual[index].clone(),
            });
            scenarios += 1;
        }
        rows.insert(
            rows.len() - suite.scenarios.len(),
            Row {
                kind: "app",
                name: suite.app,
                tsx: app_tsx,
                manual: app_manual,
            },
        );
    }

    let mut failures = 0;
    for row in &rows {
        let ratio = row.ratio();
        let ok = ratio <= gate;
        if !ok {
            failures += 1;
        }
        row.tsx.report(row.kind, row.name, &row.manual);
        if !ok {
            println!(
                "over-budget {} {} ratio={ratio:.2}x > {gate:.2}x; profile with ZANT_FRAME_STATS=1",
                row.kind, row.name
            );
        }
    }

    if failures == 0 {
        println!(
            "gate: PASS — {} apps, {scenarios} scenarios, all within {gate:.2}x hand-written Rust",
            selected.len()
        );
    } else {
        println!(
            "gate: FAIL — {failures} row(s) over {gate:.2}x (app rows and scenario rows); rerun with ZANT_FRAME_STATS=1"
        );
        std::process::exit(1);
    }
}

/// One printed comparison row: an app aggregate or a scenario.
struct Row {
    kind: &'static str,
    name: &'static str,
    tsx: Stats,
    manual: Stats,
}

impl Row {
    fn ratio(&self) -> f64 {
        let manual = self.manual.avg();
        if manual > 0.0 {
            self.tsx.avg() / manual
        } else {
            f64::INFINITY
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p95_uses_the_clamped_95_percent_index() {
        let mut stats = Stats::new();
        for i in 1..=100 {
            stats.add(f64::from(i));
        }
        assert_eq!(stats.frames(), 100);
        assert_eq!(stats.avg(), 50.5);
        // 0-based index `len * 95 / 100` of the sorted samples: 96 of 100.
        assert_eq!(stats.p95(), 96.0);
        assert_eq!(stats.max(), 100.0);
    }

    #[test]
    fn best_round_per_side_is_kept() {
        let slow = || Stats { samples: vec![2.0] };
        let fast = || Stats { samples: vec![1.0] };
        // a faster round replaces the best, a slower one does not
        assert_eq!(best_of(Some(slow()), fast()).avg(), 1.0);
        assert_eq!(best_of(Some(fast()), slow()).avg(), 1.0);
        assert_eq!(best_of(None, Stats::default()).frames(), 0);
    }

    #[test]
    fn scenario_arguments_accept_apps_scenarios_and_short_names() {
        let counter = &SUITES[0];
        assert!(want(&[], counter.app, counter.scenarios));
        assert!(want(
            &[String::from("counter")],
            counter.app,
            counter.scenarios
        ));
        assert!(want(
            &[String::from("counter-typing")],
            counter.app,
            counter.scenarios
        ));
        assert!(want(
            &[String::from("typing")],
            counter.app,
            counter.scenarios
        ));
        let settings = &SUITES[5];
        assert!(want(
            &[String::from("settings")],
            settings.app,
            settings.scenarios
        ));
        assert!(!want(
            &[String::from("counter")],
            settings.app,
            settings.scenarios
        ));
    }

    #[test]
    fn the_generated_counter_app_is_driven_like_the_control() {
        // The TSX side is the generated root component (not a stub): its keys
        // are laid out and a click runs the generated action.
        let mut tsx = Harness::new(counter_case::app::Counter::default());
        assert!(
            tsx.bounds("inc").is_some(),
            "the generated page is laid out"
        );
        assert!(tsx.click("inc").is_ok(), "the generated action runs");
        // The hand-written control is laid out the same.
        let mut manual = Harness::new(controls::counter::Counter::new());
        assert!(manual.bounds("inc").is_some());
        assert!(manual.click("inc").is_ok());
    }

    #[test]
    fn the_generated_list_page_builds_the_ten_thousand_rows_lazily() {
        // The root `<List>` is the real generated one (`ListBench.tsx`: rows
        // built once by `useMemo`, then the virtual list): early rows exist,
        // the last row was never built (10 000 rows, one screen of nodes).
        let mut tsx = Harness::new(list_case::app::ListBench::default());
        assert!(tsx.bounds("picked").is_some());
        assert!(tsx.bounds("pick-1").is_some(), "the first row is on screen");
        assert!(
            tsx.bounds("pick-10000").is_none(),
            "the virtual list must not build all 10 000 rows"
        );
        assert!(tsx.click("pick-1").is_ok());
    }

    /// The keyed nodes of a laid-out window (sorted), the structural fingerprint
    /// of the UI tree.
    fn keyed_nodes(harness: &Harness) -> Vec<String> {
        let mut keys: Vec<String> = harness
            .rt
            .tree
            .nodes
            .iter()
            .filter(|node| node.alive)
            .filter_map(|node| node.key.as_ref().map(|key| key.as_str().to_owned()))
            .collect();
        keys.sort();
        keys
    }

    #[test]
    fn the_generated_tree_has_the_control_keys() {
        // The controls mirror the TSX page's tree; the keyed nodes of the first
        // frame must be the same set (settings: five sections, 40 rows +
        // switches; counter: its nine controls).
        let tsx = Harness::new(settings_case::app::Settings::default());
        let manual = Harness::new(controls::settings::Settings::new());
        let tsx_keys = keyed_nodes(&tsx);
        // changes + 5 sections + 40 rows + 40 switches
        assert!(tsx_keys.len() >= 86, "{tsx_keys:?}");
        assert_eq!(tsx_keys, keyed_nodes(&manual));

        let tsx = Harness::new(counter_case::app::Counter::default());
        let manual = Harness::new(controls::counter::Counter::new());
        let counter_keys = keyed_nodes(&tsx);
        for key in [
            "title", "count", "name", "inc", "reset", "step", "large", "notify", "note",
        ] {
            assert!(counter_keys.iter().any(|name| name == key), "missing {key}");
        }
        assert_eq!(counter_keys, keyed_nodes(&manual));
    }

    #[test]
    fn suites_cover_the_six_acceptance_apps() {
        let apps: Vec<&str> = SUITES.iter().map(|suite| suite.app).collect();
        for app in ["counter", "todos", "list", "local", "markdown", "settings"] {
            assert!(apps.contains(&app), "no suite for {app}");
        }
        // counter 2 + todos 3 + list 2 + local 2 + markdown 1 + settings 1
        assert_eq!(
            SUITES
                .iter()
                .map(|suite| suite.scenarios.len())
                .sum::<usize>(),
            11
        );
    }
}
