//! In-place `Array.prototype` operations on [`List`] with JavaScript's
//! argument rules: index assignment, `fill`, `splice` counts and the stable
//! `sort` (comparator or the default string order).

use std::cmp::Ordering;

use super::list::{List, splice_start};

impl<T: Clone> List<T> {
    /// `arr[i] = value`. Assigning at `length` appends; further out would
    /// leave holes, which Zant arrays do not have, so it panics with the
    /// TypeScript-side fix.
    pub fn assign(&mut self, index: usize, value: T) {
        let items = self.unique();
        match index.cmp(&items.len()) {
            Ordering::Less => items[index] = value,
            Ordering::Equal => items.push(value),
            Ordering::Greater => panic!(
                "array index {index} is past the end (length {}); use push() to add items",
                items.len()
            ),
        }
    }

    /// `arr.push(...items)`.
    pub fn extend_from(&mut self, items: &[T]) {
        self.unique().extend_from_slice(items);
    }

    /// `arr.fill(value, start?, end?)`: relative indexes like `slice`.
    pub fn fill(&mut self, value: T, start: Option<f64>, end: Option<f64>) {
        let len = self.len();
        let lo = start.map_or(0, |value| super::slice_index(value, len));
        let hi = end.map_or(len, |value| super::slice_index(value, len));
        for item in self.unique()[lo..lo.max(hi)].iter_mut() {
            *item = value.clone();
        }
    }

    /// `arr.splice(start, deleteCount, ...items)` with an optional count
    /// (absent: delete to the end).
    pub fn splice_js(&mut self, start: f64, delete: Option<f64>, items: Vec<T>) -> List<T> {
        let len = self.len();
        let at = splice_start(start, len);
        let count = match delete {
            None => len - at,
            Some(count) if count.is_nan() || count <= 0.0 => 0,
            Some(count) => (count.trunc().min((len - at) as f64)) as usize,
        };
        self.splice(at as f64, count, items)
    }

    /// `arr.sort(compare)`: stable; a NaN result counts as equal, as in
    /// `Array.prototype.sort`.
    pub fn sort_js(&mut self, mut compare: impl FnMut(T, T) -> f64) {
        self.unique().sort_by(|a, b| {
            let order = compare(a.clone(), b.clone());
            if order < 0.0 {
                Ordering::Less
            } else if order > 0.0 {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        });
    }

    /// `arr.sort()`: stable, comparing the items' `String(…)` spellings by
    /// UTF-16 code units (so `10` sorts before `9`); absent items go last.
    pub fn sort_default(&mut self, key: impl Fn(&T) -> Option<String>) {
        let mut keyed: Vec<(Option<String>, T)> =
            self.iter().map(|item| (key(item), item.clone())).collect();
        keyed.sort_by(|(a, _), (b, _)| match (a, b) {
            (Some(a), Some(b)) => a.encode_utf16().cmp(b.encode_utf16()),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        });
        *self.unique() = keyed.into_iter().map(|(_, item)| item).collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list<T: Clone>(items: &[T]) -> List<T> {
        List::from_vec(items.to_vec())
    }

    #[test]
    fn assignment_replaces_or_appends() {
        let mut items = list(&[1, 2]);
        items.assign(0, 5);
        items.assign(2, 7);
        assert_eq!(items.as_slice(), &[5, 2, 7]);
    }

    #[test]
    #[should_panic(expected = "past the end")]
    fn assignment_past_the_end_panics() {
        list(&[1]).assign(3, 2);
    }

    #[test]
    fn fill_uses_relative_indexes() {
        let mut items = list(&[1, 2, 3, 4]);
        items.fill(0, Some(-2.0), None);
        assert_eq!(items.as_slice(), &[1, 2, 0, 0]);
        items.fill(9, None, Some(1.0));
        assert_eq!(items.as_slice(), &[9, 2, 0, 0]);
    }

    #[test]
    fn splice_counts_follow_javascript() {
        let mut items = list(&[1, 2, 3, 4]);
        assert_eq!(items.splice_js(1.0, None, vec![]).as_slice(), &[2, 3, 4]);
        let mut items = list(&[1, 2, 3, 4]);
        assert_eq!(items.splice_js(-2.0, Some(1.5), vec![9]).as_slice(), &[3]);
        assert_eq!(items.as_slice(), &[1, 2, 9, 4]);
        assert!(items.splice_js(0.0, Some(f64::NAN), vec![]).is_empty());
    }

    #[test]
    fn sorting_is_stable_and_the_default_order_is_textual() {
        let mut pairs = list(&[(2, 'a'), (1, 'b'), (2, 'c'), (1, 'd')]);
        pairs.sort_js(|a, b| f64::from(a.0 - b.0));
        assert_eq!(pairs.as_slice(), &[(1, 'b'), (1, 'd'), (2, 'a'), (2, 'c')]);
        let mut numbers = list(&[10.0, 9.0, 1.0, f64::NAN]);
        numbers.sort_default(|n| Some(crate::tsx::builtins::string(*n).to_string()));
        assert_eq!(numbers[0], 1.0);
        assert_eq!(numbers[1], 10.0);
        assert_eq!(numbers[2], 9.0);
        assert!(numbers[3].is_nan());
        // UTF-16 order: U+FF61 sorts after U+1F600 (a surrogate pair)
        let mut text = list(&["\u{ff61}".to_owned(), "\u{1f600}".to_owned()]);
        text.sort_default(|s| Some(s.clone()));
        assert_eq!(text[0], "\u{1f600}");
        let mut absent = list(&[None, Some(2), Some(1)]);
        absent.sort_default(|n| n.map(|n: i32| n.to_string()));
        assert_eq!(absent.as_slice(), &[Some(1), Some(2), None]);
    }
}
