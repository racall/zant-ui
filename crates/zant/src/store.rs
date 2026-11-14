//! Port of `src/store.zig` — WP-2.8 (drawn early: it is self-contained).
//!
//! A tiny Zustand-style store for UI state.
//!
//! - `get()` returns a mutable reference to the current state.
//! - `update(|state| ..)` or `set(next)` notify subscribers.
//! - `notify()` notifies after direct mutation through `get()`.
//!
//! The store is intended for the UI thread. It is deliberately not thread-safe
//! (like the Zig one): it holds an `Rc` of listeners and is `!Send`.

use std::cell::RefCell;
use std::rc::Rc;

type Listener = Box<dyn FnMut()>;

/// A tiny Zustand-style store for UI state (`Store(T)` in Zig).
pub struct Store<T> {
    state: T,
    listeners: Vec<Listener>,
}

impl<T> Store<T> {
    /// Creates a store. Zig returned an allocated `*Store(T)`; Rust callers use
    /// `Rc<RefCell<Store<T>>>` where shared mutation is needed.
    pub fn new(initial: T) -> Store<T> {
        Store {
            state: initial,
            listeners: Vec::new(),
        }
    }

    pub fn get(&self) -> &T {
        &self.state
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.state
    }

    pub fn set(&mut self, next: T) {
        self.state = next;
        self.notify();
    }

    /// Mutates the state through `update` and notifies subscribers
    /// (Zustand's `set(partial)`; Zig's `update(.{ ... })` took a struct literal).
    pub fn update(&mut self, update: impl FnOnce(&mut T)) {
        update(&mut self.state);
        self.notify();
    }

    pub fn subscribe(&mut self, listener: impl FnMut() + 'static) {
        self.listeners.push(Box::new(listener));
    }

    pub fn notify(&mut self) {
        for listener in &mut self.listeners {
            listener();
        }
    }
}

/// Shared store handle: `Rc<RefCell<Store<T>>>`.
pub type SharedStore<T> = Rc<RefCell<Store<T>>>;

/// `store::create` in Zig: a new shared store.
pub fn create<T>(initial: T) -> SharedStore<T> {
    Rc::new(RefCell::new(Store::new(initial)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default, PartialEq, Debug)]
    struct Counter {
        count: u32,
        name: String,
    }

    #[test]
    fn update_notifies_subscribers() {
        let mut store = Store::new(Counter::default());
        let seen = Rc::new(RefCell::new(0usize));
        let listener_seen = Rc::clone(&seen);
        store.subscribe(move || {
            *listener_seen.borrow_mut() += 1;
        });
        store.update(|state| {
            state.count = 3;
            state.name = "three".to_owned();
        });
        assert_eq!(store.get().count, 3);
        assert_eq!(store.get().name, "three");
        assert_eq!(*seen.borrow(), 1);
    }

    #[test]
    fn set_replaces_the_state_and_notifies() {
        let mut store = Store::new(Counter::default());
        let hits = Rc::new(RefCell::new(0));
        let listener_hits = Rc::clone(&hits);
        store.subscribe(move || *listener_hits.borrow_mut() += 1);
        store.set(Counter {
            count: 7,
            name: "seven".to_owned(),
        });
        assert_eq!(store.get().count, 7);
        assert_eq!(*hits.borrow(), 1);
        store.notify();
        assert_eq!(*hits.borrow(), 2);
    }

    #[test]
    fn shared_store_is_usable_through_a_handle() {
        let store = create(Counter::default());
        store.borrow_mut().update(|state| state.count = 1);
        assert_eq!(store.borrow().get().count, 1);
    }
}
