//! `setTimeout` / `clearTimeout` and the interval / animation-frame
//! helpers for generated TSX code.
//!
//! Generated code owns closures, so a timer stores a [`ui::Action`] (the same
//! callback type every other event uses). Timers run on the UI thread: a
//! short-lived sleeper thread only posts `ui::post_wake` at the due time, the
//! app's `on_wake` hook fires what is due (see [`crate::tsx::app::Shell`]).
//!
//! ```ignore
//! let id = tsx::timers::set_timeout(8000.0, ui::action(|s: &mut App| s._expire()));
//! tsx::timers::clear_timeout(id);
//! ```
//!
//! The registry is thread-local (one UI thread per process) and ids grow
//! monotonically per thread; `clear_*` ignores unknown ids. A timer set by a
//! callback waits for a later `fire` call, and a wake may come up to 4 ms
//! early and still run its timer. An animation frame is a 0 ms timer, i.e. it
//! runs on the next UI-thread wake (there is no separate paint callback).

use std::cell::{Cell, RefCell};
use std::time::Duration;

use crate::ui::{Action, ActionCx};

thread_local! {
    /// Pending timers of the UI thread, in the order they were set.
    static TIMERS: RefCell<Vec<Timer>> = const { RefCell::new(Vec::new()) };
    /// Next timer id (ids are f64 in generated code: TS numbers).
    static NEXT_ID: Cell<u64> = const { Cell::new(1) };
    /// Test clock override (monotonic milliseconds).
    static CLOCK: Cell<Option<fn() -> i64>> = const { Cell::new(None) };
    /// Whether sleepers should post wakes (only inside `tsx::app::run`).
    static RUNNING: Cell<bool> = const { Cell::new(false) };
}

/// How early a wake may come and still run its timer.
const EARLY_MS: i64 = 4;

struct Timer {
    id: u64,
    due_ms: i64,
    /// `Some(ms)` for `set_interval`: re-arm after each run.
    repeat_ms: Option<i64>,
    callback: Action,
}

/// The current time source (monotonic milliseconds).
pub fn now() -> i64 {
    match CLOCK.with(Cell::get) {
        Some(clock) => clock(),
        None => crate::platform::platform::monotonic_ms(),
    }
}

/// Replaces the clock (tests: a fake clock, no sleeps).
pub fn set_clock(clock: Option<fn() -> i64>) {
    CLOCK.with(|current| current.set(clock));
}

/// Enables the sleeper threads that wake the UI thread ([`crate::tsx::app::run`]
/// turns them on; tests fire timers themselves).
pub fn set_running(running: bool) {
    RUNNING.with(|current| current.set(running));
}

pub fn is_running() -> bool {
    RUNNING.with(Cell::get)
}

fn schedule_wake(delay_ms: i64) {
    if !is_running() {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis((delay_ms.max(0) + 2) as u64));
        crate::ui::post_wake();
    });
}

fn clamp_delay(ms: f64) -> i64 {
    if ms > 0.0 { ms.min(1e12) as i64 } else { 0 }
}

/// Runs `callback` on the UI thread after `ms` milliseconds; returns the id
/// for [`clear_timeout`].
pub fn set_timeout(ms: f64, callback: Action) -> f64 {
    let delay = clamp_delay(ms);
    let id = NEXT_ID.with(|next| {
        let id = next.get();
        next.set(id + 1);
        id
    });
    TIMERS.with(|timers| {
        timers.borrow_mut().push(Timer {
            id,
            due_ms: now() + delay,
            repeat_ms: None,
            callback,
        });
    });
    schedule_wake(delay);
    id as f64
}

/// Short name of [`set_timeout`] (`tsx::timers::set` in generated code).
pub fn set(ms: f64, callback: Action) -> f64 {
    set_timeout(ms, callback)
}

/// Runs `callback` every `ms` milliseconds until [`clear_interval`].
pub fn set_interval(ms: f64, callback: Action) -> f64 {
    let delay = clamp_delay(ms);
    let id = NEXT_ID.with(|next| {
        let id = next.get();
        next.set(id + 1);
        id
    });
    TIMERS.with(|timers| {
        timers.borrow_mut().push(Timer {
            id,
            due_ms: now() + delay,
            repeat_ms: Some(delay),
            callback,
        });
    });
    schedule_wake(delay);
    id as f64
}

