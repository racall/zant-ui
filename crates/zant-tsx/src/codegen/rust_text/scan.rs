//! Scanning generated Rust text: labels in use, identifier counts and expression paths (strings and comments skipped).

/// Whether `body` contains a `break`/`continue` to `label` outside string
/// literals and comments.
pub fn uses_label(body: &str, label: &str) -> bool {
    let bytes = body.as_bytes();
    let needle = format!("'{label}");
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = skip_string(bytes, index),
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index = bytes[index..]
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .map_or(bytes.len(), |offset| index + offset + 1);
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => index = skip_block_comment(bytes, index),
            b'\'' => {
                if body[index..].starts_with(&needle) {
                    return true;
                }
                index = skip_quote(bytes, index);
            }
            _ => {
                if body[index..].starts_with(&needle) {
                    return true;
                }
                index += 1;
            }
        }
    }
    false
}

pub(super) fn skip_string(bytes: &[u8], mut index: usize) -> usize {
    index += 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return index + 1,
            _ => index += 1,
        }
    }
    bytes.len()
}

pub(super) fn skip_block_comment(bytes: &[u8], mut index: usize) -> usize {
    let mut depth = 0usize;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"/*") {
            depth += 1;
            index += 2;
        } else if bytes[index..].starts_with(b"*/") {
            depth -= 1;
            index += 2;
            if depth == 0 {
                return index;
            }
        } else {
            index += 1;
        }
    }
    bytes.len()
}

pub(super) fn skip_quote(bytes: &[u8], mut index: usize) -> usize {
    index += 1;
    if bytes.get(index) == Some(&b'\\') {
        // a char literal escape: `'\n'`
        index += 2;
        if bytes.get(index) == Some(&b'\'') {
            return index + 1;
        }
        return index;
    }
    if bytes.get(index + 1) == Some(&b'\'') {
        return index + 2;
    }
    // a label or lifetime: `'label`
    while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
        index += 1;
    }
    index
}

/// Count plain identifiers in generated Rust, skipping string and char
/// literals, comments, labels and field names after `.` (`scope::finish`
/// uses the counts to decide which locals are unused).
pub fn count_identifiers(body: &str) -> std::collections::HashMap<String, usize> {
    let bytes = body.as_bytes();
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut index = 0usize;
    let mut after_period = false;
    while index < bytes.len() {
        let byte = bytes[index];
        match byte {
            b'"' => {
                index = skip_string(bytes, index);
                after_period = false;
            }
            b'\'' => {
                index = skip_quote(bytes, index);
                after_period = false;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index = bytes[index..]
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .map_or(bytes.len(), |offset| index + offset + 1);
                after_period = false;
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index = skip_block_comment(bytes, index);
                after_period = false;
            }
            b'.' => {
                after_period = true;
                index += 1;
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = index;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
                {
                    index += 1;
                }
                if !after_period && let Ok(name) = std::str::from_utf8(&bytes[start..index]) {
                    *counts.entry(name.to_owned()).or_insert(0) += 1;
                }
                after_period = false;
            }
            _ => {
                after_period = false;
                index += 1;
            }
        }
    }
    counts
}

/// A type spelling usable as an expression path: generic arguments need the
/// turbofish there (`Box<f64>` → `Box::<f64>`, as in `Box::<f64> { .. }`).
pub fn expression_path(rust: &str) -> String {
    match rust.find('<') {
        Some(at) if !rust[..at].ends_with("::") => format!("{}::{}", &rust[..at], &rust[at..]),
        _ => rust.to_owned(),
    }
}
