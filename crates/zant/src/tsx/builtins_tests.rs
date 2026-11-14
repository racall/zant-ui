//! Ported tests of `src/tsx_builtins.zig`: same names, same assertions (the
//! `tsx.fmt` template assertion becomes `format!` with [`string`], which is
//! what generated Rust code emits).

use super::*;
use crate::Str;

fn eq_str(expected: &str, got: Str) {
    assert_eq!(expected, got.as_str());
}

fn approx(expected: f64, got: f64) {
    assert!(
        (expected - got).abs() < 1e-14,
        "expected {expected}, got {got}"
    );
}

#[test]
fn math_unary_operations_integer_precision_and_ieee_edge_cases() {
    for op in [
        Unary::Floor,
        Unary::Ceil,
        Unary::Round,
        Unary::Trunc,
        Unary::Abs,
    ] {
        assert_eq!(unary(op, 9_007_199_254_740_993u64), 9_007_199_254_740_993);
    }
    assert_eq!(unary(Unary::Abs, -9i32), 9);
    for (op, input, expected) in [
        (Unary::Floor, -2.5, -3.0),
        (Unary::Ceil, -2.5, -2.0),
        (Unary::Round, -2.5, -2.0),
        (Unary::Trunc, -2.5, -2.0),
        (Unary::Abs, -2.5, 2.5),
        (Unary::Sign, -2.5, -1.0),
        (Unary::Sqrt, 9.0, 3.0),
        (Unary::Cbrt, -8.0, -2.0),
        (Unary::Exp, 0.0, 1.0),
        (Unary::Log, 1.0, 0.0),
        (Unary::Log2, 8.0, 3.0),
        (Unary::Log10, 100.0, 2.0),
        (Unary::Sin, 0.0, 0.0),
        (Unary::Cos, 0.0, 1.0),
        (Unary::Tan, 0.0, 0.0),
        (Unary::Atan, 0.0, 0.0),
    ] {
        approx(expected, unary(op, input));
    }
    assert_eq!(unary(Unary::Round, 2.5f64), 3.0);
    assert_eq!(unary(Unary::Round, 0.499_999_999_999_999_94), 0.0);
    assert!(unary(Unary::Round, -0.5).is_sign_negative());
    for op in [
        Unary::Floor,
        Unary::Ceil,
        Unary::Round,
        Unary::Trunc,
        Unary::Sign,
    ] {
        assert!(unary(op, -0.0).is_sign_negative(), "{op:?} of -0");
        let infinite = unary(op, f64::INFINITY);
        assert_eq!(
            if op == Unary::Sign {
                1.0
            } else {
                f64::INFINITY
            },
            infinite
        );
        assert!(unary(op, f64::NAN).is_nan(), "{op:?} of NaN");
    }
    assert_eq!(pow(2.0, 3.0), 8.0);
    assert_eq!(pow(f64::NAN, 0.0), 1.0);
    assert!(pow(1.0, f64::NAN).is_nan());
    assert!(pow(-1.0, f64::INFINITY).is_nan());
    assert!(pow(-2.0, 0.5).is_nan());
    assert_eq!(pow(-0.0, -3.0), f64::NEG_INFINITY);
    assert_eq!(atan2(0.0, -1.0), std::f64::consts::PI);
    assert!(atan2(-0.0, 1.0).is_sign_negative());
    assert_eq!(hypot(&[3.0, 4.0]), 5.0);
    assert_eq!(hypot(&[]), 0.0);
    assert_eq!(hypot(&[f64::NAN, f64::INFINITY]), f64::INFINITY);
    assert!(hypot(&[1e308, 1e308]).is_finite());
    assert_eq!(extrema(true, &[] as &[f64]), f64::INFINITY);
    assert_eq!(extrema(false, &[] as &[f64]), f64::NEG_INFINITY);
    assert_eq!(extrema(true, &[7i32, -9]), -9);
    assert!(extrema(false, &[1.0, f64::NAN, 3.0]).is_nan());
    assert!(extrema(true, &[0.0, -0.0]).is_sign_negative());
    assert!(!extrema(false, &[-0.0, 0.0]).is_sign_negative());
}

