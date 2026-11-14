//! Port of `src/core/local_state.zig` — WP-2.4 (drawn early: `ui::local_state`).
//!
//! Per-instance component state: the runtime owns one heap slot per state id; a
//! slot that no rebuild asks for again is freed after that rebuild (the
//! component unmounted).
//!
//! Subtrees skipped by `ui::memo` do not run their builders, so the ids a memo
//! build touched are remembered on its node and touched again on a skip.
//!
//! Rust shape (plan.md §4): values are stored as `Rc<dyn Any>` and handed out as
//! `Rc<RefCell<T>>`; one id used with two different types panics. Dropping the
//! last `Rc` runs `T`'s destructor, so callbacks that captured a slot keep it
//! alive even after the component unmounted (safer than the Zig raw pointer).

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::core::hash::Wyhash;

type SlotValue = Rc<dyn Any>;

struct Entry {
    value: SlotValue,
    /// Rebuild that last asked for this slot.
    seen: u64,
}

/// The slot for `id`: the shared value plus whether this call created it.
pub struct Slot<T> {
    pub value: Rc<RefCell<T>>,
    /// Created by this call (first build of the instance).
    pub fresh: bool,
}

/// Runtime-owned store of local state slots.
pub struct Store {
    entries: HashMap<u64, Entry>,
    /// Rebuild counter; sweeping frees entries not seen in the current one.
    build: u64,
    /// Ids touched inside the memo build being reconciled (see `record`).
    recording: Vec<Vec<u64>>,
    /// Set while a rebuild runs; `sweep` only frees after one.
    building: bool,
}

impl Store {
    pub fn new() -> Store {
        Store {
            entries: HashMap::new(),
            build: 0,
            recording: Vec::new(),
            building: false,
        }
    }

    pub fn begin_build(&mut self) {
        self.build += 1;
        self.building = true;
    }

    /// The slot for `id`, created from `init` on first use.
    ///
    /// Panics when the same id is used with a different type (Zig:
    /// `@typeName` check).
    pub fn slot<T: 'static>(&mut self, id: u64, init: impl FnOnce() -> T) -> Slot<T> {
        if let Some(entry) = self.entries.get_mut(&id) {
            let Some(cell) = entry.value.clone().downcast::<RefCell<T>>().ok() else {
                panic!(
                    "Zant UI: local state id {id:x} used with two different types ({})",
                    std::any::type_name::<T>()
                );
            };
            self.touch_one(id);
            return Slot {
                value: cell,
                fresh: false,
            };
        }
        let value = Rc::new(RefCell::new(init()));
        let erased: SlotValue = value.clone();
        self.entries.insert(
            id,
            Entry {
                value: erased,
                seen: self.build,
            },
        );
        if let Some(list) = self.recording.last_mut() {
            list.push(id);
        }
        Slot { value, fresh: true }
    }

    /// Keeps the slots of a memo subtree that was skipped this rebuild.
    pub fn touch(&mut self, ids: &[u64]) {
        for &id in ids {
            if let Some(entry) = self.entries.get_mut(&id) {
                entry.seen = self.build;
                if let Some(list) = self.recording.last_mut() {
                    list.push(id);
                }
            }
        }
    }

    /// Marks one slot as used by the current rebuild (internal + memo recording).
    fn touch_one(&mut self, id: u64) {
        let build = self.build;
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.seen = build;
        }
        if let Some(list) = self.recording.last_mut() {
            list.push(id);
        }
    }

    /// Starts recording the ids touched while a memo subtree is built.
    pub fn record(&mut self) {
        self.recording.push(Vec::new());
    }

    /// Ends a recording; an enclosing memo also keeps the inner ids.
    pub fn end_record(&mut self) -> Vec<u64> {
        let Some(inner) = self.recording.pop() else {
            return Vec::new();
        };
        if let Some(outer) = self.recording.last_mut() {
            outer.extend_from_slice(&inner);
        }
        inner
    }

    /// After a rebuild: frees the slots it did not ask for (unmounted).
    pub fn sweep(&mut self) {
        if !self.building {
            return;
        }
        self.building = false;
        let build = self.build;
        let stale: Vec<u64> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.seen != build)
            .map(|(id, _)| *id)
            .collect();
        for id in stale {
            self.entries.remove(&id);
        }
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

impl Default for Store {
    fn default() -> Self {
        Store::new()
    }
}

/// One child instance id: its parent's id mixed with a name, key or position.
///
/// Zig: `childId(parent, part)` hashes `u64` parts with a trailing `"#"` and
/// byte-string parts with a trailing `"$"`, through Wyhash — the Rust port keeps
/// the same byte stream so ids match the Zig version.
pub fn child_id(parent: u64, part: impl ChildIdPart) -> u64 {
    let mut hasher = Wyhash::init(parent);
    part.hash_into(&mut hasher);
    hasher.finish()
}

/// A name / key / positional index of a child instance.
pub trait ChildIdPart {
    fn hash_into(&self, hasher: &mut Wyhash);
}

impl ChildIdPart for &str {
    fn hash_into(&self, hasher: &mut Wyhash) {
        hasher.update(self.as_bytes());
        hasher.update(b"$");
    }
}

