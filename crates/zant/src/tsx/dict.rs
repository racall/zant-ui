//! `Record<K, V>` / `{ [key: string]: V }` / `Map<K, V>` ([`Dict`]) and
//! `Set<T>` ([`Set`]): insertion-ordered like JavaScript's `Map` and `Set`
//! (overwriting keeps the position, deleting and re-adding moves to the
//! end), and copy-on-write values like [`List`].
//!
//! Keys are strings, numbers (SameValueZero: `NaN` equals `NaN`, `+0`
//! equals `-0`), booleans or string-union enums ([`DictKey`]).

use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

use super::List;
use crate::Str;

/// A TSX value usable as a key: how it is hashed and compared.
pub trait DictKey: Clone {
    type Stored: Hash + Eq + Clone;
    fn stored(&self) -> Self::Stored;
}

impl DictKey for Str {
    type Stored = Str;
    fn stored(&self) -> Str {
        self.clone()
    }
}

impl DictKey for bool {
    type Stored = bool;
    fn stored(&self) -> bool {
        *self
    }
}

impl DictKey for f64 {
    /// SameValueZero: one key for every NaN and for both zeros.
    type Stored = u64;
    fn stored(&self) -> u64 {
        if self.is_nan() {
            f64::NAN.to_bits()
        } else if *self == 0.0 {
            0
        } else {
            self.to_bits()
        }
    }
}

struct Data<K: DictKey, V> {
    entries: Vec<(K, V)>,
    index: HashMap<K::Stored, usize>,
}

impl<K: DictKey, V: Clone> Clone for Data<K, V> {
    fn clone(&self) -> Self {
        Data {
            entries: self.entries.clone(),
            index: self.index.clone(),
        }
    }
}

/// An insertion-ordered map with copy-on-write storage.
pub struct Dict<K: DictKey, V>(Rc<Data<K, V>>);

impl<K: DictKey, V> Clone for Dict<K, V> {
    fn clone(&self) -> Self {
        Dict(Rc::clone(&self.0))
    }
}

impl<K: DictKey, V: Clone> Default for Dict<K, V> {
    fn default() -> Self {
        Dict::new()
    }
}

impl<K: DictKey, V: Clone> Dict<K, V> {
    pub fn new() -> Self {
        Dict(Rc::new(Data {
            entries: Vec::new(),
            index: HashMap::new(),
        }))
    }

    /// `{ a: 1, b: 2 }` / `new Map([[k, v], …])`: a repeated key keeps its
    /// first position and its last value.
    pub fn from_entries(entries: impl IntoIterator<Item = (K, V)>) -> Self {
        let mut dict = Dict::new();
        for (key, value) in entries {
            dict.insert(key, value);
        }
        dict
    }

    fn unique(&mut self) -> &mut Data<K, V> {
        Rc::make_mut(&mut self.0)
    }

    pub fn len(&self) -> usize {
        self.0.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.entries.is_empty()
    }

    /// `map.get(k)` / `record[k]`: absent is `None` (`undefined`).
    pub fn get(&self, key: &K) -> Option<V> {
        let index = *self.0.index.get(&key.stored())?;
        Some(self.0.entries[index].1.clone())
    }

    /// `map.has(k)` / `k in record`.
    pub fn contains(&self, key: &K) -> bool {
        self.0.index.contains_key(&key.stored())
    }

    /// `map.set(k, v)` / `record[k] = v`.
    pub fn insert(&mut self, key: K, value: V) {
        let stored = key.stored();
        let data = self.unique();
        match data.index.get(&stored) {
            Some(&index) => data.entries[index].1 = value,
            None => {
                data.index.insert(stored, data.entries.len());
                data.entries.push((key, value));
            }
        }
    }

    /// `map.delete(k)` / `delete record[k]`: whether it was there.
    pub fn remove(&mut self, key: &K) -> bool {
        let stored = key.stored();
        if !self.0.index.contains_key(&stored) {
            return false;
        }
        let data = self.unique();
        let index = data.index.remove(&stored).expect("present");
        data.entries.remove(index);
        for position in data.index.values_mut() {
            if *position > index {
                *position -= 1;
            }
        }
        true
    }

    pub fn clear(&mut self) {
        let data = self.unique();
        data.entries.clear();
        data.index.clear();
    }

    pub fn keys(&self) -> List<K> {
        self.0.entries.iter().map(|(key, _)| key.clone()).collect()
    }

