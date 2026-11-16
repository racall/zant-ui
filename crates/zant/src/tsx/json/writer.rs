//! `JSON.stringify` on `serde_json`'s serializer. Two JavaScript rules sit
//! on top: numbers print the way JavaScript prints them (`ryu-js`: `1`,
//! `1e+21`, `1e-7`; NaN and ±Infinity become `null`), and an object's
//! array-index keys come first in numeric order (JavaScript's property
//! order). serde_json already escapes strings like `JSON.stringify`.

use std::io;

use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};
use serde_json::ser::{CompactFormatter, Formatter, PrettyFormatter};

use super::{JsonValue, Str};

/// `JSON.stringify(value, null, indent)`: a string indent is cut to its
/// first ten UTF-16 units; the compiler turns a number into that many
/// spaces (at most ten).
pub fn stringify(value: &JsonValue, indent: Option<&str>) -> Str {
    let units: Vec<u16> = indent.unwrap_or("").encode_utf16().take(10).collect();
    let gap = String::from_utf16_lossy(&units);
    let mut out = Vec::new();
    let written = if gap.is_empty() {
        value.serialize(&mut serde_json::Serializer::with_formatter(
            &mut out,
            JsNumbers(CompactFormatter),
        ))
    } else {
        value.serialize(&mut serde_json::Serializer::with_formatter(
            &mut out,
            JsNumbers(PrettyFormatter::with_indent(gap.as_bytes())),
        ))
    };
    written.expect("writing JSON to memory");
    Str::from(String::from_utf8(out).expect("serde_json writes UTF-8"))
}

impl Serialize for JsonValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            JsonValue::Null => serializer.serialize_unit(),
            JsonValue::Bool(value) => serializer.serialize_bool(*value),
            JsonValue::Number(value) if value.is_finite() => serializer.serialize_f64(*value),
            JsonValue::Number(_) => serializer.serialize_unit(),
            JsonValue::String(value) => serializer.serialize_str(value),
            JsonValue::Array(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            JsonValue::Object(fields) => {
                let mut ordered: Vec<_> = fields.iter().collect();
                // stable: the other keys keep their insertion order
                ordered.sort_by_key(|(key, _)| array_index(key).map_or(u64::MAX, u64::from));
                let mut map = serializer.serialize_map(Some(ordered.len()))?;
                for (key, value) in ordered {
                    map.serialize_entry(key.as_str(), value)?;
                }
                map.end()
            }
        }
    }
}

/// A canonical array index (`"0"`, `"42"`, not `"01"` or `"4294967295"`).
fn array_index(key: &str) -> Option<u32> {
    let index: u32 = key.parse().ok()?;
    (index != u32::MAX && index.to_string() == key).then_some(index)
}

/// serde_json's formatter with JavaScript's number text.
struct JsNumbers<F>(F);

impl<F: Formatter> Formatter for JsNumbers<F> {
    fn write_f64<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        writer.write_all(ryu_js::Buffer::new().format(value).as_bytes())
    }
    fn begin_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.begin_array(writer)
    }
    fn end_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_array(writer)
    }
    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.0.begin_array_value(writer, first)
    }
    fn end_array_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_array_value(writer)
    }
    fn begin_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.begin_object(writer)
    }
    fn end_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_object(writer)
    }
    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.0.begin_object_key(writer, first)
    }
    fn end_object_key<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_object_key(writer)
    }
    fn begin_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.begin_object_value(writer)
    }
    fn end_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_object_value(writer)
    }
}
