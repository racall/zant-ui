//! JSON runtime for ahead-of-time compiled TSX, without a JavaScript engine.
//!
//! Objects retain insertion order. Parsing replaces duplicate keys in place.
//! Since `Str` is UTF-8, isolated UTF-16 surrogates become U+FFFD; valid pairs
//! are preserved. This is the deliberate difference from JS UTF-16 strings.
mod convert;
mod parser;
#[cfg(test)]
mod tests;
mod writer;

use super::error::{Error, Thrown};
use crate::Str;
pub use parser::parse;
pub use writer::stringify;

#[derive(Clone, Debug, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(Str),
    Array(Vec<JsonValue>),
    Object(Vec<(Str, JsonValue)>),
}

impl JsonValue {
    fn kind(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "a boolean",
            Self::Number(_) => "a number",
            Self::String(_) => "a string",
            Self::Array(_) => "an array",
            Self::Object(_) => "an object",
        }
    }
}

/// Conversion contract used by generated types. `None` means JS `undefined`.
/// Nullable `Option<T>` uses JSON null; optional struct fields use `optional`.
pub trait Json: Sized {
    fn to_json(&self) -> Option<JsonValue>;
    fn from_json(value: &JsonValue, path: &mut JsonPath) -> Result<Self, String>;
    /// Missing required fields fail; nullable types override this to return None.
    fn missing(path: &mut JsonPath) -> Result<Self, String> {
        Err(format!("missing field at {path}"))
    }
}

/// Mutable decoding path. Scoped methods restore the path even on failure.
#[derive(Clone, Debug)]
pub struct JsonPath(String);
impl Default for JsonPath {
    fn default() -> Self {
        Self("$".into())
    }
}
impl std::fmt::Display for JsonPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl JsonPath {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn field<T>(&mut self, name: &str, read: impl FnOnce(&mut Self) -> T) -> T {
        let len = self.0.len();
        if !name.is_empty()
            && name.chars().enumerate().all(|(i, c)| {
                c == '_' || c == '$' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
            })
        {
            self.0.push('.');
            self.0.push_str(name);
        } else {
            self.0.push('[');
            self.0
                .push_str(&serde_json::to_string(name).expect("a string is valid JSON"));
            self.0.push(']');
        }
        let result = read(self);
        self.0.truncate(len);
        result
    }
    pub fn index<T>(&mut self, index: usize, read: impl FnOnce(&mut Self) -> T) -> T {
        let len = self.0.len();
        use std::fmt::Write;
        write!(self.0, "[{index}]").expect("writing to String");
        let result = read(self);
        self.0.truncate(len);
        result
    }
    pub fn expected(&self, expected: &str, value: &JsonValue) -> String {
        format!("expected {expected} at {self}, found {}", value.kind())
    }
}

/// Builder for generated struct serializers; repeated fields replace in place.
#[derive(Default)]
pub struct ObjectBuilder(Vec<(Str, JsonValue)>);
/// Start a generated struct serializer.
pub fn object_builder() -> ObjectBuilder {
    ObjectBuilder::default()
}
impl ObjectBuilder {
    pub fn field(mut self, name: &'static str, value: &impl Json) -> Self {
        if let Some(value) = value.to_json() {
            if let Some((_, old)) = self.0.iter_mut().find(|(key, _)| key.as_str() == name) {
                *old = value;
            } else {
                self.0.push((Str::Static(name), value));
            }
        }
        self
    }
    /// TS `x?: T`: absent values are omitted, unlike nullable regular fields.
    pub fn optional<T: Json>(self, name: &'static str, value: &Option<T>) -> Self {
        match value {
            Some(value) => self.field(name, value),
            None => self,
        }
    }
    pub fn build(self) -> JsonValue {
        JsonValue::Object(self.0)
    }
}

/// Read a required or nullable field, extending the path for diagnostics.
pub fn read_field<T: Json>(
    object: &JsonValue,
    name: &str,
    path: &mut JsonPath,
) -> Result<T, String> {
    let JsonValue::Object(fields) = object else {
        return Err(path.expected("an object", object));
    };
    path.field(name, |path| {
        match fields.iter().rev().find(|(key, _)| key.as_str() == name) {
            Some((_, value)) => T::from_json(value, path),
            None => T::missing(path),
        }
    })
}

/// Missing and null optional properties both decode as None.
pub fn read_optional<T: Json>(
    object: &JsonValue,
    name: &str,
    path: &mut JsonPath,
) -> Result<Option<T>, String> {
    read_field(object, name, path)
}

/// Top-level undefined returns None, never the string "undefined".
/// Pass numeric JS indentation as a clamped run of spaces (e.g. Some("  ")).
pub fn stringify_value<T: Json>(value: &T, indent: Option<&str>) -> Option<Str> {
    value.to_json().map(|value| stringify(&value, indent))
}
/// Parse and decode; grammar failures are SyntaxError, shape failures TypeError.
pub fn parse_as<T: Json>(text: &str) -> Result<T, Thrown> {
    let value = parse(text)?;
    T::from_json(&value, &mut JsonPath::new()).map_err(|detail| {
        Thrown::error(Error::new(
            "TypeError",
            Str::from(format!("JSON does not match the expected type: {detail}")),
        ))
    })
}
