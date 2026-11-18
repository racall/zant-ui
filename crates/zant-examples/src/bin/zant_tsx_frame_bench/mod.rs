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
//! Each of the `ZANT_BENCH_REPEAT` rounds (default 5) compares median frame
//! times, alternating which side runs first. The median of those ratios is
//! gated at `ZANT_BENCH_MAX_RATIO` (default 1.2). App rows pool frames within
//! each round. Independent manual/manual pairs measure noise for each row.
//! For sensitivity testing, `ZANT_BENCH_SLOW_TSX=1.3` busy-waits after each
//! TSX frame until its elapsed time reaches 1.3 times the measured duration.
//! `scripts/tsx/bench-tsx.sh` formats the comparison table.
#![forbid(unsafe_code)]

use std::cell::{Cell, RefMut};
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

#[allow(clippy::all, dead_code)]
mod local_soak_case {
    include!(concat!(
        env!("OUT_DIR"),
        "/tsx-frame-bench/cases/local_soak/main.rs"
    ));
}

mod controls;

mod counter;
mod list;
mod local;
mod markdown;
mod settings;
mod soak;
mod todos;

mod stats;
mod suites;
#[cfg(test)]
mod tests;

use self::stats::Row;
pub use self::stats::Stats;
pub use self::suites::*;

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

// Each suite constructs its harnesses inside the side being measured.
thread_local! {
    static FRAME_TIME_FACTOR: Cell<f64> = const { Cell::new(1.0) };
}

/// One whole frame in milliseconds (`renderer.begin` → `render_frame` →
/// `renderer.end`).
pub fn frame(rt: &mut Runtime, renderer: &mut Renderer) -> f64 {
    advance_clock(16);
    let t0 = platform::monotonic_us();
    let canvas = renderer.begin(1.0).expect("a raster canvas");
    rt.render_frame(canvas, W, H, 1.0);
    renderer.end();
    let mut elapsed = platform::monotonic_us() - t0;
    let factor = FRAME_TIME_FACTOR.get();
    if factor > 1.0 {
        let target = elapsed as f64 * factor;
        while (elapsed as f64) < target {
            std::hint::spin_loop();
            elapsed = platform::monotonic_us() - t0;
        }
    }
    elapsed as f64 / 1000.0
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

/// `main`: parse the scenario arguments, set motion off, run the suites.
pub fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    zant::motion::motion::set_enabled(false);
    if soak::requested(&args) {
        soak::run(&args);
        return;
    }
    let rounds = repeats();
    let gate = max_ratio();
    println!(
        "zant-tsx-frame-bench: raster canvas {W}x{H}, median of {rounds} paired round(s), gate <= {gate:.2}x manual"
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

    let slow_tsx = std::env::var("ZANT_BENCH_SLOW_TSX")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|factor| factor.is_finite() && *factor > 1.0)
        .unwrap_or(1.0);
    let mut failures = 0;
    let mut scenarios = 0;
    for suite in &selected {
        scenarios += suite.scenarios.len();
        for row in measure(suite, rounds, slow_tsx) {
            let ratio = row.report();
            if ratio > gate {
                failures += 1;
                println!(
                    "over-budget {} {} ratio={ratio:.2}x > {gate:.2}x; profile with ZANT_FRAME_STATS=1",
                    row.kind, row.name
                );
            }
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