/// Runs `callback` on the next UI-thread wake (the next frame); returns the id
/// for [`cancel_animation_frame`].
pub fn request_animation_frame(callback: Action) -> f64 {
    set_timeout(0.0, callback)
}

/// Cancels a timer that has not run yet (unknown ids are ignored).
pub fn clear_timeout(id: f64) {
    if id.is_nan() || id < 1.0 {
        return;
    }
    let want = id as u64;
    TIMERS.with(|timers| {
        let mut timers = timers.borrow_mut();
        if let Some(index) = timers.iter().position(|timer| timer.id == want) {
            timers.remove(index);
        }
    });
}

/// Short name of [`clear_timeout`] (`tsx::timers::clear` in generated code).
pub fn clear(id: f64) {
    clear_timeout(id);
}

/// Cancels an interval (same ids as [`clear_timeout`]).
pub fn clear_interval(id: f64) {
    clear_timeout(id);
}

/// Cancels a `requestAnimationFrame` callback.
pub fn cancel_animation_frame(id: f64) {
    clear_timeout(id);
}

/// Runs the timers due by `now_ms` (in the order they were set); true when one
/// ran. Timers set by a callback wait for a later call.
pub fn fire(now_ms: i64, action_cx: &mut ActionCx<'_>) -> bool {
    let snapshot = TIMERS.with(|timers| timers.borrow().len());
    let mut ran = false;
    let mut index = 0;
    let mut seen = 0;
    while seen < snapshot {
        let due = TIMERS.with(|timers| {
            timers
                .borrow()
                .get(index)
                .is_some_and(|timer| timer.due_ms <= now_ms)
        });
        if due {
            let timer = TIMERS.with(|timers| {
                let mut timers = timers.borrow_mut();
                timers.remove(index)
            });
            (timer.callback).call(action_cx);
            if let Some(repeat) = timer.repeat_ms {
                TIMERS.with(|timers| {
                    timers.borrow_mut().push(Timer {
                        id: timer.id,
                        due_ms: now() + repeat,
                        repeat_ms: Some(repeat),
                        callback: timer.callback,
                    });
                });
            }
            ran = true;
            seen += 1;
            continue;
        }
        let len = TIMERS.with(|timers| timers.borrow().len());
        if index >= len {
            break;
        }
        index += 1;
        seen += 1;
    }
    ran
}

/// The app shell's wake: runs what is due, then makes sure a wake comes for
/// the next timer (a sleep may end a little early, e.g. on Windows).
pub fn fire_due(action_cx: &mut ActionCx<'_>) -> bool {
    let now = now();
    // a wake a few ms early still runs the timer it was for
    let ran = fire(now + EARLY_MS, action_cx);
    // every timer has its own sleeper (`set`); a wake that came just before a
    // timer was due gets another one for it
    let next = TIMERS.with(|timers| timers.borrow().iter().map(|timer| timer.due_ms).min());
    if let Some(due) = next
        && due - now < 100
    {
        schedule_wake((due - now).max(1));
    }
    ran
}

