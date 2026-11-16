use super::*;
use crate::tsx::{Any, Bytes, Dict, DictKey, List, Set, builtins};
use std::{cell::RefCell, rc::Rc};

impl Json for JsonValue {
    fn to_json(&self) -> Option<JsonValue> {
        Some(self.clone())
    }
    fn from_json(value: &JsonValue, _: &mut JsonPath) -> Result<Self, String> {
        Ok(value.clone())
    }
}
macro_rules! scalar {
    ($ty:ty, $variant:ident, $expected:literal) => {
        impl Json for $ty {
            fn to_json(&self) -> Option<JsonValue> {
                Some(JsonValue::$variant(self.clone()))
            }
            fn from_json(value: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
                if let JsonValue::$variant(v) = value {
                    Ok(v.clone())
                } else {
                    Err(path.expected($expected, value))
                }
            }
        }
    };
}
scalar!(f64, Number, "a number");
scalar!(bool, Bool, "a boolean");
scalar!(Str, String, "a string");
impl Json for f32 {
    fn to_json(&self) -> Option<JsonValue> {
        (*self as f64).to_json()
    }
    fn from_json(value: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
        let v = f64::from_json(value, path)?;
        let narrowed = v as f32;
        if v.is_finite() && !narrowed.is_finite() {
            Err(path.expected("a number in f32 range", value))
        } else {
            Ok(narrowed)
        }
    }
}
macro_rules! integer {
    ($($ty:ty),*) => {$(impl Json for $ty {
        fn to_json(&self) -> Option<JsonValue> { Some(JsonValue::Number(*self as f64)) }
        fn from_json(value: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
            let v = f64::from_json(value, path)?;
            // Upper bounds are exclusive powers of two, avoiding rounded MAX
            // values (e.g. u64::MAX rounds up to 2^64 in binary64).
            let upper = if <$ty>::MIN == 0 { 2f64.powi(<$ty>::BITS as i32) } else { 2f64.powi(<$ty>::BITS as i32 - 1) };
            if v.is_finite() && v.fract() == 0.0 && v >= <$ty>::MIN as f64 && v < upper {
                Ok(v as $ty)
            } else { Err(path.expected(concat!("an integer in ", stringify!($ty), " range"), value)) }
        }
    })*};
}
integer!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
);
impl Json for String {
    fn to_json(&self) -> Option<JsonValue> {
        Some(JsonValue::String(Str::copy(self)))
    }
    fn from_json(value: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
        Str::from_json(value, path).map(|s| s.to_string())
    }
}
impl<T: Json> Json for Option<T> {
    fn to_json(&self) -> Option<JsonValue> {
        self.as_ref().map_or(Some(JsonValue::Null), Json::to_json)
    }
    fn from_json(value: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
        if matches!(value, JsonValue::Null) {
            Ok(None)
        } else {
            T::from_json(value, path).map(Some)
        }
    }
    fn missing(_: &mut JsonPath) -> Result<Self, String> {
        Ok(None)
    }
}
fn array<T: Json>(items: &[T]) -> JsonValue {
    JsonValue::Array(
        items
            .iter()
            .map(|v| v.to_json().unwrap_or(JsonValue::Null))
            .collect(),
    )
}
impl<T: Json> Json for Vec<T> {
    fn to_json(&self) -> Option<JsonValue> {
        Some(array(self))
    }
    fn from_json(value: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
        let JsonValue::Array(items) = value else {
            return Err(path.expected("an array", value));
        };
        items
            .iter()
            .enumerate()
            .map(|(i, v)| path.index(i, |path| T::from_json(v, path)))
            .collect()
    }
}
impl<T: Json> Json for List<T> {
    fn to_json(&self) -> Option<JsonValue> {
        Some(array(self))
    }
    fn from_json(value: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
        Vec::from_json(value, path).map(List::from_vec)
    }
}
macro_rules! dict {
    ($key:ty, $encode:expr, $decode:expr) => {
        impl<V: Json + Clone> Json for Dict<$key, V> {
            fn to_json(&self) -> Option<JsonValue> {
                // Record entries always remain present; undefined becomes null.
                Some(JsonValue::Object(
                    self.iter()
                        .map(|(k, v)| (($encode)(k), v.to_json().unwrap_or(JsonValue::Null)))
                        .collect(),
                ))
            }
            fn from_json(value: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
                let JsonValue::Object(items) = value else {
                    return Err(path.expected("an object", value));
                };
                let mut out = Self::new();
                for (key, value) in items {
                    let v = path.field(key, |path| V::from_json(value, path))?;
                    out.insert(($decode)(key), v);
                }
                Ok(out)
            }
        }
    };
}
dict!(Str, |key: &Str| key.clone(), |key: &Str| key.clone());
dict!(
    f64,
    |key: &f64| crate::tsx::builtins_format::js_float_string(*key),
    |key: &Str| builtins::number(key.as_str())
);
impl<T: DictKey> Json for Set<T> {
    fn to_json(&self) -> Option<JsonValue> {
        Some(JsonValue::Object(vec![]))
    }
    fn from_json(_: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
        Err(format!("a Set cannot be read from JSON at {path}"))
    }
}
macro_rules! tuple {
    ($(($len:literal; $($name:ident $index:tt),+)),*) => {$(
        impl<$($name: Json),+> Json for ($($name,)+) {
            fn to_json(&self) -> Option<JsonValue> { Some(JsonValue::Array(vec![$(self.$index.to_json().unwrap_or(JsonValue::Null)),+])) }
            fn from_json(value: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
                let JsonValue::Array(items) = value else { return Err(path.expected("an array", value)); };
                if items.len() != $len { return Err(format!("expected an array of length {} at {path}, found length {}", $len, items.len())); }
                Ok(($(path.index($index, |path| $name::from_json(&items[$index], path))?,)+))
            }
        }
    )*};
}
tuple!((1; A 0), (2; A 0, B 1), (3; A 0, B 1, C 2), (4; A 0, B 1, C 2, D 3), (5; A 0, B 1, C 2, D 3, E 4), (6; A 0, B 1, C 2, D 3, E 4, F 5));
impl<T: Json> Json for Rc<RefCell<T>> {
    fn to_json(&self) -> Option<JsonValue> {
        self.borrow().to_json()
    }
    fn from_json(value: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
        T::from_json(value, path).map(|v| Rc::new(RefCell::new(v)))
    }
}
impl Json for Bytes {
    fn to_json(&self) -> Option<JsonValue> {
        // Uint8Array has enumerable index properties, unlike ordinary arrays.
        Some(JsonValue::Object(
            self.iter()
                .enumerate()
                .map(|(i, v)| (Str::from(i.to_string()), JsonValue::Number(f64::from(*v))))
                .collect(),
        ))
    }
    fn from_json(value: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
        let JsonValue::Object(items) = value else {
            return Err(path.expected("an object", value));
        };
        let mut bytes = Vec::with_capacity(items.len());
        for i in 0..items.len() {
            bytes.push(read_field(value, &i.to_string(), path)?);
        }
        Ok(bytes.into())
    }
}
impl Json for Any {
    fn to_json(&self) -> Option<JsonValue> {
        match self {
            Self::Str(v) => v.to_json(),
            Self::Number(v) => v.to_json(),
            Self::Bool(v) => v.to_json(),
        }
    }
    fn from_json(value: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
        match value {
            JsonValue::String(v) => Ok(Self::Str(v.clone())),
            JsonValue::Number(v) => Ok(Self::Number(*v)),
            JsonValue::Bool(v) => Ok(Self::Bool(*v)),
            _ => Err(path.expected("a string, number or boolean", value)),
        }
    }
}
impl Json for Error {
    fn to_json(&self) -> Option<JsonValue> {
        Some(JsonValue::Object(vec![]))
    }
    fn from_json(_: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
        Err(format!("an Error cannot be read from JSON at {path}"))
    }
}
impl Json for Thrown {
    fn to_json(&self) -> Option<JsonValue> {
        match self {
            Self::Error(e) => e.to_json(),
            Self::Null => Some(JsonValue::Null),
            Self::Boolean(v) => v.to_json(),
            Self::Number(v) => v.to_json(),
            Self::String(v) => v.to_json(),
        }
    }
    fn from_json(value: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
        match value {
            JsonValue::Null => Ok(Self::Null),
            JsonValue::Bool(v) => Ok(Self::Boolean(*v)),
            JsonValue::Number(v) => Ok(Self::Number(*v)),
            JsonValue::String(v) => Ok(Self::String(v.clone())),
            _ => Err(path.expected("a thrown primitive", value)),
        }
    }
}
