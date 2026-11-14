//! A native module with a background thread: `start` runs work on a thread
//! that posts a wake; `take` returns the result on the UI thread.

use std::sync::atomic::{AtomicU32, Ordering};

static DONE: AtomicU32 = AtomicU32::new(0);
static RESULT: AtomicU32 = AtomicU32::new(0);

pub fn start(n: u32) {
    let _ = std::thread::Builder::new().spawn(move || work(n));
}

fn work(n: u32) {
    RESULT.store(n * 2, Ordering::Release);
    DONE.store(1, Ordering::Release);
    zant::ui::post_wake();
}

/// The result once the thread finished, else 0.
pub fn take() -> u32 {
    if DONE.swap(0, Ordering::AcqRel) == 0 {
        return 0;
    }
    RESULT.load(Ordering::Acquire)
}
