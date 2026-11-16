//! JavaScript promises for generated TSX: `Promise<T>` with `then` /
//! `catch` / `finally`, usable from Rust as a `Future`.
//!
//! The JavaScript rules that matter for app code hold:
//! - a promise settles once; later resolve / reject calls are ignored;
//! - every reaction runs later, as a microtask on the UI thread
//!   ([`super::task::queue`]), never inside the call that settled it;
//! - any number of reactions; `then` returns a new promise (a callback
//!   returning a promise is adopted: `then_promise`); a rejection skips
//!   `then` callbacks until a `catch`; `finally` passes the result on;
//! - a rejection nobody handles is reported ("Uncaught (in promise) …")
//!   once the microtasks have run, like a browser.
//!
//! Reactions get the app's `ActionCx`, so `.then(setItems)` updates state.
//! Awaiting a settled promise does not yield first (JavaScript would wait
//! one microtask); for UI code the order of independent awaits is the same.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::task::{Context, Poll, Waker};

use super::error::Thrown;
use super::task;
use crate::ui::ActionCx;

/// A reaction: runs once with the settled result.
type Reaction<T> = Box<dyn FnOnce(&mut ActionCx<'_>, Result<T, Thrown>)>;

struct Inner<T> {
    result: Option<Result<T, Thrown>>,
    reactions: Vec<Reaction<T>>,
    wakers: Vec<Waker>,
    /// Something reacted to or awaited it (a rejection is then handled).
    observed: bool,
}

/// A JavaScript promise (UI thread only; clones share the promise).
pub struct Promise<T>(Rc<RefCell<Inner<T>>>);

impl<T> Clone for Promise<T> {
    fn clone(&self) -> Self {
        Promise(Rc::clone(&self.0))
    }
}

impl<T> std::fmt::Debug for Promise<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.0.borrow();
        f.write_str(match &inner.result {
            None => "Promise { <pending> }",
            Some(Ok(_)) => "Promise { <fulfilled> }",
            Some(Err(_)) => "Promise { <rejected> }",
        })
    }
}

impl<T> PartialEq for Promise<T> {
    /// Promises compare by identity (JavaScript objects).
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl<T: Clone + 'static> Default for Promise<T> {
    /// A promise that never settles (the value of an unset field).
    fn default() -> Self {
        Promise::pending().0
    }
}

/// Settles its promise (`resolve` / `reject` of `new Promise(…)`).
pub struct Resolver<T>(Promise<T>);

impl<T: Clone + 'static> Resolver<T> {
    pub fn resolve(&self, value: T) {
        self.0.settle(Ok(value));
    }

    pub fn reject(&self, error: Thrown) {
        self.0.settle(Err(error));
    }

    pub fn settle(&self, result: Result<T, Thrown>) {
        self.0.settle(result);
    }
}

/// Checks a rejected promise: its error while nobody has observed it.
type UnhandledCheck = Box<dyn Fn() -> Option<Thrown>>;

thread_local! {
    /// Rejected promises nobody has observed yet (checked after microtasks).
    static UNHANDLED: RefCell<Vec<UnhandledCheck>> = const { RefCell::new(Vec::new()) };
}