/// Timers still waiting (tests).
pub fn pending() -> usize {
    TIMERS.with(|timers| timers.borrow().len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell as StdCell, RefCell};
    use std::sync::atomic::{AtomicI64, Ordering};

    static CLOCK_NOW: AtomicI64 = AtomicI64::new(0);

    fn fake_now() -> i64 {
        CLOCK_NOW.load(Ordering::SeqCst)
    }

    fn setup() {
        // timers run on the calling thread, no sleepers
        set_clock(Some(fake_now));
        set_running(false);
        TIMERS.with(|timers| timers.borrow_mut().clear());
        NEXT_ID.with(|next| next.set(1));
        CLOCK_NOW.store(0, Ordering::SeqCst);
    }

    #[test]
    fn timers_run_when_due_in_order_and_can_be_cleared() {
        setup();
        let log = std::rc::Rc::new(RefCell::new(Vec::<char>::new()));
        let mut root = ();
        let mut cx = ActionCx::new(&mut root);
        let push = |tag: char| {
            let log = std::rc::Rc::clone(&log);
            Action::new(move |_| log.borrow_mut().push(tag))
        };
        let a = set_timeout(0.0, push('a'));
        let b = set_timeout(50.0, push('b'));
        let c = set_timeout(0.0, push('c'));
        assert!(a > 0.0 && b > a && c > b);
        clear_timeout(c);
        assert!(fire(now(), &mut cx));
        assert_eq!(*log.borrow(), vec!['a']);
        assert_eq!(pending(), 1);
        assert!(!fire(now(), &mut cx));
        assert!(fire(now() + 100, &mut cx));
        assert_eq!(*log.borrow(), vec!['a', 'b']);
        clear_timeout(b); // already ran: ignored
        clear_timeout(-1.0);
        assert_eq!(pending(), 0);
    }

    #[test]
    fn timers_set_by_a_callback_wait_for_a_later_call() {
        setup();
        let hits = std::rc::Rc::new(StdCell::new(0u32));
        let mut root = ();
        let mut cx = ActionCx::new(&mut root);
        let again = std::rc::Rc::clone(&hits);
        let first = std::rc::Rc::clone(&hits);
        let id = set_timeout(
            0.0,
            Action::new(move |_| {
                first.set(first.get() + 1);
                if first.get() == 1 {
                    let again = std::rc::Rc::clone(&again);
                    set_timeout(0.0, Action::new(move |_| again.set(again.get() + 1)));
                }
            }),
        );
        assert!(id > 0.0);
        assert!(fire(now(), &mut cx));
        assert_eq!(hits.get(), 1, "the nested timer did not run yet");
        assert!(fire(now(), &mut cx));
        assert_eq!(hits.get(), 2);
        assert_eq!(pending(), 0);
    }

    #[test]
    fn intervals_rearm_until_cleared() {
        setup();
        let hits = std::rc::Rc::new(StdCell::new(0u32));
        let mut root = ();
        let mut cx = ActionCx::new(&mut root);
        let counter = std::rc::Rc::clone(&hits);
        let id = set_interval(10.0, Action::new(move |_| counter.set(counter.get() + 1)));
        assert!(fire(now() + 10, &mut cx), "first run at the 10 ms deadline");
        assert_eq!(hits.get(), 1);
        assert_eq!(pending(), 1, "re-armed");
        clear_interval(id);
        assert!(!fire(now() + 1_000, &mut cx));
        assert_eq!(pending(), 0);
        assert_eq!(hits.get(), 1);
    }

    #[test]
    fn animation_frames_run_on_the_next_wake() {
        setup();
        let hits = std::rc::Rc::new(StdCell::new(0u32));
        let mut root = ();
        let mut cx = ActionCx::new(&mut root);
        let counter = std::rc::Rc::clone(&hits);
        let id = request_animation_frame(Action::new(move |_| counter.set(counter.get() + 1)));
        assert!(!fire(now() - 1, &mut cx), "not before the next wake");
        assert!(fire(now(), &mut cx));
        assert_eq!(hits.get(), 1);
        assert_eq!(pending(), 0);
        let cancelled = request_animation_frame(Action::new(|_| panic!("cancelled")));
        cancel_animation_frame(cancelled);
        assert!(!fire(now(), &mut cx));
        assert!(id > 0.0);
    }

    #[test]
    fn fire_due_reports_whether_anything_ran() {
        setup();
        let hits = std::rc::Rc::new(StdCell::new(0u32));
        let mut root = ();
        let mut cx = ActionCx::new(&mut root);
        let counter = std::rc::Rc::clone(&hits);
        assert!(!fire_due(&mut cx));
        let _ = set_timeout(
            1_000.0,
            Action::new(move |_| counter.set(counter.get() + 1)),
        );
        assert!(!fire_due(&mut cx), "not due yet");
        CLOCK_NOW.store(1_000, Ordering::SeqCst);
        assert!(fire_due(&mut cx));
        assert_eq!(hits.get(), 1);
        assert_eq!(pending(), 0);
    }

    #[test]
    fn delays_are_clamped() {
        setup();
        let id = set_timeout(-5.0, Action::new(|_| {}));
        assert!(id > 0.0);
        let mut root = ();
        assert!(fire(now(), &mut ActionCx::new(&mut root)));
        assert_eq!(pending(), 0);
        let long = set_timeout(1e12, Action::new(|_| {}));
        assert_eq!(pending(), 1);
        clear_timeout(long);
        assert_eq!(pending(), 0);
    }
}
