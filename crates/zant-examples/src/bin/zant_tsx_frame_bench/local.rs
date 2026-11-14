//! `local_bench.tsx` vs `local_manual.zig` (ported): 300 rows mounted one click
//! at a time, then 300 clicks on the row counters (each a `useState` /
//! `ui::local_state`) — Zig `bench_local.zig`.
#![forbid(unsafe_code)]

use zant::ui::App;

use super::controls::local::LocalBench as ManualLocal;
use super::{Harness, Stats, local_case};

const ROWS: usize = 300;

/// The TSX root component (`src/App.tsx` of `examples/tsx/local_bench.tsx`).
type TsxLocal = local_case::app::LocalBench;

/// Zig `scenario`: mount `rows` counters, then click the visible ones.
fn run(app: impl App) -> Vec<Stats> {
    let mut harness = Harness::new(app);

    let mut mount = Stats::new();
    for _ in 0..ROWS {
        mount.add(harness.click("add").expect("the add button is on screen"));
    }
    // Zig asserted the mount really created one local-state slot per row (and
    // none extra): 300 `<Counter>` instances. The slot count is public on the
    // runtime's local-state store.
    let slots = harness.rt.local_states.borrow().count();
    assert_eq!(
        slots, ROWS,
        "the local-state page must hold one slot per counter"
    );

    let mut click = Stats::new();
    for index in 0..300 {
        let key = format!("inc-{}", index % 12 + 1);
        click.add(harness.click(&key).expect("the visible rows are on screen"));
    }
    vec![mount, click]
}

/// The TSX side: the generated root component of `local_bench.tsx`.
pub fn tsx() -> Vec<Stats> {
    run(TsxLocal::default())
}

/// The hand-written control: `local_manual.zig` ported.
pub fn manual() -> Vec<Stats> {
    run(ManualLocal::new())
}
