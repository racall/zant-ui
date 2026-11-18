//! Unit tests.

use super::*;
use crate::ast::{Expr, ExprData};

fn with_generator(f: impl FnOnce(&mut Generator<'_>)) {
    let mut diag = None;
    let mut g = Generator::new(&mut diag);
    f(&mut g);
}

#[test]
fn integer_peers_prefer_the_wider_type() {
    assert_eq!(Some(Type::Usize), Type::numeric_peer(Type::U8, Type::Usize));
    assert_eq!(Some(Type::I32), Type::numeric_peer(Type::U16, Type::I32));
    // u64 and i32 hold different ranges: a TS number
    assert_eq!(Some(Type::F64), Type::numeric_peer(Type::U64, Type::I32));
    assert_eq!(
        Some(Type::F32),
        Type::numeric_peer(Type::F32, Type::IntLiteral)
    );
    assert_eq!(
        Some(Type::F64),
        Type::numeric_peer(Type::FloatLiteral, Type::I32)
    );
    assert_eq!(None, Type::numeric_peer(Type::Boolean, Type::F64));
}

#[test]
fn literal_codes_are_typed_for_the_target() {
    assert_eq!("1u32", literal_code("1", Type::U32));
    assert_eq!("-2i64", literal_code("-2", Type::I64));
    assert_eq!("1.0f64", literal_code("1", Type::F64));
    assert_eq!("0.5f32", literal_code("0.5", Type::F32));
    assert_eq!("1e3f64", literal_code("1e3", Type::F64));
    assert_eq!("(0xff as f64)", literal_code("0xff", Type::F64));
}

#[test]
fn integer_literals_parse_radix_and_separators() {
    assert_eq!(Some(255), parse_integer(b"0xff"));
    assert_eq!(Some(-2), parse_integer(b"-2"));
    assert_eq!(Some(1_000_000), parse_integer(b"1_000_000"));
    assert_eq!(Some(8), parse_integer(b"0o10"));
    assert_eq!(Some(5), parse_integer(b"0b101"));
    assert_eq!(None, parse_integer(b"1.5"));
}

#[test]
fn float_literals_are_decimal_only() {
    assert!(float_literal(b"1.5"));
    assert!(float_literal(b"1e3"));
    assert!(!float_literal(b"15"));
    assert!(!float_literal(b"0xff"));
}

#[test]
fn out_of_range_literals_report_the_source_location() {
    with_generator(|g| {
        let e = Expr {
            loc: 3,
            data: ExprData::Number(b"300".to_vec()),
        };
        assert!(check_literal(g, &e, Type::U8).is_err());
        let diag = g.diag.as_ref().expect("a diagnostic");
        assert!(diag.msg.contains("does not fit u8"), "{}", diag.msg);
        assert_eq!(3, diag.loc);
    });
}

#[test]
fn integer_literals_inside_the_range_pass() {
    with_generator(|g| {
        let e = Expr {
            loc: 0,
            data: ExprData::Number(b"200".to_vec()),
        };
        assert!(check_literal(g, &e, Type::U8).is_ok());
        assert!(check_literal(g, &e, Type::I8).is_err());
    });
}

#[test]
fn unify_null_with_a_concrete_type_is_optional() {
    with_generator(|g| {
        let ty = unify(g, Type::String, Type::Null, 0).expect("unifies");
        assert_eq!(Some(Type::String), model::optional_child(g, ty));
        assert!(ty.rust(g).starts_with("Option<"));
        let nodes = unify(g, Type::Node, Type::Null, 0).expect("unifies");
        assert_eq!(Type::OptionalNode, nodes);
    });
}

#[test]
fn is_literal_recognizes_negative_number_literals() {
    let negative = Expr {
        loc: 0,
        data: ExprData::Unary(ast::Unary {
            op: b"-".to_vec(),
            operand: Box::new(Expr {
                loc: 1,
                data: ExprData::Number(b"1".to_vec()),
            }),
        }),
    };
    assert!(is_literal(&negative));
    assert!(numeric_literals(&negative));
    assert!(!is_literal(&Expr {
        loc: 0,
        data: ExprData::Ident(b"x".to_vec()),
    }));
}