impl<T: Clone + 'static> Promise<T> {
    /// A pending promise and the resolver that settles it.
    pub fn pending() -> (Promise<T>, Resolver<T>) {
        let promise = Promise(Rc::new(RefCell::new(Inner {
            result: None,
            reactions: Vec::new(),
            wakers: Vec::new(),
            observed: false,
        })));
        (promise.clone(), Resolver(promise))
    }

    /// `Promise.resolve(value)`.
    pub fn resolve(value: T) -> Promise<T> {
        let (promise, resolver) = Promise::pending();
        resolver.resolve(value);
        promise
    }

    /// `Promise.reject(error)`.
    pub fn reject(error: Thrown) -> Promise<T> {
        let (promise, resolver) = Promise::pending();
        resolver.reject(error);
        promise
    }

    /// A promise settled by `future`, which runs on the UI thread's executor.
    pub fn from_future(future: impl Future<Output = Result<T, Thrown>> + 'static) -> Promise<T> {
        let (promise, resolver) = Promise::pending();
        task::spawn(async move { resolver.settle(future.await) });
        promise
    }

    fn settle(&self, result: Result<T, Thrown>) {
        let (reactions, wakers) = {
            let mut inner = self.0.borrow_mut();
            if inner.result.is_some() {
                return;
            }
            inner.result = Some(result.clone());
            (
                std::mem::take(&mut inner.reactions),
                std::mem::take(&mut inner.wakers),
            )
        };
        if result.is_err() && reactions.is_empty() && wakers.is_empty() {
            let watched: Weak<RefCell<Inner<T>>> = Rc::downgrade(&self.0);
            UNHANDLED.with(|list| {
                list.borrow_mut().push(Box::new(move || {
                    let inner = watched.upgrade()?;
                    let inner = inner.borrow();
                    match (&inner.result, inner.observed) {
                        (Some(Err(error)), false) => Some(error.clone()),
                        _ => None,
                    }
                }))
            });
        }
        for reaction in reactions {
            let result = result.clone();
            task::queue(move |cx| reaction(cx, result));
        }
        for waker in wakers {
            waker.wake();
        }
    }

    /// Runs `reaction` once settled (as a microtask).
    fn subscribe(&self, reaction: Reaction<T>) {
        let mut inner = self.0.borrow_mut();
        inner.observed = true;
        match &inner.result {
            Some(result) => {
                let result = result.clone();
                task::queue(move |cx| reaction(cx, result));
            }
            None => inner.reactions.push(reaction),
        }
    }

    /// `p.then(value => …)` returning a value (or throwing: `Err`).
    pub fn then<U: Clone + 'static>(
        &self,
        on_fulfilled: impl FnOnce(&mut ActionCx<'_>, T) -> Result<U, Thrown> + 'static,
    ) -> Promise<U> {
        let (next, resolver) = Promise::pending();
        self.subscribe(Box::new(move |cx, result| match result {
            Ok(value) => resolver.settle(on_fulfilled(cx, value)),
            Err(error) => resolver.reject(error),
        }));
        next
    }

    /// `p.then(value => anotherPromise)`: the returned promise is adopted.
    pub fn then_promise<U: Clone + 'static>(
        &self,
        on_fulfilled: impl FnOnce(&mut ActionCx<'_>, T) -> Result<Promise<U>, Thrown> + 'static,
    ) -> Promise<U> {
        let (next, resolver) = Promise::pending();
        self.subscribe(Box::new(move |cx, result| match result {
            Ok(value) => match on_fulfilled(cx, value) {
                Ok(inner) => inner.subscribe(Box::new(move |_, result| resolver.settle(result))),
                Err(error) => resolver.reject(error),
            },
            Err(error) => resolver.reject(error),
        }));
        next
    }

    /// `p.catch(error => …)`: a value for the rejected case.
    pub fn catch(
        &self,
        on_rejected: impl FnOnce(&mut ActionCx<'_>, Thrown) -> Result<T, Thrown> + 'static,
    ) -> Promise<T> {
        let (next, resolver) = Promise::pending();
        self.subscribe(Box::new(move |cx, result| match result {
            Ok(value) => resolver.resolve(value),
            Err(error) => resolver.settle(on_rejected(cx, error)),
        }));
        next
    }

    /// `p.finally(() => …)`: runs either way; the result passes through
    /// unless the callback throws.
    pub fn finally(
        &self,
        on_settled: impl FnOnce(&mut ActionCx<'_>) -> Result<(), Thrown> + 'static,
    ) -> Promise<T> {
        let (next, resolver) = Promise::pending();
        self.subscribe(Box::new(move |cx, result| match on_settled(cx) {
            Ok(()) => resolver.settle(result),
            Err(error) => resolver.reject(error),
        }));
        next
    }
}

impl<T: Clone + 'static> Future for Promise<T> {
    type Output = Result<T, Thrown>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.0.borrow_mut();
        inner.observed = true;
        match &inner.result {
            Some(result) => Poll::Ready(result.clone()),
            None => {
                if !inner.wakers.iter().any(|known| known.will_wake(cx.waker())) {
                    inner.wakers.push(cx.waker().clone());
                }
                Poll::Pending
            }
        }
    }
}

/// Reports the rejections still unobserved after a microtask checkpoint.
pub(crate) fn report_unhandled() {
    let watched = UNHANDLED.with(|list| std::mem::take(&mut *list.borrow_mut()));
    for check in watched {
        if let Some(error) = check() {
            eprintln!("Uncaught (in promise) {error}");
        }
    }
}

pub(crate) fn release_app() {
    UNHANDLED.with(|list| list.borrow_mut().clear());
}

#[cfg(test)]
mod tests;
