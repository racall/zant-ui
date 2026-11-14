//! `settings.tsx` vs the hand-written settings control: 2000 switch clicks on
//! the 40-row settings page.
//!
//! `controls/settings.rs` is the hand-written counterpart with the same tree
//! (`Settings · N changes`, five sections, 40 switches).
#![forbid(unsafe_code)]

use zant::ui::App;

use super::controls::settings::Settings as ManualSettings;
use super::{Harness, Stats, settings_case};

/// The TSX root component (`src/App.tsx` of `tsx/corpus/settings.tsx`).
type TsxSettings = settings_case::app::Settings;

const CLICKS: usize = 2000;

/// 2000 clicks cycling through the switches that fit in the
/// window (12 of the 40).
fn run(app: impl App) -> Vec<Stats> {
    let mut harness = Harness::new(app);
    let mut stats = Stats::new();
    for index in 0..CLICKS {
        let key = format!("sw-{}", index % 12);
        stats.add(harness.click(&key).expect("the switches are on screen"));
    }
    vec![stats]
}

/// The TSX side: the generated root component of `settings.tsx`.
pub fn tsx() -> Vec<Stats> {
    run(TsxSettings::default())
}

/// The hand-written control (`controls/settings.rs`).
pub fn manual() -> Vec<Stats> {
    run(ManualSettings::new())
}