    pub fn values(&self) -> List<V> {
        self.0
            .entries
            .iter()
            .map(|(_, value)| value.clone())
            .collect()
    }

    pub fn entries(&self) -> List<(K, V)> {
        self.0.entries.iter().cloned().collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &(K, V)> {
        self.0.entries.iter()
    }
}

impl<K: DictKey + PartialEq, V: PartialEq> PartialEq for Dict<K, V> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0) || self.0.entries == other.0.entries
    }
}

impl<K: DictKey + std::fmt::Debug, V: std::fmt::Debug> std::fmt::Debug for Dict<K, V> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_map()
            .entries(self.0.entries.iter().map(|(key, value)| (key, value)))
            .finish()
    }
}

/// `Set<T>`: insertion-ordered unique values.
pub struct Set<T: DictKey>(Dict<T, ()>);

impl<T: DictKey> Clone for Set<T> {
    fn clone(&self) -> Self {
        Set(self.0.clone())
    }
}

impl<T: DictKey> Default for Set<T> {
    fn default() -> Self {
        Set(Dict::new())
    }
}

impl<T: DictKey> Set<T> {
    pub fn new() -> Self {
        Set::default()
    }

    pub fn from_values(values: impl IntoIterator<Item = T>) -> Self {
        Set(Dict::from_entries(
            values.into_iter().map(|value| (value, ())),
        ))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn contains(&self, value: &T) -> bool {
        self.0.contains(value)
    }

    pub fn insert(&mut self, value: T) {
        self.0.insert(value, ());
    }

    pub fn remove(&mut self, value: &T) -> bool {
        self.0.remove(value)
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn values(&self) -> List<T> {
        self.0.keys()
    }
}

impl<T: DictKey + PartialEq> PartialEq for Set<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<K: DictKey + super::hot::Hot, V: Clone + super::hot::Hot> super::hot::Hot for Dict<K, V> {
    fn save(&self) -> super::hot::Value {
        super::hot::Value::Array(
            self.iter()
                .map(|(key, value)| super::hot::Value::Array(vec![key.save(), value.save()]))
                .collect(),
        )
    }
    fn load(value: &super::hot::Value) -> Option<Self> {
        let mut entries = Vec::new();
        for pair in value.as_array()? {
            let pair = pair.as_array()?;
            entries.push((K::load(pair.first()?)?, V::load(pair.get(1)?)?));
        }
        Some(Dict::from_entries(entries))
    }
}

impl<T: DictKey + super::hot::Hot> super::hot::Hot for Set<T> {
    fn save(&self) -> super::hot::Value {
        super::hot::Value::Array(self.values().iter().map(super::hot::Hot::save).collect())
    }
    fn load(value: &super::hot::Value) -> Option<Self> {
        let values: Option<Vec<T>> = value.as_array()?.iter().map(T::load).collect();
        Some(Set::from_values(values?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insertion_order_overwrites_and_deletes_like_javascript_maps() {
        let mut map: Dict<Str, f64> = Dict::from_entries([
            (Str::from("b"), 1.0),
            (Str::from("a"), 2.0),
            (Str::from("b"), 3.0),
        ]);
        assert_eq!(map.keys().as_slice(), &[Str::from("b"), Str::from("a")]);
        assert_eq!(map.get(&Str::from("b")), Some(3.0));
        let snapshot = map.clone();
        assert!(map.remove(&Str::from("b")));
        map.insert(Str::from("b"), 4.0);
        assert_eq!(map.keys().as_slice(), &[Str::from("a"), Str::from("b")]);
        // copy-on-write: the earlier value is unchanged
        assert_eq!(snapshot.get(&Str::from("b")), Some(3.0));
        assert!(!map.remove(&Str::from("zzz")));
    }

    #[test]
    fn number_keys_use_same_value_zero() {
        let mut map: Dict<f64, &str> = Dict::new();
        map.insert(f64::NAN, "nan");
        map.insert(-0.0, "zero");
        assert_eq!(map.get(&f64::NAN), Some("nan"));
        assert_eq!(map.get(&0.0), Some("zero"));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn sets_keep_unique_values_in_order() {
        let mut set = Set::from_values([3.0, 1.0, 3.0]);
        set.insert(2.0);
        assert_eq!(set.values().as_slice(), &[3.0, 1.0, 2.0]);
        assert!(set.contains(&1.0) && !set.contains(&9.0));
    }
}
