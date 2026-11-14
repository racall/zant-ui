//! Runtime support of the TSX hooks (`useMemo` / `useEffect` / `useRef` and
//! typed callback props): `tsx::Deps`, `tsx::Memo`, `tsx::Effect`,
//! `tsx::Callback`, `tsx::Ref`.
//!
//! Port of `src/tsx_hooks.zig` and the `Deps` / `Memo` / `Effect` /
//! `deepEql` / `Callback` parts of `src/tsx.zig`. The Zig version copied
//! dependency values into an arena because the frame arena owned them; Rust
//! hook values are `Clone` and compared by value, so the hook just stores the
//! last list (`docs/rust-port/tsx-codegen.md` §3: generated values are `Copy`
//! or cheaply `Clone`).
//!
//! ```ignore
//! // useMemo: the field keeps the value while the dependency list is equal
//! let matches: tsx::List<Row> = match self._memo1.hit((self.query.clone(), self.rows.clone())) {
//!     Some(cached) => cached,
//!     None => self._memo1.store(compute(&self.query, &self.rows)),
//! };
//!
//! // useEffect: the field queues the effect (and the previous cleanup)
//! if self._effect2.changed((self.query.clone(),)) {
//!     self._effect2.queue(ui::action(|s: &mut App| s._load()));
//! }
//! ```

use std::cell::RefCell;
use std::rc::Rc;

use crate::ui::Action;

/// A `useRef` value: the live value, shared with the closures that write it
/// (`src/tsx.zig` had no ref runtime: generated Zig used a plain field, but a
/// Rust closure needs shared ownership). `useRef` writes do not snapshot the
/// state, they mutate through `borrow_mut()`.
pub type Ref<T> = Rc<RefCell<T>>;

/// `useRef(initial)`: a new shared ref value.
pub fn new_ref<T>(value: T) -> Ref<T> {
    Rc::new(RefCell::new(value))
}

/// `ui::localState` for generated child components (re-exported here so
/// generated code only needs `tsx::`): the `Rc<RefCell<T>>` slot of instance
/// `id`, created from `init` on the first build.
pub fn local_state<T: 'static>(id: u64, init: impl FnOnce() -> T) -> Ref<T> {
    crate::ui::local_state(id, init)
}

/// The dependency list of `useMemo` / `useEffect`: the last list, compared by
/// value (Zig `Deps(D)`; `deepEql` is Rust's `PartialEq`).
///
/// Generated code passes the dependency tuple, e.g. `(self.a.clone(),
/// self.items.clone())`; an empty list is `()`.
pub struct Deps<D> {
    last: Option<D>,
}

impl<D> Deps<D> {
    pub fn new() -> Deps<D> {
        Deps { last: None }
    }

    /// Forgets the last list, so the next [`Deps::changed`] reports true again.
    pub fn clear(&mut self) {
        self.last = None;
    }
}

impl<D> Default for Deps<D> {
    fn default() -> Deps<D> {
        Deps::new()
    }
}

impl<D: Clone + PartialEq> Deps<D> {
    /// True on the first call and whenever `deps` differs from the last list;
    /// remembers `deps`.
    pub fn changed(&mut self, deps: D) -> bool {
        if self.last.as_ref().is_some_and(|last| *last == deps) {
            return false;
        }
        self.last = Some(deps);
        true
    }
}

/// `useMemo`: the value computed for the current dependency list
/// (Zig `Memo(T, D)`). A cached value is cloned out: generated values are
/// cheap to clone (`docs/rust-port/tsx-codegen.md` §3).
pub struct Memo<T, D> {
    deps: Deps<D>,
    value: Option<T>,
}

impl<T, D> Memo<T, D> {
    pub fn new() -> Memo<T, D> {
        Memo {
            deps: Deps::new(),
            value: None,
        }
    }

    /// The cached value when `deps` is unchanged; `None` means compute it and
    /// call [`Memo::store`].
    pub fn hit(&mut self, deps: D) -> Option<T>
    where
        T: Clone,
        D: Clone + PartialEq,
    {
        if !self.deps.changed(deps) {
            return self.value.clone();
        }
        self.value = None;
        None
    }

    /// Caches `value` for the dependency list just stored by [`Memo::hit`] and
    /// returns it.
    pub fn store(&mut self, value: T) -> T
    where
        T: Clone,
    {
        self.value = Some(value.clone());
        value
    }

    /// The cached value without comparing the dependency list.
    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }
}

impl<T, D> Default for Memo<T, D> {
    fn default() -> Memo<T, D> {
        Memo::new()
    }
}

/// `useEffect` with a cleanup function: the dependency list plus the cleanup
/// the last run returned (Zig `Effect(D)`).
///
/// The cleanup is queued as an effect before the effect runs again
/// ([`Effect::queue`]) and when the component unmounts: dropping the
/// `Effect` (its `local_state` slot is freed) queues the cleanup for the next
/// effect run, where the root state is available again. Zig called the cleanup
/// with a raw state pointer at that point; Rust runs it as an effect, which is
/// the safe equivalent (same call order: after the rebuild).
pub struct Effect<D> {
    deps: Deps<D>,
    cleanup: Option<Action>,
}