#[test]
fn number_conversion_predicates_and_js_prefix_parsers() {
    for (input, expected) in [
        ("", 0.0),
        (" \t\u{feff}\u{a0}", 0.0),
        (" 0x1f ", 31.0),
        ("0b101", 5.0),
        ("0o17", 15.0),
        (" +1.25e2 ", 125.0),
        ("-Infinity", f64::NEG_INFINITY),
    ] {
        assert_eq!(number(input), expected, "Number({input:?})");
    }
    for input in [
        "1tail", "0x", "-0x1f", "0b12", "1e", "nan", "inf", "1_0", "\u{85}1",
    ] {
        assert!(number(input).is_nan(), "Number({input:?}) must be NaN");
    }
    assert_eq!(number(true), 1.0);
    assert_eq!(number(false), 0.0);
    assert_eq!(number(42u32), 42.0);
    assert_eq!(number(1.5f32), 1.5);
    assert!(predicate(Predicate::IsNaN, f64::NAN));
    assert!(!predicate(Predicate::IsNaN, "NaN"));
    assert!(!predicate(Predicate::IsFinite, f64::INFINITY));
    assert!(!predicate(Predicate::IsFinite, true));
    assert!(predicate(Predicate::IsInteger, 9_007_199_254_740_993u64));
    assert!(predicate(Predicate::IsInteger, -0.0));
    assert!(!predicate(Predicate::IsInteger, 1.5));
    assert_eq!(parse_int(" \u{2028}-0x1fz", 0.0), -31.0);
    assert_eq!(parse_int("+1012", 2.0), 5.0);
    assert_eq!(parse_int("z!", 36.0), 35.0);
    assert_eq!(parse_int("0x1f", 16.0), 31.0);
    assert_eq!(parse_int("0x1f", 10.0), 0.0);
    assert_eq!(parse_int("12", 4_294_967_306.0), 12.0);
    assert!(parse_int("a", 10.0).is_nan());
    assert!(parse_int("1", 1.0).is_nan());
    assert!(parse_int("0x", 0.0).is_nan());
    assert!(parse_int("-0", 0.0).is_sign_negative());
    assert_eq!(parse_int("9007199254740993", 10.0), 9_007_199_254_740_992.0);
    assert_eq!(parse_float(" \u{3000}+1.25e2tail"), 125.0);
    assert_eq!(parse_float("1e+oops"), 1.0);
    assert_eq!(parse_float("0x1f"), 0.0);
    assert_eq!(parse_float("-Infinity!"), f64::NEG_INFINITY);
    assert_eq!(parse_float("1e999"), f64::INFINITY);
    assert!(parse_float(".").is_nan());
    assert!(parse_float("inf").is_nan());
    assert!(parse_float("-0").is_sign_negative());
}

#[test]
fn js_number_formatting_decimal_carries_templates_and_clocks() {
    for (value, expected) in [
        (1.0, "1"),
        (-0.0, "0"),
        (f64::NAN, "NaN"),
        (f64::INFINITY, "Infinity"),
        (f64::NEG_INFINITY, "-Infinity"),
        (1e20, "100000000000000000000"),
        (1e21, "1e+21"),
        (1e-6, "0.000001"),
        (1e-7, "1e-7"),
        (5e-324, "5e-324"),
        (1e308, "1e+308"),
        (1e23, "1e+23"),
        (1000000000000000128.0, "1000000000000000100"),
    ] {
        eq_str(expected, string(value));
    }
    eq_str("9007199254740993", string(9_007_199_254_740_993u64));
    eq_str("1000000000000000128", to_fixed(1000000000000000128.0, 0.0));
    eq_str("100000000000000000000.00", to_fixed(1e20, 2.0));
    eq_str("0.61", to_fixed(0.615, 2.0));
    eq_str("1.00", to_fixed(1.005, 2.0));
    eq_str("true", string(true));
    eq_str("hello", string("hello"));
    for (value, digits, expected) in [
        (0.995, 2.0, "0.99"),
        (9.999, 2.0, "10.00"),
        (1.25, 1.0, "1.3"),
        (-1.25, 1.0, "-1.3"),
        (-0.0, 2.0, "0.00"),
        (-0.001, 2.0, "-0.00"),
        (1e21, 2.0, "1e+21"),
        (f64::INFINITY, 2.0, "Infinity"),
        (f64::NAN, 2.0, "NaN"),
        (0.3, 20.0, "0.29999999999999998890"),
        (5e-324, 20.0, "0.00000000000000000000"),
    ] {
        eq_str(expected, to_fixed(value, digits));
    }
    // Zig: `tsx.fmt("{{{d}}} {d} {d} {d} {d}", …)`; Rust codegen emits
    // `format!` with the JS spelling at the float fields.
    let formatted = format!(
        "{{{}}} {} {} {} {}",
        string(1e21f64),
        string(-0.0f64),
        string(f64::NAN),
        string(f64::INFINITY),
        42u64
    );
    eq_str("{1e+21} 0 NaN Infinity 42", Str::from(formatted));
    assert!(date_now() > 1_700_000_000_000.0);
    let before = performance_now();
    assert!(performance_now() >= before);
    for _ in 0..100 {
        let value = random();
        assert!((0.0..1.0).contains(&value), "Math.random out of range");
    }
}
