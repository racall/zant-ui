//! Unit tests.

use super::*;

/// Pins the number of entries of each table.
#[test]
fn table_counts() {
    assert_eq!(TABLE.len(), 321);
    assert_eq!(BIG_OPS.len(), 20);
    assert_eq!(FUNCTIONS.len(), 40);
    assert_eq!(ACCENTS.len(), 14);
}

/// `lookup` returns the first table entry with the given name; the table
/// has no duplicate names.
#[test]
fn table_names_are_unique() {
    for (i, sym) in TABLE.iter().enumerate() {
        for other in &TABLE[i + 1..] {
            assert_ne!(sym.name, other.name);
        }
    }
}

#[test]
fn lookup_samples() {
    assert_eq!(
        lookup("alpha"),
        Some(Symbol {
            name: "alpha",
            text: "α",
            class: Class::Ord
        })
    );
    assert_eq!(lookup("pm").expect("pm").class, Class::Bin);
    assert_eq!(lookup("leq").expect("leq").class, Class::Rel);
    assert_eq!(lookup("lbrace").expect("lbrace").class, Class::Open);
    assert_eq!(lookup("rbrace").expect("rbrace").class, Class::Close);
    assert_eq!(lookup("colon").expect("colon").class, Class::Punct);
    assert_eq!(lookup("ldots").expect("ldots").class, Class::Inner);
    assert_eq!(lookup("textbackslash").expect("textbackslash").text, "\\");
    assert_eq!(lookup("not-a-command"), None);
}

#[test]
fn big_ops_and_functions() {
    let sum = big_op("sum").expect("sum");
    assert!(sum.limits);
    assert!(!sum.integral);
    let int = big_op("int").expect("int");
    assert!(!int.limits);
    assert!(int.integral);
    assert!(function("lim").expect("lim").limits);
    assert!(!function("sin").expect("sin").limits);
    assert_eq!(function("liminf").expect("liminf").text, "lim inf");
    assert_eq!(big_op("?"), None);
    assert_eq!(function("?"), None);
}

#[test]
fn accent_samples() {
    assert!(!accent("hat").expect("hat").wide);
    assert!(accent("widehat").expect("widehat").wide);
    assert_eq!(accent("vec").expect("vec").glyph, "→");
    assert_eq!(accent("?"), None);
}

#[test]
fn delimiters() {
    assert_eq!(delimiter("("), Some("("));
    assert_eq!(delimiter("]"), Some("]"));
    assert_eq!(delimiter("<"), Some("⟨"));
    assert_eq!(delimiter(">"), Some("⟩"));
    assert_eq!(delimiter("."), Some(""));
    assert_eq!(delimiter("x"), None);
    assert_eq!(delimiter("\\{"), Some("{"));
    assert_eq!(delimiter("\\lbrace"), Some("{"));
    assert_eq!(delimiter("\\}"), Some("}"));
    assert_eq!(delimiter("|"), Some("|"));
    assert_eq!(delimiter("\\|"), Some("‖"));
    assert_eq!(delimiter("\\Vert"), Some("‖"));
    assert_eq!(delimiter("\\vert"), Some("|"));
    assert_eq!(delimiter("\\backslash"), Some("\\"));
    assert_eq!(delimiter("\\langle"), Some("⟨"));
    assert_eq!(delimiter("\\Updownarrow"), Some("⇕"));
    assert_eq!(delimiter("\\unknown"), None);
}

#[test]
fn char_classes() {
    assert_eq!(char_class('+'), Class::Bin);
    assert_eq!(char_class('\u{2212}'), Class::Bin);
    assert_eq!(char_class('='), Class::Rel);
    assert_eq!(char_class('\u{2192}'), Class::Rel);
    assert_eq!(char_class('('), Class::Open);
    assert_eq!(char_class('\u{27E8}'), Class::Open);
    assert_eq!(char_class(']'), Class::Close);
    assert_eq!(char_class('!'), Class::Close);
    assert_eq!(char_class(','), Class::Punct);
    assert_eq!(char_class('x'), Class::Ord);
}