impl<D> Effect<D> {
    pub fn new() -> Effect<D> {
        Effect {
            deps: Deps::new(),
            cleanup: None,
        }
    }
}

impl<D> Default for Effect<D> {
    fn default() -> Effect<D> {
        Effect::new()
    }
}

impl<D: Clone + PartialEq> Effect<D> {
    /// True on the first call and whenever the dependency list changed.
    pub fn changed(&mut self, deps: D) -> bool {
        self.deps.changed(deps)
    }

    /// Queues the previous cleanup, then the effect (`ui::effect`).
    pub fn queue(&mut self, effect: Action) {
        if let Some(cleanup) = self.cleanup.take() {
            crate::ui::effect(cleanup);
        }
        crate::ui::effect(effect);
    }

    /// Keeps `cleanup` for the next run / unmount (`return () => …` in the
    /// effect body).
    pub fn set_cleanup(&mut self, cleanup: Action) {
        self.cleanup = Some(cleanup);
    }

    /// Runs the cleanup now (generated code does not call this; [`Drop`] and
    /// [`Effect::queue`] do).
    pub fn run_cleanup(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            crate::ui::effect(cleanup);
        }
    }
}

impl<D> Drop for Effect<D> {
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            crate::ui::effect(cleanup);
        }
    }
}

/// A typed callback prop of a TSX child component (`onToggle: (id: number) =>
/// void`): generated Zig had no closures and used `tsx.Callback(fn (u32)
/// void)`; Rust's cheap closure is `Rc<dyn Fn(..)>`
/// (`docs/rust-port/tsx-codegen.md` §2), so this alias names that type.
///
/// `tsx::Callback<dyn Fn(u32)>` is `Rc<dyn Fn(u32)>`.
pub type Callback<F> = Rc<F>;

/// Wraps a closure as a [`Callback`].
pub fn callback<F: 'static>(function: F) -> Callback<F> {
    Rc::new(function)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn deps_and_memo_compare_by_value() {
        let mut deps: Deps<(String, u32)> = Deps::new();
        assert!(deps.changed((String::from("ab"), 1)));
        assert!(!deps.changed((String::from("ab"), 1)));
        assert!(deps.changed((String::from("ab"), 2)));
        assert!(deps.changed((String::from("ax"), 2)));

        let mut memo: Memo<Vec<u32>, (u32,)> = Memo::new();
        assert_eq!(memo.hit((1,)), None);
        assert_eq!(memo.store(vec![4, 5]), vec![4, 5]);
        assert_eq!(memo.hit((1,)), Some(vec![4, 5]));
        assert_eq!(memo.hit((2,)), None);
        assert_eq!(
            memo.get(),
            None,
            "hit with a new list drops the cached value"
        );
    }

    #[test]
    fn effect_keeps_its_cleanup_until_the_next_run_or_drop() {
        thread_local! {
            static CLEANED: Cell<u32> = const { Cell::new(0) };
            static SEEN: Cell<usize> = const { Cell::new(0) };
            static QUEUED: Cell<u32> = const { Cell::new(0) };
        }
        CLEANED.with(|value| value.set(0));
        SEEN.with(|value| value.set(0));
        QUEUED.with(|value| value.set(0));
        crate::ui::begin_frame();
        let mut effect: Effect<(u32,)> = Effect::new();
        assert!(effect.changed((1,)));
        let text = String::from("abc");
        let kept = text.clone();
        effect.set_cleanup(Action::new(move |_| {
            CLEANED.with(|value| value.set(value.get() + 1));
            SEEN.with(|value| value.set(kept.len()));
        }));
        assert!(!effect.changed((1,)));
        effect.run_cleanup();
        // the cleanup is queued as an effect; run the queue like the runtime
        let mut root = ();
        let mut cx = crate::ui::ActionCx::new(&mut root);
        crate::ui::run_effects(&mut cx);
        assert_eq!(CLEANED.with(Cell::get), 1);
        assert_eq!(SEEN.with(Cell::get), 3);
        effect.run_cleanup(); // once
        crate::ui::run_effects(&mut cx);
        assert_eq!(CLEANED.with(Cell::get), 1);
        effect.set_cleanup(Action::new(|_| {
            QUEUED.with(|value| value.set(value.get() + 1));
        }));
        drop(effect);
        crate::ui::run_effects(&mut cx);
        assert_eq!(QUEUED.with(Cell::get), 1, "unmounting queues the cleanup");
        crate::ui::end_frame();
    }

    #[test]
    fn typed_callbacks_pass_their_arguments_through() {
        let total = new_ref(0.0_f64);
        let slot = Rc::clone(&total);
        let callback: Callback<dyn Fn(f64, u32)> = callback(move |amount: f64, times: u32| {
            *slot.borrow_mut() += amount * f64::from(times)
        });
        callback(1.5, 2);
        callback(1.0, 1);
        assert_eq!(*total.borrow(), 4.0);
    }

    #[test]
    fn refs_hold_the_live_value() {
        let counter = new_ref(1u32);
        let writer = Rc::clone(&counter);
        *writer.borrow_mut() += 1;
        assert_eq!(*counter.borrow(), 2);
    }
}
