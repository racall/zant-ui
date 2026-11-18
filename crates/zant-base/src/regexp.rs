//! RegExp validation with JavaScript's `SyntaxError` messages: the runtime's
//! `RegExp` constructor and the TSX compiler (which checks literals when it
//! compiles them) share it, so both report V8's wording.

/// Compiles `source` with `flags` (`dgimsuvy`, each at most once, not both
/// `u` and `v`) the way a JavaScript `RegExp` reads them: full Unicode with
/// `u` / `v`, UTF-16 code units otherwise. `Err` is the `SyntaxError`
/// message.
pub fn compile(source: &str, flags: &str) -> Result<regress::Regex, String> {
    let mut seen = String::new();
    for c in flags.chars() {
        if !"dgimsuvy".contains(c) || seen.contains(c) {
            return Err(flags_message(flags));
        }
        seen.push(c);
    }
    if seen.contains('u') && seen.contains('v') {
        return Err(flags_message(flags));
    }
    let full_unicode = flags.contains('u') || flags.contains('v');
    let engine_flags = regress::Flags {
        unicode: full_unicode,
        ..flags.into()
    };
    // Legacy patterns operate on code units, including literal astral characters.
    let pattern: Vec<u32> = if full_unicode {
        source.chars().map(u32::from).collect()
    } else {
        source.encode_utf16().map(u32::from).collect()
    };
    regress::Regex::from_unicode(pattern.into_iter(), engine_flags)
        .map_err(|e| pattern_message(source, flags, &e.to_string()))
}

fn flags_message(flags: &str) -> String {
    format!("Invalid flags supplied to RegExp constructor '{flags}'")
}

fn pattern_message(source: &str, flags: &str, detail: &str) -> String {
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
    format!("Invalid regular expression: /{source}/{flags}: {reason}")
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

#[cfg(test)]
mod tests {
    use super::compile;

    #[test]
    fn errors_use_the_javascript_wording() {
        assert!(compile("a+", "gi").is_ok());
        assert_eq!(
            compile("a", "gg").unwrap_err(),
            "Invalid flags supplied to RegExp constructor 'gg'"
        );
        assert_eq!(
            compile("a", "uv").unwrap_err(),
            "Invalid flags supplied to RegExp constructor 'uv'"
        );
        assert_eq!(
            compile("(a", "").unwrap_err(),
            "Invalid regular expression: /(a/: Unterminated group"
        );
        assert_eq!(
            compile("a)", "").unwrap_err(),
            "Invalid regular expression: /a)/: Unmatched ')'"
        );
        assert_eq!(
            compile("a{3,1}", "").unwrap_err(),
            "Invalid regular expression: /a{3,1}/: numbers out of order in {} quantifier"
        );
    }
}
