//! `ui::local_state` / `ui::local_slot` / `ui::child_id`.
//!
//! Component-local state that lives while the instance keeps being built: the
//! slot for `id` is created from `initial` on the first build and freed after
//! the first rebuild that does not ask for it (unmount). The `Rc<RefCell<T>>`
//! stays valid for callbacks of the frame; unmount drops the store's handle.
//!
//! ```ignore
//! let open = ui::local_state(ui::child_id(parent_id, "faq-3"), || false);
//! ```
#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::local_state::{Slot, Store};

use super::context;

thread_local! {
    /// Fallback store for builds outside a runtime (tests, previews).
    static FALLBACK: RefCell<Store> = RefCell::new(Store::new());
}

pub(crate) fn release_app() {
    let old = FALLBACK.with(|store| store.replace(Store::new()));
    drop(old);
}

fn with_store<R>(f: impl FnOnce(&mut Store) -> R) -> R {
    match context::build_states() {
        Some(states) => f(&mut states.borrow_mut()),
        None => FALLBACK.with(|store| f(&mut store.borrow_mut())),
    }
}

/// Component-local state (`ui::local_state`): the slot for `id`, created from
/// `init` on the first build of the instance.
pub fn local_state<T: 'static>(id: u64, init: impl FnOnce() -> T) -> Rc<RefCell<T>> {
    with_store(|store| store.slot(id, init).value)
}

/// Component-local state that participates in hot reload snapshots.
pub fn local_state_hot<T: crate::tsx::hot::Hot + 'static>(
    id: u64,
    init: impl FnOnce() -> T,
) -> Rc<RefCell<T>> {
    with_store(|store| store.slot_hot(id, init).value)
}

/// `local_state` without discarding `fresh` (`ui::local_slot`): the caller can
/// initialise the value only on the first build.
pub fn local_slot<T: 'static>(id: u64, init: impl FnOnce() -> T) -> Slot<T> {
    with_store(|store| store.slot(id, init))
}

/// The id of a child instance: the parent's id mixed with a name, key or
/// position (`&str` or an integer).
pub fn child_id(parent: u64, part: impl crate::core::local_state::ChildIdPart) -> u64 {
    crate::core::local_state::child_id(parent, part)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_state_persists_in_the_fallback_store() {
        let id = 0xFA11_BACC_u64;
        let first = local_state(id, || 5u32);
        assert_eq!(*first.borrow(), 5);
        *first.borrow_mut() = 9;
        drop(first);
        let again = local_state(id, || 0u32);
        assert_eq!(*again.borrow(), 9, "the slot is not re-initialised");
    }

    #[test]
    fn local_slot_reports_fresh_only_once() {
        let id = 0xFEED_BEEF_u64;
        let first = local_slot(id, || 1u8);
        assert!(first.fresh);
        drop(first);
        let again = local_slot(id, || 0u8);
        assert!(!again.fresh);
    }

    #[test]
    fn child_ids_are_stable_and_typed() {
        assert_eq!(child_id(7, "row"), child_id(7, "row"));
        assert_ne!(child_id(7, 1u64), child_id(7, "1"));
    }

    #[test]
    fn runtime_store_drives_persistence_and_sweeps() {
        let store = Rc::new(RefCell::new(Store::new()));
        context::set_build_states(Some(Rc::clone(&store)));

        store.borrow_mut().begin_build();
        {
            let slot = local_state(0x5EED_u64, || 1u32);
            assert_eq!(*slot.borrow(), 1);
            *slot.borrow_mut() = 7;
        }
        store.borrow_mut().sweep();
        assert_eq!(store.borrow().count(), 1);

        store.borrow_mut().begin_build();
        let again = local_state(0x5EED_u64, || 0u32);
        assert_eq!(
            *again.borrow(),
            7,
            "the runtime store keeps the value across builds"
        );
        drop(again);
        store.borrow_mut().sweep();
        assert_eq!(store.borrow().count(), 1);

        // the next rebuild does not ask for the slot: it unmounted
        store.borrow_mut().begin_build();
        store.borrow_mut().sweep();
        assert_eq!(store.borrow().count(), 0);

        context::set_build_states(None);
    }

    #[test]
    fn memo_recording_keeps_skipped_slots_alive() {
        let store = Rc::new(RefCell::new(Store::new()));
        context::set_build_states(Some(Rc::clone(&store)));

        // a memo build records the ids it touches
        store.borrow_mut().begin_build();
        store.borrow_mut().record();
        let _ = local_state(0xA1_u64, || 0u8);
        let recorded = store.borrow_mut().end_record();
        store.borrow_mut().sweep();
        assert_eq!(recorded, vec![0xA1]);
        assert_eq!(store.borrow().count(), 1);

        // the memo is skipped next build: its ids are touched instead
        store.borrow_mut().begin_build();
        store.borrow_mut().touch(&recorded);
        store.borrow_mut().sweep();
        assert_eq!(
            store.borrow().count(),
            1,
            "the skipped memo keeps its component state"
        );

        store.borrow_mut().begin_build();
        store.borrow_mut().sweep();
        assert_eq!(store.borrow().count(), 0, "the memo subtree unmounted");

        context::set_build_states(None);
    }
}
