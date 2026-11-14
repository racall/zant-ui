//! `counter.tsx` vs `counter_manual.zig` (ported): 3000 clicks cycling every
//! control of the page, then 300 typed characters. Zig `bench.zig`'s `clicks`
//! and `typing` scenarios.
#![forbid(unsafe_code)]

use zant::ui::App;

use super::controls::counter::Counter as ManualCounter;
use super::{Harness, Stats, counter_case};

/// Zig `counter_keys`: the controls in the order the scenario clicks them.
const COUNTER_KEYS: [&str; 9] = [
    "inc", "inc", "step", "inc", "large", "notify", "reset", "inc", "step",
];

/// The TSX root component (`src/App.tsx` of `examples/tsx/counter.tsx`).
type TsxCounter = counter_case::app::Counter;

/// Zig `clickCounter` decided whether `reset` was clickable from the state
/// (`count == 0`). The TSX root state is private in Rust, so the benchmark
/// tracks the same tiny state machine (count / step) on both sides; clicks are
/// only dispatched where the Zig loop dispatched them.
#[derive(Clone, Copy)]
struct Shadow {
    count: u32,
    step: u32,
}

impl Shadow {
    fn new() -> Shadow {
        Shadow { count: 0, step: 1 }
    }

    fn skipped(&self, key: &str) -> bool {
        key == "reset" && self.count == 0
    }

    fn apply(&mut self, key: &str) {
        match key {
            "inc" => self.count += self.step,
            "step" => self.step = if self.step == 1 { 5 } else { 1 },
            "reset" => self.count = 0,
            _ => {}
        }
    }
}

/// Zig `clickCounter`: 3000 timed click frames.
fn clicks(app: impl App) -> Stats {
    let mut harness = Harness::new(app);
    let mut stats = Stats::new();
    let mut shadow = Shadow::new();
    let mut index = 0usize;
    while stats.frames() < 3000 {
        let key = COUNTER_KEYS[index % COUNTER_KEYS.len()];
        index += 1;
        if shadow.skipped(key) {
            continue;
        }
        stats.add(harness.click(key).expect("counter controls are on screen"));
        shadow.apply(key);
    }
    stats
}

/// Zig `typing`: click the input, then 300 characters.
fn typing(app: impl App) -> Stats {
    let mut harness = Harness::new(app);
    harness.click("name").expect("the name input is on screen");
    let mut stats = Stats::new();
    for _ in 0..300 {
        stats.add(harness.type_text("a"));
    }
    stats
}

/// The TSX side: the generated root component of `counter.tsx`.
pub fn tsx() -> Vec<Stats> {
    vec![clicks(TsxCounter::default()), typing(TsxCounter::default())]
}

/// The hand-written control: `counter_manual.zig` ported.
pub fn manual() -> Vec<Stats> {
    vec![clicks(ManualCounter::new()), typing(ManualCounter::new())]
}
