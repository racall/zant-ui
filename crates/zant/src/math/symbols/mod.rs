//! TeX command tables (subset of KaTeX's `symbols.js` / `functions`).

mod operators;
mod table;
#[cfg(test)]
mod tests;

pub use self::operators::*;
pub use self::table::*;

/// TeX atom classes (KaTeX `groupTypes`), used for inter-atom spacing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Ord,
    Op,
    Bin,
    Rel,
    Open,
    Close,
    Punct,
    Inner,
}

pub fn lookup(name: &str) -> Option<Symbol> {
    TABLE.iter().find(|sym| sym.name == name).copied()
}

pub fn big_op(name: &str) -> Option<BigOp> {
    BIG_OPS.iter().find(|op| op.name == name).copied()
}

pub fn function(name: &str) -> Option<Function> {
    FUNCTIONS.iter().find(|f| f.name == name).copied()
}

pub fn accent(name: &str) -> Option<Accent> {
    ACCENTS.iter().find(|a| a.name == name).copied()
}

/// Delimiter names usable after `\left`, `\right`, `\big` ... → glyph text.
pub fn delimiter(token: &str) -> Option<&str> {
    if token.len() == 1 {
        return match token.as_bytes()[0] {
            b'(' | b')' | b'[' | b']' | b'|' | b'/' | b'<' | b'>' => match token.as_bytes()[0] {
                b'<' => Some("⟨"),
                b'>' => Some("⟩"),
                _ => Some(token),
            },
            b'.' => Some(""),
            _ => None,
        };
    }
    let name = token.strip_prefix('\\').unwrap_or(token);
    if name == "{" || name == "lbrace" {
        return Some("{");
    }
    if name == "}" || name == "rbrace" {
        return Some("}");
    }
    if name == "|" || name == "Vert" || name == "lVert" || name == "rVert" {
        return Some("‖");
    }
    if name == "vert" || name == "lvert" || name == "rvert" {
        return Some("|");
    }
    if name == "backslash" {
        return Some("\\");
    }
    for n in [
        "langle",
        "rangle",
        "lfloor",
        "rfloor",
        "lceil",
        "rceil",
        "lgroup",
        "rgroup",
        "uparrow",
        "downarrow",
        "updownarrow",
        "Uparrow",
        "Downarrow",
        "Updownarrow",
        "llbracket",
        "rrbracket",
    ] {
        if name == n {
            return Some(lookup(n).expect("delimiter is in the symbol table").text);
        }
    }
    None
}

/// Class of a literal character in math mode.
pub fn char_class(cp: char) -> Class {
    match cp {
        '+' | '-' | '*' | '\u{2212}' | '\u{00B1}' | '\u{00D7}' | '\u{00F7}' | '\u{22C5}'
        | '\u{2218}' | '\u{2219}' => Class::Bin,
        '=' | '<' | '>' | ':' | '\u{2264}' | '\u{2265}' | '\u{2260}' | '\u{2248}' | '\u{2261}'
        | '\u{223C}' | '\u{2208}' | '\u{2209}' | '\u{2282}' | '\u{2283}' | '\u{2286}'
        | '\u{2287}' | '\u{2192}' | '\u{2190}' | '\u{21D2}' | '\u{21D4}' => Class::Rel,
        '(' | '[' | '\u{27E8}' => Class::Open,
        ')' | ']' | '!' | '?' | '\u{27E9}' => Class::Close,
        ',' | ';' => Class::Punct,
        _ => Class::Ord,
    }
}
