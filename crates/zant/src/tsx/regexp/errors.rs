use super::*;
use crate::tsx::Error;

pub(super) fn error(name: &'static str, message: String) -> Thrown {
    Error::new(name, Str::from(message)).into()
}
pub(super) fn escape_source(source: &str) -> Str {
    if source.is_empty() {
        return Str::Static("(?:)");
    }
    let mut out = String::new();
    let mut escaped = false;
    for c in source.chars() {
        // A legacy escaped line terminator needs only one backslash in source.
        if escaped && matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
            out.pop();
        }
        match c {
            '/' if !escaped => out.push_str("\\/"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            _ => out.push(c),
        }
        escaped = c == '\\' && !escaped;
    }
    Str::from(out)
}
