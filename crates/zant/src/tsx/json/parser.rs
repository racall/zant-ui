//! `JSON.parse` on `serde_json`: a visitor builds [`JsonValue`] directly, so
//! objects keep their key order without serde_json's `preserve_order`
//! feature (which would change every other `serde_json::Map` in the
//! workspace). A later duplicate key replaces the earlier value in place,
//! like JavaScript. Errors are `SyntaxError`s with serde_json's wording.

use std::collections::HashMap;
use std::fmt;

use serde::Deserialize;
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};

use super::{Error, JsonValue, Str, Thrown};

/// Objects with this many keys look duplicates up in a hash index.
const INDEX_FROM: usize = 16;

/// Parse JSON text (RFC 8259 / ECMA-404) into a [`JsonValue`].
pub fn parse(text: &str) -> Result<JsonValue, Thrown> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value = JsonValue::deserialize(&mut deserializer)
        .and_then(|value| deserializer.end().map(|()| value))
        .map_err(|error| Thrown::error(Error::new("SyntaxError", Str::from(error.to_string()))))?;
    Ok(value)
}

impl<'de> Deserialize<'de> for JsonValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ValueVisitor)
    }
}

struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = JsonValue;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_unit<E>(self) -> Result<JsonValue, E> {
        Ok(JsonValue::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<JsonValue, E> {
        Ok(JsonValue::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<JsonValue, E> {
        Ok(JsonValue::Number(value as f64))
    }

    fn visit_u64<E>(self, value: u64) -> Result<JsonValue, E> {
        Ok(JsonValue::Number(value as f64))
    }

    fn visit_f64<E>(self, value: f64) -> Result<JsonValue, E> {
        Ok(JsonValue::Number(value))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<JsonValue, E> {
        Ok(JsonValue::String(Str::from(value.to_owned())))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<JsonValue, E> {
        Ok(JsonValue::String(Str::from(value)))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<JsonValue, A::Error> {
        let mut items = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(JsonValue::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<JsonValue, A::Error> {
        let mut entries: Vec<(Str, JsonValue)> = Vec::new();
        // key → position, built once an object is large enough that the
        // linear duplicate check would make parsing quadratic
        let mut index: Option<HashMap<Str, usize>> = None;
        while let Some((key, value)) = map.next_entry::<String, JsonValue>()? {
            // `{"a":1,"a":2}`: the last value wins, at the first position
            let known = match &index {
                Some(index) => index.get(key.as_str()).copied(),
                None => entries.iter().position(|(known, _)| known.as_str() == key),
            };
            match known {
                Some(position) => entries[position].1 = value,
                None => {
                    let key = Str::from(key);
                    if let Some(index) = &mut index {
                        index.insert(key.clone(), entries.len());
                    }
                    entries.push((key, value));
                    if index.is_none() && entries.len() >= INDEX_FROM {
                        index = Some(
                            entries
                                .iter()
                                .enumerate()
                                .map(|(position, (key, _))| (key.clone(), position))
                                .collect(),
                        );
                    }
                }
            }
        }
        Ok(JsonValue::Object(entries))
    }
}
