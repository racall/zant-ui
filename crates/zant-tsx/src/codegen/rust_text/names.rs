//! Rust identifiers, string literals and enum variants for TSX names and strings.

use super::*;

/// Rust spelling of a source identifier.
///
/// JS identifiers may contain `$`, which is not a Rust identifier character;
/// such bytes become `_`. Rust keywords get a `_` suffix (`type` → `type_`);
/// `self`, `Self`, `super` and `crate` cannot be raw identifiers, so they take
/// a suffix too. Returns `None` for names that are not valid UTF-8.
pub fn id(name: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(name).ok()?;
    let mut out = String::with_capacity(text.len() + 1);
    for (index, ch) in text.chars().enumerate() {
        let valid = ch == '_' || ch.is_alphanumeric();
        if valid && !(index == 0 && ch.is_numeric()) {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    // `_` is not an identifier: "" is `Empty`, symbol-only members spell
    // their bytes ("-" → `Sym2d`)
    if out.is_empty() {
        if text.is_empty() {
            out.push_str("Empty");
        } else {
            out.push_str("Sym");
            for byte in text.bytes() {
                out.push_str(&format!("{byte:02x}"));
            }
        }
    }
    let is_keyword = matches!(
        out.as_str(),
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "try"
            // edition 2024 reserved word
            | "gen"
            // Rust 2021 reserved-for-future keywords (a valid TSX name such as
            // `box` would make the generated item unparsable)
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
    );
    // the Rust prelude's names: a TSX `type Result = …` must not shadow the
    // `Result` / `Option` / `Ok` / … the generated code itself uses
    let is_prelude = matches!(
        out.as_str(),
        "Result"
            | "Option"
            | "Some"
            | "None"
            | "Ok"
            | "Err"
            | "Vec"
            | "String"
            | "Box"
            | "Default"
            | "Clone"
            | "Copy"
            | "Drop"
            | "Fn"
            | "FnMut"
            | "FnOnce"
            | "Iterator"
            | "IntoIterator"
            | "PartialEq"
            | "Eq"
            | "PartialOrd"
            | "Ord"
            | "Send"
            | "Sync"
            | "Sized"
            | "ToString"
            | "ToOwned"
            | "From"
            | "Into"
            | "AsRef"
            | "AsMut"
            | "Extend"
    );
    if is_keyword || is_prelude {
        out.push('_');
    }
    Some(out)
}

/// [`id`] at a diagnostic location.
pub fn id_at(g: &mut Generator<'_>, name: &[u8], loc: ast::Loc) -> Result<String> {
    match id(name) {
        Some(value) => Ok(value),
        None => fail(g, loc, "identifier is not valid UTF-8"),
    }
}

/// A Rust string literal from TSX bytes. Returns `None` when the bytes are
/// not UTF-8 (WTF-8 lone surrogates have no Rust `str` representation).
pub fn quote(s: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(s).ok()?;
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 || ch as u32 == 0x7f => {
                out.push_str(&format!("\\u{{{:x}}}", ch as u32));
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    Some(out)
}

/// [`quote`] at a diagnostic location.
pub fn quote_at(g: &mut Generator<'_>, s: &[u8], loc: ast::Loc) -> Result<String> {
    match quote(s) {
        Some(value) => Ok(value),
        None => fail(
            g,
            loc,
            "string literal is not valid UTF-8; Rust cannot represent an unpaired surrogate",
        ),
    }
}

/// PascalCase Rust enum variant for a string-literal union member
/// (`"in-progress"` → `InProgress`).
pub fn enum_variant(member: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(member).ok()?;
    let mut out = String::with_capacity(text.len());
    let mut upper = true;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            if upper {
                for upper_ch in ch.to_uppercase() {
                    out.push(upper_ch);
                }
                upper = false;
            } else {
                out.push(ch);
            }
        } else {
            upper = true;
        }
    }
    // `_` is not an identifier: "" is `Empty`, symbol-only members spell
    // their bytes ("-" → `Sym2d`)
    if out.is_empty() {
        if text.is_empty() {
            out.push_str("Empty");
        } else {
            out.push_str("Sym");
            for byte in text.bytes() {
                out.push_str(&format!("{byte:02x}"));
            }
        }
    }
    if out.chars().next().is_some_and(|ch| ch.is_numeric()) {
        out.insert(0, '_');
    }
    if out == "Self" || out == "Self_" {
        out = "Self_".to_owned();
    }
    Some(out)
}

pub fn eq(a: &[u8], b: &[u8]) -> bool {
    a == b
}
