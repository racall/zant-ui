//! Name-based, fallible state snapshots for hot reload.
pub use serde_json::Value;
use std::{cell::RefCell, rc::Rc};

/// A state value that can survive a code replacement.
pub trait Hot: Sized {
    fn save(&self) -> Value;
    fn load(value: &Value) -> Option<Self>;
}
macro_rules! scalar {
    ($($ty:ty),*) => {$(impl Hot for $ty {
        fn save(&self) -> Value { serde_json::to_value(self).unwrap_or(Value::Null) }
        fn load(value: &Value) -> Option<Self> { serde_json::from_value(value.clone()).ok() }
    })*};
}
scalar!(
    i8, i16, i32, i64, isize, u8, u16, u32, u64, usize, bool, String
);
// JSON numbers cannot represent the entire 128-bit integer range.
macro_rules! wide {
    ($($ty:ty),*) => {$(impl Hot for $ty {
        fn save(&self) -> Value { serde_json::json!({stringify!($ty): self.to_string()}) }
        fn load(value: &Value) -> Option<Self> {
            serde_json::from_value(value.clone()).ok().or_else(|| value.get(stringify!($ty))?.as_str()?.parse().ok())
        }
    })*};
}
wide!(i128, u128);
impl Hot for f64 {
    fn save(&self) -> Value {
        Value::from(*self)
    }
    fn load(value: &Value) -> Option<Self> {
        value.as_f64().filter(|v| v.is_finite())
    }
}
impl Hot for f32 {
    fn save(&self) -> Value {
        Value::from(f64::from(*self))
    }
    fn load(value: &Value) -> Option<Self> {
        let v = f64::load(value)? as f32;
        v.is_finite().then_some(v)
    }
}
impl Hot for crate::Str {
    fn save(&self) -> Value {
        Value::from(self.as_str())
    }
    fn load(value: &Value) -> Option<Self> {
        Some(Self::copy(value.as_str()?))
    }
}
impl<T: Hot> Hot for Option<T> {
    fn save(&self) -> Value {
        self.as_ref().map_or(Value::Null, Hot::save)
    }
    fn load(value: &Value) -> Option<Self> {
        if value.is_null() {
            Some(None)
        } else {
            Some(Some(T::load(value)?))
        }
    }
}
impl<T: Hot> Hot for Vec<T> {
    fn save(&self) -> Value {
        Value::Array(self.iter().map(Hot::save).collect())
    }
    fn load(value: &Value) -> Option<Self> {
        value.as_array()?.iter().map(T::load).collect()
    }
}
impl<T: Hot> Hot for super::List<T> {
    fn save(&self) -> Value {
        Value::Array(self.iter().map(Hot::save).collect())
    }
    fn load(value: &Value) -> Option<Self> {
        Some(Self::from_vec(Vec::<T>::load(value)?))
    }
}
impl<T: Hot> Hot for Rc<RefCell<T>> {
    fn save(&self) -> Value {
        self.try_borrow().map_or(Value::Null, |v| v.save())
    }
    fn load(value: &Value) -> Option<Self> {
        Some(Rc::new(RefCell::new(T::load(value)?)))
    }
}
impl Hot for super::Bytes {
    fn save(&self) -> Value {
        self.to_vec().save()
    }
    fn load(value: &Value) -> Option<Self> {
        Some(Vec::<u8>::load(value)?.into())
    }
}
impl Hot for super::Any {
    fn save(&self) -> Value {
        match self {
            Self::Str(v) => v.save(),
            Self::Number(v) => v.save(),
            Self::Bool(v) => v.save(),
        }
    }
    fn load(value: &Value) -> Option<Self> {
        match value {
            Value::String(_) => Some(Self::Str(crate::Str::load(value)?)),
            Value::Number(_) => Some(Self::Number(f64::load(value)?)),
            Value::Bool(v) => Some(Self::Bool(*v)),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct Snapshot(serde_json::Map<String, Value>);
impl Snapshot {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn put(&mut self, name: &str, value: Value) {
        self.0.insert(name.into(), value);
    }
    pub fn finish(self) -> String {
        Value::Object(self.0).to_string()
    }
    pub fn value(self) -> Value {
        Value::Object(self.0)
    }
}
#[derive(Default)]
pub struct Restore(Value);
impl Restore {
    pub fn parse(state: &str) -> Self {
        Self(serde_json::from_str(state).unwrap_or(Value::Null))
    }
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.0.as_object()?.get(name)
    }
}

#[doc(hidden)]
pub struct Probe<'a, T>(pub &'a T);
#[doc(hidden)]
pub struct DecodeError;
#[doc(hidden)]
pub trait Dispatch<T> {
    fn hot_save(self) -> Option<Value>;
    fn hot_load(self, value: &Value) -> Result<Option<T>, DecodeError>;
}
impl<T: Hot> Dispatch<T> for &Probe<'_, T> {
    fn hot_save(self) -> Option<Value> {
        Some(self.0.save())
    }
    fn hot_load(self, value: &Value) -> Result<Option<T>, DecodeError> {
        T::load(value).map(Some).ok_or(DecodeError)
    }
}
impl<T> Dispatch<T> for &&Probe<'_, T> {
    fn hot_save(self) -> Option<Value> {
        None
    }
    fn hot_load(self, _: &Value) -> Result<Option<T>, DecodeError> {
        Ok(None)
    }
}
/// Saves supported fields; unsupported types are skipped at compile time.
#[macro_export]
macro_rules! hot_put {
    ($snapshot:expr, $name:expr, $field:expr) => {{
        use $crate::tsx::hot::Dispatch as _;
        if let Some(value) = (&$crate::tsx::hot::Probe(&$field)).hot_save() {
            $snapshot.put($name, value);
        }
    }};
}
/// Restores supported fields only when decoding succeeds.
/// Returns false for a supported field with an incompatible value; missing
/// and unsupported fields are successful no-ops.
#[macro_export]
macro_rules! hot_get {
    ($restore:expr, $name:expr, $field:expr) => {{
        use $crate::tsx::hot::Dispatch as _;
        if let Some(value) = $restore.get($name) {
            match (&$crate::tsx::hot::Probe(&$field)).hot_load(value) {
                Ok(Some(decoded)) => {
                    $field = decoded;
                    true
                }
                Ok(None) => true,
                Err(_) => false,
            }
        } else {
            true
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn round_trip<T: Hot + PartialEq + std::fmt::Debug>(value: T) {
        // Exercise the JSON text boundary, including wide integers.
        let encoded: Value = serde_json::from_str(&value.save().to_string()).unwrap();
        assert_eq!(T::load(&encoded), Some(value));
        assert!(T::load(&json!({"foreign": true})).is_none());
    }

    #[test]
    fn all_integer_types_round_trip_and_reject_overflow() {
        macro_rules! check {
            ($($ty:ty),*) => {$(
                round_trip(<$ty>::MIN);
                round_trip(<$ty>::MAX);
                assert!(<$ty>::load(&json!(1.5)).is_none());
                assert!(<$ty>::load(&json!("3")).is_none());
            )*};
        }
        check!(
            i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
        );
        macro_rules! narrow {
            ($($ty:ty),*) => {$(assert!(<$ty>::load(&json!(u64::MAX)).is_none());)*};
        }
        narrow!(i8, i16, i32, i64, isize, u8, u16, u32);
        assert_eq!(i128::load(&json!(-42)), Some(-42));
        assert_eq!(u128::load(&json!(u64::MAX)), Some(u128::from(u64::MAX)));
        assert!(u128::load(&json!(-1)).is_none());
        assert!(u64::load(&json!(-1)).is_none());
        assert!(usize::load(&json!(-1)).is_none());
        assert!(i128::load(&json!({"i128": "170141183460469231731687303715884105728"})).is_none());
        assert!(u128::load(&json!({"u128": "340282366920938463463374607431768211456"})).is_none());
    }

    #[test]
    fn scalars_and_wrappers_round_trip_and_reject_mismatches() {
        round_trip(1.25f32);
        round_trip(1.25f64);
        round_trip(true);
        round_trip(String::from("hello"));
        round_trip(crate::Str::from("世界"));
        round_trip(Some(7u8));
        round_trip(None::<u8>);
        round_trip(vec![7u8, 9]);
        round_trip(super::super::List::from_vec(vec![7u8]));
        round_trip(Rc::new(RefCell::new(7u8)));
        round_trip(super::super::Bytes::from(vec![1u8, 2]));
        round_trip(super::super::Any::from("text"));
        round_trip(super::super::Any::from(2.5));
        round_trip(super::super::Any::from(true));
        assert!(Vec::<u8>::load(&json!([1, "bad"])).is_none());
        assert!(Option::<u8>::load(&json!("bad")).is_none());
        assert!(bool::load(&json!(1)).is_none());
        assert!(String::load(&json!(1)).is_none());
        assert!(f32::load(&json!(1e100)).is_none());
        assert!(f64::load(&Value::Null).is_none());
    }

    #[test]
    fn macros_skip_non_hot_fields_and_preserve_defaults() {
        struct Unsupported;
        let mut unsupported = Unsupported;
        let mut count = 7u32;
        let mut snapshot = Snapshot::new();
        crate::hot_put!(snapshot, "unsupported", unsupported);
        crate::hot_put!(snapshot, "count", count);
        let restore = Restore::parse(&snapshot.finish());
        assert!(restore.get("unsupported").is_none());
        count = 0;
        crate::hot_get!(restore, "count", count);
        crate::hot_get!(restore, "count", unsupported);
        let _ = &unsupported;
        assert_eq!(count, 7);
        let restore = Restore::parse(r#"{"count":"changed type"}"#);
        crate::hot_get!(restore, "count", count);
        assert_eq!(count, 7);
        for text in ["bad json", "[]", "null", "42"] {
            assert!(Restore::parse(text).get("count").is_none());
        }
    }
}
