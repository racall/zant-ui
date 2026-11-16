//! The UI thread's async executor (on `async-task`) and microtask queue.
//!
//! Generated TSX runs async work here: `fetch`, promise chains, `async`
//! handlers. Everything app-facing stays on the UI thread; other threads
//! (the HTTP runtime, native workers) only wake tasks:
//!
//! - a task (`spawn`) is polled on the UI thread; a wake from any thread
//!   puts it on a thread-safe queue and posts a UI wake (`ui::post_wake`);
//! - a microtask (`queue`) is a closure given the app's [`ActionCx`]:
//!   promise reactions and state updates from async code, run in order
//!   (JavaScript's microtask queue). Futures never hold `&mut` app state.
//!
//! [`run`] drains both until nothing is left; the TSX shell calls it when
//! the UI thread wakes (`Shell::on_wake`), then every window rebuilds once.
//! [`release_app`] drops all of it before a hot reload unloads the app's
//! library (dropping an `async_task::Task` cancels its future).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Mutex};

use async_task::{Runnable, Task};

use crate::ui::ActionCx;

/// The tasks of one UI thread woken from any thread.
type Woken = Arc<Mutex<VecDeque<Runnable>>>;

type Microtask = Box<dyn FnOnce(&mut ActionCx<'_>)>;

thread_local! {
    static WOKEN: Woken = Arc::new(Mutex::new(VecDeque::new()));
    static MICROTASKS: RefCell<VecDeque<Microtask>> = const { RefCell::new(VecDeque::new()) };
    /// Running tasks (their handles; dropping one cancels it).
    static TASKS: RefCell<Vec<Task<()>>> = const { RefCell::new(Vec::new()) };
}

fn drain(woken: &Woken) -> Vec<Runnable> {
    woken
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .drain(..)
        .collect()
}

/// Runs `future` on this (the UI) thread, first polled at the next [`run`].
pub fn spawn(future: impl Future<Output = ()> + 'static) {
    // a wake from any thread queues the task for the thread that spawned it
    let woken = WOKEN.with(Arc::clone);
    let schedule = move |runnable: Runnable| {
        woken
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .push_back(runnable);
        crate::ui::post_wake();
    };
    let (runnable, task) = async_task::spawn_local(future, schedule);
    TASKS.with(|tasks| {
        let mut tasks = tasks.borrow_mut();
        tasks.retain(|task| !task.is_finished());
        tasks.push(task);
    });
    runnable.schedule();
}

/// Queues `work` to run with the app state at the next checkpoint, after
/// the work queued before it.
pub fn queue(work: impl FnOnce(&mut ActionCx<'_>) + 'static) {
    MICROTASKS.with(|queue| queue.borrow_mut().push_back(Box::new(work)));
    crate::ui::post_wake();
}

/// Runs the woken tasks and the microtasks until both queues are empty;
/// true when anything ran (the app's state may have changed).
pub fn run(cx: &mut ActionCx<'_>) -> bool {
    let mut ran = false;
    loop {
        let mut progressed = false;
        // tasks woken so far (each poll may wake more: next round)
        for runnable in WOKEN.with(drain) {
            runnable.run();
            progressed = true;
        }
        while let Some(work) = MICROTASKS.with(|queue| queue.borrow_mut().pop_front()) {
            work(cx);
            progressed = true;
        }
        super::promise::report_unhandled();
        if !progressed {
            return ran;
        }
        ran = true;
    }
}

/// Whether work is waiting (tests; the shell just calls [`run`]).
pub fn pending() -> bool {
    WOKEN.with(|woken| {
        !woken
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .is_empty()
    }) || MICROTASKS.with(|queue| !queue.borrow().is_empty())
}

/// Drops every task and queued closure of the stopped app version (its
/// code is about to be unloaded).
pub fn release_app() {
    drop(WOKEN.with(drain));
    MICROTASKS.with(|queue| queue.borrow_mut().clear());
    let tasks = TASKS.with(|tasks| std::mem::take(&mut *tasks.borrow_mut()));
    drop(tasks);
    // dropping futures may queue more (a promise settling): drop that too
    MICROTASKS.with(|queue| queue.borrow_mut().clear());
    super::promise::release_app();
}