impl ChildIdPart for String {
    fn hash_into(&self, hasher: &mut Wyhash) {
        hasher.update(self.as_bytes());
        hasher.update(b"$");
    }
}

impl ChildIdPart for &String {
    fn hash_into(&self, hasher: &mut Wyhash) {
        hasher.update(self.as_bytes());
        hasher.update(b"$");
    }
}

macro_rules! impl_child_id_unsigned {
    ($($ty:ty),* $(,)?) => {
        $(
            impl ChildIdPart for $ty {
                fn hash_into(&self, hasher: &mut Wyhash) {
                    hasher.update(&(*self as u64).to_le_bytes());
                    hasher.update(b"#");
                }
            }
        )*
    };
}

macro_rules! impl_child_id_signed {
    ($($ty:ty),* $(,)?) => {
        $(
            impl ChildIdPart for $ty {
                fn hash_into(&self, hasher: &mut Wyhash) {
                    // Zig `@intCast`s to u64; negative indices are a programming
                    // error there. Keep the two's-complement bits here.
                    hasher.update(&(*self as u64).to_le_bytes());
                    hasher.update(b"#");
                }
            }
        )*
    };
}

impl ChildIdPart for u64 {
    fn hash_into(&self, hasher: &mut Wyhash) {
        hasher.update(&self.to_le_bytes());
        hasher.update(b"#");
    }
}

impl ChildIdPart for u128 {
    fn hash_into(&self, hasher: &mut Wyhash) {
        hasher.update(&u64::try_from(*self).unwrap_or(0).to_le_bytes());
        hasher.update(b"#");
    }
}

impl_child_id_unsigned!(u8, u16, u32, usize);
impl_child_id_signed!(i8, i16, i32, i64, isize);

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn slots_persist_across_builds_and_free_when_not_rebuilt() {
        static FREED: AtomicUsize = AtomicUsize::new(0);
        struct S {
            n: u32,
        }
        impl Drop for S {
            fn drop(&mut self) {
                FREED.fetch_add(1, Ordering::SeqCst);
            }
        }
        let mut store = Store::new();
        store.begin_build();
        let first = store.slot(1, || S { n: 7 });
        assert!(first.fresh);
        assert_eq!(first.value.borrow().n, 7);
        first.value.borrow_mut().n = 9;
        drop(first);
        let _ = store.slot(2, || S { n: 0 });
        store.sweep();

        store.begin_build();
        let again = store.slot(1, || S { n: 0 });
        assert!(!again.fresh);
        assert_eq!(again.value.borrow().n, 9, "state persisted");
        drop(again);
        store.sweep();
        assert_eq!(
            FREED.load(Ordering::SeqCst),
            1,
            "the unmounted slot was dropped"
        );
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn memo_recordings_keep_skipped_slots_alive() {
        let mut store = Store::new();
        store.begin_build();
        store.record();
        store.record();
        let _ = store.slot(5, || 0u32);
        let inner = store.end_record();
        assert_eq!(inner, vec![5]);
        let _ = store.slot(6, || 0u32);
        let outer = store.end_record();
        assert_eq!(
            outer,
            vec![5, 6],
            "an enclosing memo also keeps the inner ids"
        );
        store.sweep();
        assert_eq!(store.count(), 2);

        // the memo is skipped: its ids are touched instead of rebuilt
        store.begin_build();
        store.touch(&outer);
        store.sweep();
        assert_eq!(store.count(), 2);
    }

    #[test]
    fn child_ids_differ_by_key_kind_and_parent() {
        assert_ne!(child_id(1, 3u64), child_id(1, "3"));
        assert_ne!(child_id(1, "a"), child_id(2, "a"));
        assert_eq!(child_id(9, "row"), child_id(9, "row"));
        assert_ne!(child_id(9, "row"), child_id(9, "column"));
    }

    /// Reference values produced by Zig 0.17.1 `childId`
    /// (`src/core/local_state.zig`, values printed with `zig run`): ids must be
    /// bit-identical across the two implementations so memo / local-state
    /// parity runs can compare them (plan.md §4).
    #[test]
    fn child_ids_match_zig() {
        assert_eq!(child_id(0, 0u64), 0x3076_e496_f66a_89bb);
        assert_eq!(child_id(0, "row"), 0x8943_aef2_fe85_1d89);
        assert_eq!(child_id(9, "row"), 0x288e_2b61_86b3_ff10);
        assert_eq!(child_id(1, 3u64), 0x1c2b_5cfb_b83b_73ea);
        assert_eq!(child_id(0xDEAD, 1u64), 0x164d_2cfb_bbd3_397a);
        assert_eq!(child_id(u64::MAX, "x"), 0x2142_bf46_c451_6f7e);
        assert_eq!(child_id(42, 0u64), 0x26e2_240a_8827_0202);
        // integers of another width go through the same u64 byte stream
        assert_eq!(child_id(1, 3u32), child_id(1, 3u64));
        assert_eq!(child_id(1, 3u8), child_id(1, 3u64));
    }

    #[test]
    fn one_id_with_two_types_panics() {
        let mut store = Store::new();
        store.begin_build();
        let _ = store.slot(1, || 0u32);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = store.slot(1, String::new);
        }));
        assert!(result.is_err(), "mismatched types must panic");
    }
}
