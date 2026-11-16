use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::Str;
use crate::tsx::error::Error;

/// Runs the queued tasks and microtasks with a stand-in app state.
fn checkpoint() {
    let mut root = ();
    task::run(&mut ActionCx::new(&mut root));
}

fn log() -> Rc<RefCell<Vec<String>>> {
    Rc::new(RefCell::new(Vec::new()))
}

fn error(message: &str) -> Thrown {
    Thrown::error(Error::new("Error", Str::from(message.to_owned())))
}

#[test]
fn reactions_run_later_in_order_and_chain() {
    let seen = log();
    let (promise, resolver) = Promise::<f64>::pending();
    let (a, b) = (seen.clone(), seen.clone());
    let doubled = promise.then(move |_, value| {
        a.borrow_mut().push(format!("first {value}"));
        Ok(value * 2.0)
    });
    doubled.then(move |_, value| {
        b.borrow_mut().push(format!("second {value}"));
        Ok(())
    });
    resolver.resolve(21.0);
    // settling never runs a reaction synchronously
    assert!(seen.borrow().is_empty());
    checkpoint();
    assert_eq!(*seen.borrow(), ["first 21", "second 42"]);
    // a later resolve is ignored
    resolver.resolve(1.0);
    checkpoint();
    assert_eq!(seen.borrow().len(), 2);
}

#[test]
fn a_rejection_skips_then_until_catch_and_finally_passes_through() {
    let seen = log();
    let (a, b, c) = (seen.clone(), seen.clone(), seen.clone());
    Promise::<f64>::reject(error("boom"))
        .then(move |_, value| {
            a.borrow_mut().push(format!("then {value}"));
            Ok(value)
        })
        .finally(move |_| {
            b.borrow_mut().push("finally".into());
            Ok(())
        })
        .catch(move |_, error| {
            c.borrow_mut().push(format!("catch {error}"));
            Ok(0.0)
        });
    checkpoint();
    assert_eq!(*seen.borrow(), ["finally", "catch Error: boom"]);
}

#[test]
fn a_returned_promise_is_adopted_and_many_can_react() {
    let seen = log();
    let (inner, inner_resolver) = Promise::<Str>::pending();
    let outer = Promise::resolve(1.0).then_promise(move |_, _| Ok(inner));
    for name in ["x", "y"] {
        let seen = seen.clone();
        outer.then(move |_, text| {
            seen.borrow_mut().push(format!("{name}={text}"));
            Ok(())
        });
    }
    checkpoint();
    assert!(seen.borrow().is_empty(), "waits for the adopted promise");
    inner_resolver.resolve(Str::Static("done"));
    checkpoint();
    assert_eq!(*seen.borrow(), ["x=done", "y=done"]);
}

#[test]
fn a_promise_is_a_future() {
    let seen = log();
    let (promise, resolver) = Promise::<f64>::pending();
    let result = Promise::from_future(async move {
        let value = promise.await?;
        Ok(value + 1.0)
    });
    let recorded = seen.clone();
    result.then(move |_, value| {
        recorded.borrow_mut().push(value.to_string());
        Ok(())
    });
    checkpoint();
    assert!(seen.borrow().is_empty());
    resolver.resolve(41.0);
    checkpoint();
    assert_eq!(*seen.borrow(), ["42"]);
}

#[test]
fn another_thread_wakes_the_task_on_this_thread() {
    let seen = log();
    let (sender, receiver) = tokio::sync::oneshot::channel::<String>();
    let ui_thread = std::thread::current().id();
    let recorded = seen.clone();
    task::spawn(async move {
        let text = receiver.await.unwrap_or_default();
        assert_eq!(std::thread::current().id(), ui_thread);
        recorded.borrow_mut().push(text);
    });
    checkpoint();
    std::thread::spawn(move || sender.send("from a worker".into()).unwrap())
        .join()
        .unwrap();
    assert!(task::pending(), "the wake queued the task");
    checkpoint();
    assert_eq!(*seen.borrow(), ["from a worker"]);
}

#[test]
fn release_drops_pending_work() {
    let seen = log();
    let recorded = seen.clone();
    Promise::resolve(1.0).then(move |_, _| {
        recorded.borrow_mut().push("ran".into());
        Ok(())
    });
    task::release_app();
    checkpoint();
    assert!(seen.borrow().is_empty());
}
