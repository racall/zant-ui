use super::*;
use crate::tsx::Error;

pub(super) fn error(name: &'static str, message: String) -> Thrown {
    Error::new(name, Str::from(message)).into()
}
pub(super) fn flags(flags: &str) -> Thrown {
    error(
        "SyntaxError",
        format!("Invalid flags supplied to RegExp constructor '{flags}'"),
    )
}
pub(super) fn pattern(source: &str, flags: &str, detail: &str) -> Thrown {
    let reason = match detail {
        "Unbalanced parenthesis" => {
            if unmatched_close(source) {
                "Unmatched ')'"
            } else {
                "Unterminated group"
            }
        }
        "Unbalanced bracket" | "Unbalanced class set bracket" => "Unterminated character class",
        "Range values reversed, start char code is greater than end char code." => {
            "Range out of order in character class"
        }
        "Invalid character range" => "Invalid character class",
        "Quantifier not allowed here" => "Nothing to repeat",
        "Invalid atom character" if source.contains(['*', '+', '?']) => "Nothing to repeat",
        "Invalid atom character" => "Lone quantifier brackets",
        "Invalid quantifier" if reversed_quantifier(source) => {
            "numbers out of order in {} quantifier"
        }
        "Invalid quantifier" => "Incomplete quantifier",
        "Invalid braced quantifier" => "Incomplete quantifier",
        "Incomplete escape" | "Unterminated escape" => "\\ at end of pattern",
        "Invalid group modifier" => "Invalid group",
        "Invalid token at named capture group identifier" => {
            if source.contains("(?<") {
                "Invalid capture group name"
            } else {
                "Invalid group"
            }
        }
        "Invalid named backreference syntax" => "Invalid named reference",
        "Invalid character escape" => "Invalid escape",
        "Invalid unicode escape" => "Invalid Unicode escape",
        "Invalid character at property escape start" => "Invalid property name",
        _ if detail.starts_with("Backreference to invalid named capture group:") => {
            "Invalid named capture referenced"
        }
        // Engine-specific resource limits and uncommon diagnostics remain explicit.
        other => other,
    };
    error(
        "SyntaxError",
        format!("Invalid regular expression: /{source}/{flags}: {reason}"),
    )
}
fn reversed_quantifier(source: &str) -> bool {
    source.split('{').skip(1).any(|tail| {
        let Some((body, _)) = tail.split_once('}') else {
            return false;
        };
        let Some((min, max)) = body.split_once(',') else {
            return false;
        };
        matches!((min.parse::<u64>(), max.parse::<u64>()), (Ok(a), Ok(b)) if a > b)
    })
}
fn unmatched_close(source: &str) -> bool {
    let mut depth = 0;
    let mut escaped = false;
    let mut class = false;
    for c in source.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '[' => class = true,
            ']' => class = false,
            '(' if !class => depth += 1,
            ')' if !class => {
                if depth == 0 {
                    return true;
                }
                depth -= 1;
            }
            _ => (),
        }
    }
    false
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
