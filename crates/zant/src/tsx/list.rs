//! `tsx::List<T>`: the Rust type of a TypeScript array (`T[]`) in generated
//! code.
//!
//! It is an `Rc<Vec<T>>` with copy-on-write mutation, so cloning an array
//! (which generated code does constantly: every read of a non-`Copy` variable
//! clones) is a refcount bump and
//! `push` / `pop` / `splice` only copy when another handle is alive, exactly
//! like mutating a TS array does not affect a snapshot taken before.
//!
//! ```ignore
//! let mut rows = tsx::List::from_vec(vec![1, 2]);
//! let snapshot = rows.clone();
//! rows.push(3);
//! assert_eq!(snapshot.as_slice(), &[1, 2]);
//! ```

use std::ops::{Deref, DerefMut};
use std::rc::Rc;

/// A cheap-to-clone, copy-on-write array (TS `T[]`).
#[derive(Clone, Default)]
pub struct List<T>(Rc<Vec<T>>);

impl<T> List<T> {
    /// An empty list (TS `[]`).
    pub fn new() -> List<T> {
        List(Rc::new(Vec::new()))
    }

    /// Wraps an owned vector (TS array literal).
    pub fn from_vec(items: Vec<T>) -> List<T> {
        List(Rc::new(items))
    }

    /// The items as a plain slice.
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// The allocation shared by cloned lists / the runtime.
    pub fn as_rc(&self) -> &Rc<Vec<T>> {
        &self.0
    }

    /// Number of live handles: `1` means mutating will not copy.
    pub fn ref_count(&self) -> usize {
        Rc::strong_count(&self.0)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Mutation needs `T: Clone`: shared lists copy first (`Rc::make_mut`), which
/// is the copy-on-write rule of generated values (every TSX value is `Copy` or
/// cheaply `Clone`).
impl<T: Clone> List<T> {
    /// The unique vector, copied first when the list is shared.
    fn unique(&mut self) -> &mut Vec<T> {
        Rc::make_mut(&mut self.0)
    }

    /// TS `push`.
    pub fn push(&mut self, value: T) {
        self.unique().push(value);
    }

    /// TS `pop`.
    pub fn pop(&mut self) -> Option<T> {
        self.unique().pop()
    }

    /// TS `unshift`.
    pub fn unshift(&mut self, value: T) {
        self.unique().insert(0, value);
    }

    /// TS `shift`.
    pub fn shift(&mut self) -> Option<T> {
        let items = self.unique();
        if items.is_empty() {
            return None;
        }
        Some(items.remove(0))
    }

    /// `arr[i] = value`: grows with `fill()` values like assigning a TS array
    /// index.
    pub fn set(&mut self, index: usize, value: T, fill: impl Fn() -> T) {
        let items = self.unique();
        while items.len() <= index {
            items.push(fill());
        }
        items[index] = value;
    }

    /// TS `splice(start, deleteCount, ...items)`: the removed items.
    ///
    /// `start` is clamped like `Array.prototype.splice`; `delete` items are
    /// removed from there (fewer when the array ends first).
    pub fn splice(
        &mut self,
        start: f64,
        delete: usize,
        items: impl IntoIterator<Item = T>,
    ) -> List<T> {
        let len = self.len();
        let at = splice_start(start, len);
        let removed_count = delete.min(len - at);
        let replacement: Vec<T> = items.into_iter().collect();
        let vector = self.unique();
        let removed: Vec<T> = vector.splice(at..at + removed_count, replacement).collect();
        List(Rc::new(removed))
    }

    /// `arr.slice(start, end)`: a shallow copy of the range (JS clamping).
    pub fn slice(&self, start: f64, end: Option<f64>) -> List<T> {
        let len = self.len();
        let lo = super::slice_index(start, len);
        let hi = end.map_or(len, |value| super::slice_index(value, len));
        List(Rc::new(self.0[lo..lo.max(hi)].to_vec()))
    }

    /// TS `reverse` in place.
    pub fn reverse(&mut self) {
        self.unique().reverse();
    }

    /// Removes every element (TS `length = 0`).
    pub fn clear(&mut self) {
        self.unique().clear();
    }
}

impl<T> Deref for List<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.0
    }
}

impl<T: Clone> DerefMut for List<T> {
    /// Mutable slice access (copy-on-write first): sorting / in-place updates
    /// from generated code (`arr.sort(..)`, `arr[i] = ..` with an existing
    /// index) behave like the TS array's.
    fn deref_mut(&mut self) -> &mut [T] {
        Rc::make_mut(&mut self.0).as_mut_slice()
    }
}

impl<T> From<Vec<T>> for List<T> {
    fn from(items: Vec<T>) -> List<T> {
        List::from_vec(items)
    }
}

