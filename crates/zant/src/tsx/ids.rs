//! Child-component instance ids of TSX-generated code (`tsx::child_id` /
//! `tsx::site_id`).
//!
//! A stateful child component gets its `local_state` slot from the parent's
//! instance id mixed with:
//!
//! - `child_id`: the component's name and its `key` prop (string or number), or
//! - `site_id`: the component's name and its call-site number (no key).
//!
//! The hash is the same Wyhash stream as `core::local_state::child_id`; the
//! resulting ids are pinned by tests.

use crate::core::hash::Wyhash;
use crate::core::local_state::child_id as local_child_id;

/// The id of a keyed child instance: `parent_id ^ hash(0, name)` mixed with
/// `key` (a string, a number or an integer id).
///
/// TSX: `<Counter key={row.id} />` → `tsx::child_id(parent, "Counter", row.id)`.
pub fn child_id<K: ChildKey>(parent: u64, name: &str, key: K) -> u64 {
    let named = parent ^ Wyhash::hash(0, name.as_bytes());
    key.child_id_part(named)
}

/// The id of a key-less child instance: call sites are numbered by source
/// position (`site`), so it never collides with a keyed sibling.
///
/// TSX: `<Counter />` → `tsx::site_id(parent, "Counter", 0)`.
pub fn site_id(parent: u64, name: &str, site: u32) -> u64 {
    local_child_id(parent ^ Wyhash::hash(1, name.as_bytes()), site)
}

/// A TSX `key` value: a string or a number (floats are compared by
/// their f64 bits, like `tsx::list_key`).
pub trait ChildKey {
    fn child_id_part(self, parent: u64) -> u64;
}

impl ChildKey for &str {
    fn child_id_part(self, parent: u64) -> u64 {
        local_child_id(parent, self)
    }
}

impl ChildKey for String {
    fn child_id_part(self, parent: u64) -> u64 {
        local_child_id(parent, self.as_str())
    }
}

impl ChildKey for &String {
    fn child_id_part(self, parent: u64) -> u64 {
        local_child_id(parent, self.as_str())
    }
}

macro_rules! child_key_int {
    ($($ty:ty),* $(,)?) => {
        $(
            impl ChildKey for $ty {
                fn child_id_part(self, parent: u64) -> u64 {
                    local_child_id(parent, self)
                }
            }
        )*
    };
}

child_key_int!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

impl ChildKey for f64 {
    fn child_id_part(self, parent: u64) -> u64 {
        local_child_id(parent, self.to_bits())
    }
}

impl ChildKey for f32 {
    fn child_id_part(self, parent: u64) -> u64 {
        local_child_id(parent, f64::from(self).to_bits())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pinned regression values of the Wyhash-based ids.
    #[test]
    fn ids_are_pinned() {
        assert_eq!(child_id(0, "Child", 0u64), 0x4232_64cd_89a6_5067);
        assert_eq!(child_id(0, "Child", "row"), 0x7240_374a_9028_02dd);
        assert_eq!(child_id(42, "Row", 7u32), 0x2a3f_103b_05e3_5c30);
        assert_eq!(child_id(0xDEAD, "Item", 1.5f64), 0x5a06_0303_9823_7823);
        assert_eq!(site_id(0, "Child", 0), 0x59bf_6044_b947_4071);
        assert_eq!(site_id(0, "Child", 3), 0x7da4_9531_c58b_7616);
        assert_eq!(site_id(0xDEAD, "Row", 2), 0x47b4_c3d6_7a9b_ca22);
        assert_eq!(site_id(1, "a-b", 9), 0x66cc_44b1_0ecf_55de);
    }

    #[test]
    fn key_kind_and_parent_separate_instances() {
        assert_ne!(child_id(1, "Row", 3u64), child_id(1, "Row", "3"));
        assert_ne!(child_id(1, "Row", "a"), child_id(2, "Row", "a"));
        assert_eq!(child_id(9, "Row", "a"), child_id(9, "Row", "a"));
        assert_ne!(child_id(9, "Row", "a"), child_id(9, "column", "a"));
        assert_ne!(site_id(9, "Row", 0), site_id(9, "Row", 1));
        assert_ne!(site_id(9, "Row", 0), child_id(9, "Row", 0u64));
        // integers of another width go through the same u64 byte stream
        assert_eq!(child_id(1, "Row", 3u8), child_id(1, "Row", 3u64));
        assert_eq!(
            child_id(1, "Row", String::from("k")),
            child_id(1, "Row", "k")
        );
        assert_eq!(child_id(1, "Row", 1.5f32), child_id(1, "Row", 1.5f64));
    }
}
