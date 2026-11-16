use super::{JsonValue, Str};
use crate::tsx::builtins_format::js_float_string;

/// JS JSON.stringify output. The string gap is truncated to ten UTF-16 units, as in JS.
/// A split surrogate at the gap boundary becomes U+FFFD, like parsed strings.
/// Numeric indentation is supplied as spaces by the compiler (maximum ten).
/// Array-index object keys are emitted first in numeric order, as in JS.
pub fn stringify(value: &JsonValue, indent: Option<&str>) -> Str {
    let units: Vec<u16> = indent.unwrap_or("").encode_utf16().take(10).collect();
    let gap = String::from_utf16_lossy(&units);
    let mut out = String::new();
    enum Task<'a> {
        Value(&'a JsonValue, usize),
        Key(&'a str),
        Text(&'static str),
        Line(usize),
    }
    let mut tasks = vec![Task::Value(value, 0)];
    while let Some(task) = tasks.pop() {
        match task {
            Task::Text(text) => out.push_str(text),
            Task::Key(key) => quote(key, &mut out),
            Task::Line(depth) => {
                if !gap.is_empty() {
                    out.push('\n');
                    for _ in 0..depth {
                        out.push_str(&gap);
                    }
                }
            }
            Task::Value(value, depth) => match value {
                JsonValue::Null => out.push_str("null"),
                JsonValue::Bool(v) => out.push_str(if *v { "true" } else { "false" }),
                JsonValue::Number(v) => {
                    if v.is_finite() {
                        out.push_str(&js_float_string(*v));
                    } else {
                        out.push_str("null");
                    }
                }
                JsonValue::String(v) => quote(v, &mut out),
                JsonValue::Array(items) => {
                    out.push('[');
                    tasks.push(Task::Text("]"));
                    if !items.is_empty() {
                        tasks.push(Task::Line(depth));
                    }
                    for (i, item) in items.iter().enumerate().rev() {
                        tasks.push(Task::Value(item, depth + 1));
                        tasks.push(Task::Line(depth + 1));
                        if i > 0 {
                            tasks.push(Task::Text(","));
                        }
                    }
                }
                JsonValue::Object(fields) => {
                    out.push('{');
                    tasks.push(Task::Text("}"));
                    if !fields.is_empty() {
                        tasks.push(Task::Line(depth));
                    }
                    let mut ordered: Vec<_> = fields.iter().collect();
                    ordered.sort_by_key(|(key, _)| array_index(key).map_or(u64::MAX, u64::from));
                    for (i, (key, value)) in ordered.into_iter().enumerate().rev() {
                        tasks.push(Task::Value(value, depth + 1));
                        tasks.push(Task::Text(if gap.is_empty() { ":" } else { ": " }));
                        tasks.push(Task::Key(key));
                        tasks.push(Task::Line(depth + 1));
                        if i > 0 {
                            tasks.push(Task::Text(","));
                        }
                    }
                }
            },
        }
    }
    Str::from(out)
}
fn array_index(key: &str) -> Option<u32> {
    let index: u32 = key.parse().ok()?;
    (index != u32::MAX && index.to_string() == key).then_some(index)
}

pub(super) fn quote(text: &str, out: &mut String) {
    const HEX: &[u8] = b"0123456789abcdef";
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0'..='\u{1f}' => {
                out.push_str("\\u00");
                out.push(HEX[c as usize >> 4] as char);
                out.push(HEX[c as usize & 15] as char);
            }
            _ => out.push(c),
        }
    }
    out.push('"');
}