impl<T: Clone> From<&[T]> for List<T> {
    fn from(items: &[T]) -> List<T> {
        List::from_vec(items.to_vec())
    }
}

impl<T> FromIterator<T> for List<T> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> List<T> {
        List::from_vec(items.into_iter().collect())
    }
}

impl<T: Clone> IntoIterator for List<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        Rc::unwrap_or_clone(self.0).into_iter()
    }
}

impl<'a, T> IntoIterator for &'a List<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T: PartialEq> PartialEq for List<T> {
    fn eq(&self, other: &List<T>) -> bool {
        self.0 == other.0
    }
}

impl<T: Eq> Eq for List<T> {}

impl<T: std::fmt::Debug> std::fmt::Debug for List<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, formatter)
    }
}

/// `Array.prototype.splice` start: negative counts from the end, clamped to
/// `0..=len` (a fractional value truncates toward zero like ToIntegerOrInfinity
/// except for the NaN case, which is 0).
fn splice_start(start: f64, len: usize) -> usize {
    let len_f = len as f64;
    if start.is_nan() {
        return 0;
    }
    let value = start.trunc();
    if value < 0.0 {
        (len_f + value).max(0.0) as usize
    } else {
        value.min(len_f) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloning_is_a_refcount_bump_and_mutation_copies() {
        let mut rows = List::from_vec(vec![1, 2]);
        assert_eq!(rows.ref_count(), 1);
        let snapshot = rows.clone();
        assert_eq!(rows.ref_count(), 2);
        rows.push(3);
        assert_eq!(rows.as_slice(), &[1, 2, 3]);
        assert_eq!(snapshot.as_slice(), &[1, 2], "the snapshot is unchanged");
        assert_eq!(rows.ref_count(), 1);

        rows.pop();
        assert_eq!(rows.as_slice(), &[1, 2]);
        rows.unshift(0);
        assert_eq!(rows.as_slice(), &[0, 1, 2]);
        assert_eq!(rows.shift(), Some(0));
        assert_eq!(rows.shift(), Some(1));
        assert_eq!(rows.shift(), Some(2));
        assert_eq!(rows.shift(), None);
        assert!(rows.is_empty());
    }

    #[test]
    fn splice_removes_inserts_and_returns_the_removed_items() {
        let mut items = List::from_vec(vec!['a', 'b', 'c', 'd']);
        let removed = items.splice(1.0, 2, vec!['x', 'y', 'z']);
        assert_eq!(removed.as_slice(), &['b', 'c']);
        assert_eq!(items.as_slice(), &['a', 'x', 'y', 'z', 'd']);
        // negative start counts from the end
        let removed = items.splice(-1.0, 1, std::iter::empty());
        assert_eq!(removed.as_slice(), &['d']);
        // past the end removes nothing
        let removed = items.splice(100.0, 5, vec!['q']);
        assert!(removed.is_empty());
        assert_eq!(items.as_slice(), &['a', 'x', 'y', 'z', 'q']);
        // NaN start is 0
        let removed = items.splice(f64::NAN, 1, std::iter::empty());
        assert_eq!(removed.as_slice(), &['a']);
    }

    #[test]
    fn slice_uses_javascript_clamping() {
        let items = List::from_vec(vec![0, 1, 2, 3]);
        assert_eq!(items.slice(1.0, Some(3.0)).as_slice(), &[1, 2]);
        assert_eq!(items.slice(-2.0, None).as_slice(), &[2, 3]);
        assert_eq!(items.slice(2.0, Some(1.0)).as_slice(), &[] as &[i32]);
        assert_eq!(items.slice(-100.0, Some(100.0)).as_slice(), &[0, 1, 2, 3]);
    }

    #[test]
    fn deref_gives_slice_access_and_iteration() {
        let mut items = List::from_vec(vec![3, 1, 2]);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], 3);
        items.sort_unstable();
        assert_eq!(items.as_slice(), &[1, 2, 3]);
        let doubled: List<i32> = items.iter().map(|value| value * 2).collect();
        assert_eq!(doubled.as_slice(), &[2, 4, 6]);
        let refs: Vec<&i32> = (&items).into_iter().collect();
        assert_eq!(refs.len(), 3);
    }

    #[test]
    fn set_grows_with_fill_values() {
        let mut items: List<u8> = List::new();
        items.set(2, 7, || 0);
        assert_eq!(items.as_slice(), &[0, 0, 7]);
        items.set(0, 9, || 0);
        assert_eq!(items.as_slice(), &[9, 0, 7]);
    }

    #[test]
    fn equality_is_by_value() {
        assert_eq!(
            List::from_vec(vec![1, 2]),
            List::from_vec(vec![1, 2]),
            "lists compare by content, not allocation"
        );
        assert_ne!(List::from_vec(vec![1]), List::from_vec(vec![2]));
    }
}
