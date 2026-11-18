//! Unit tests.

use super::*;
use crate::ast::{Expr, ExprData, Member, Name};
use crate::codegen::{Generator, scope};

fn with_generator(f: impl FnOnce(&mut Generator<'_>)) {
    let mut diag = None;
    let mut g = Generator::new(&mut diag);
    f(&mut g);
}

#[test]
fn keywords_and_dollar_signs_get_safe_names() {
    assert_eq!("type_", id(b"type").unwrap());
    assert_eq!("self_", id(b"self").unwrap());
    assert_eq!("_x", id(b"$x").unwrap());
    assert_eq!("name", id(b"name").unwrap());
    assert_eq!("ünïcode", id("ünïcode".as_bytes()).unwrap());
    assert!(id(b"\xff").is_none());
}

#[test]
fn strings_quote_with_rust_escapes() {
    assert_eq!("\"a\\\"b\"", quote(b"a\"b").unwrap());
    assert_eq!("\"a\\\\b\"", quote(b"a\\b").unwrap());
    assert_eq!("\"a\\nb\\tc\"", quote(b"a\nb\tc").unwrap());
    assert_eq!("\"caf\u{e9}\"", quote("café".as_bytes()).unwrap());
    assert_eq!("\"\\u{1}\"", quote(b"\x01").unwrap());
    assert!(quote(b"\xff").is_none());
}

#[test]
fn enum_variants_are_pascal_case() {
    assert_eq!("Active", enum_variant(b"active").unwrap());
    assert_eq!("InProgress", enum_variant(b"in-progress").unwrap());
    assert_eq!("SpaceBetween", enum_variant(b"space between").unwrap());
    assert_eq!("_2x", enum_variant(b"2x").unwrap());
    assert_eq!("Empty", enum_variant(b"").unwrap());
    assert_eq!("Sym2d", enum_variant(b"-").unwrap());
}

#[test]
fn precedence_parenthesizes_only_weaker_operands() {
    let sum = Value {
        code: "a + b".to_owned(),
        precedence: Precedence::Sum,
        ..Value::default()
    };
    assert_eq!("a + b", operand(&sum, Precedence::Sum, false));
    assert_eq!("(a + b)", operand(&sum, Precedence::Product, false));
    assert_eq!("(a + b)", operand(&sum, Precedence::Product, true));
    let primary = Value::new(Type::F64, "a");
    assert_eq!("a", operand(&primary, Precedence::Suffix, true));
}

#[test]
fn format_text_doubles_braces_and_quotes_literals() {
    let mut format = Format::default();
    format.text(b"a{b}\n");
    with_generator(|g| {
        // without substitutions the text stays ordinary (only braces
        // matter for a format! pattern, which is built separately)
        assert_eq!(
            "zant::Str::from(\"a{b}\\n\")",
            format.finish(g, false).unwrap()
        );
    });
    let pattern = String::from_utf8(format.pattern).unwrap();
    assert_eq!("a{{b}}\n", pattern);
}

#[test]
fn format_joins_substitutions_with_str_copy() {
    let mut format = Format::default();
    format.text(b"hi ");
    format.args.push("name".to_owned());
    format.pattern.extend_from_slice(b"{}");
    format.text(b"!");
    with_generator(|g| {
        assert_eq!(
            "zant::Str::copy(&format!(\"hi {}!\", name))",
            format.finish(g, false).unwrap()
        );
    });
}

#[test]
fn color_literals_convert_to_argb() {
    with_generator(|g| {
        let mut c = scope::Context::default();
        let rgb = Expr {
            loc: 0,
            data: ExprData::String(b"#1677ff".to_vec()),
        };
        assert_eq!("0xff1677ff", color(g, &mut c, &rgb).unwrap());
        let rgba = Expr {
            loc: 0,
            data: ExprData::String(b"#1677ff80".to_vec()),
        };
        assert_eq!("0x801677ff", color(g, &mut c, &rgba).unwrap());
        let short = Expr {
            loc: 0,
            data: ExprData::String(b"#f0a".to_vec()),
        };
        assert_eq!("0xffff00aa", color(g, &mut c, &short).unwrap());
        let bad = Expr {
            loc: 0,
            data: ExprData::String(b"red".to_vec()),
        };
        assert!(color(g, &mut c, &bad).is_err());
    });
}

#[test]
fn labels_are_found_only_outside_literals_and_comments() {
    let body = "if x { break '_loop; }";
    assert!(uses_label(body, "_loop"));
    assert!(!uses_label("log(\"break '_loop;\");", "_loop"));
    assert!(!uses_label("// break '_loop;\n", "_loop"));
    assert!(!uses_label("/* break '_loop; */", "_loop"));
    assert!(!uses_label("break '_other;", "_loop"));
}

#[test]
fn identifier_counting_skips_fields_labels_and_literals() {
    let counts =
        count_identifiers("let value = self.value.clone();\nbreak '_loop;\nlog(\"value\");\n");
    // `let value`, `self`, `break`, `log`: field names, labels and string
    // contents are not identifiers.
    assert_eq!(Some(&1), counts.get("value"));
    assert!(!counts.contains_key("_loop"));
    assert!(!counts.contains_key("clone"));
}

#[test]
fn member_names_are_available_to_the_scanner() {
    let member = Member {
        object: Box::new(Expr {
            loc: 0,
            data: ExprData::Ident(b"s".to_vec()),
        }),
        name: Name {
            name: b"prompt".to_vec(),
            loc: 2,
        },
        optional: false,
    };
    assert_eq!(b"prompt".to_vec(), member.name.name);
}
