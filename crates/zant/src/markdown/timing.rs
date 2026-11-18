//! Time spent in the Markdown pipeline of streaming widgets
//! (`ZANT_FRAME_STATS`): remend (healing the incomplete tail), splitting the
//! source into blocks and parsing the changed blocks, summed per frame and
//! printed with the `[slow-frame]` line.

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Instant;

use crate::renderer::text::LAYOUT_STATS;

/// One pipeline step.
#[derive(Clone, Copy)]
pub(crate) enum Step {
    Remend,
    Split,
    Parse,
}

static STEP_US: [AtomicI64; 3] = [AtomicI64::new(0), AtomicI64::new(0), AtomicI64::new(0)];

/// Starts timing a step; `None` while frame statistics are off.
pub(crate) fn start() -> Option<Instant> {
    LAYOUT_STATS.enabled().then(Instant::now)
}

/// Adds the time since `start` to `step`.
pub(crate) fn finish(step: Step, start: Option<Instant>) {
    if let Some(start) = start {
        let us = i64::try_from(start.elapsed().as_micros()).unwrap_or(i64::MAX);
        STEP_US[step as usize].fetch_add(us, Ordering::Relaxed);
    }
}

/// Returns the microseconds of (remend, split, parse) since the last call and
/// resets them.
pub fn take() -> [i64; 3] {
    [0, 1, 2].map(|i| STEP_US[i].swap(0, Ordering::Relaxed))
}
