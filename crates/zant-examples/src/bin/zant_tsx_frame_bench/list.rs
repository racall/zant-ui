//! `list_bench.tsx` vs `list_manual.zig` (ported): a 10 000-row virtual list,
//! 300 wheel scrolls and then 100 row clicks — Zig `bench_list.zig`.
//!
//! The Zig benchmark filled the TSX root's `rows` state from the driver
//! (`app.rows.set(tsx_rows)`); Rust codegen keeps root state private, so the
//! generated page builds the same 10 000 `Row title` rows in its `useState`
//! initialiser (`tsx_views/ListBench.tsx`, `list_bench.tsx` plus a `makeRows()`
//! function) and this driver just runs the root app. Both sides get the
//! identical 10 000 rows.
#![forbid(unsafe_code)]

use zant::Str;
use zant::core::events::{Event, Modifiers, Scroll};
use zant::ui::App;

use super::controls::list::{ListBench as ManualList, Row as ManualRow};
use super::{Harness, Stats, list_case};

/// The TSX root component (`src/App.tsx` of `tsx_views/ListBench.tsx`, the
/// `list_bench.tsx` page; its `useState` initialiser builds the rows).
type TsxList = list_case::app::ListBench;

const ROWS: usize = 10_000;

/// Zig `scenario`: 300 scroll frames, then 100 clicks on the rows on screen.
fn run(app: impl App) -> Vec<Stats> {
    let mut harness = Harness::new(app);

    let mut scroll = Stats::new();
    for index in 0..300 {
        let dy: f32 = if index < 200 { -120.0 } else { 120.0 };
        let _ = harness.rt.dispatch(Event::Scroll(Scroll {
            x: 200.0,
            y: 400.0,
            dx: 0.0,
            dy,
            modifiers: Modifiers::default(),
        }));
        scroll.add(harness.frame());
    }

    let mut click = Stats::new();
    let mut clicked = 0usize;
    let mut id: usize = 1;
    while clicked < 100 {
        let key = format!("pick-{id}");
        if harness.bounds(&key).is_some() {
            click.add(harness.click(&key).expect("the row is on screen"));
            clicked += 1;
            if id > 15 {
                id = 0;
            }
        } else if id > ROWS {
            panic!("no row is on screen after scrolling");
        }
        id += 1;
    }
    vec![scroll, click]
}

/// The TSX side: the generated `<List>` page.
pub fn tsx() -> Vec<Stats> {
    run(TsxList::default())
}

/// The hand-written control: `list_manual.zig` ported.
pub fn manual() -> Vec<Stats> {
    let rows: Vec<ManualRow> = (0..ROWS)
        .map(|index| ManualRow {
            id: index as u32 + 1,
            title: Str::Static("Row title"),
        })
        .collect();
    run(ManualList::new(rows))
}
