//! JavaScript number formatting: `String(number)` / `.toString()` /
//! `.toFixed()` / `.toPrecision()` / `.toString(radix)`.
//!
//! The codegen emits `format!` directly and calls [`js_float_string`] at the
//! float fields of a template string.
//!
//! `toFixed` rounds the *exact binary value* half away from zero (JS
//! `toFixed`, unlike Rust's `{:.n}` which rounds half to even): `1.25` → `1.3`
//! but `0.615` → `0.61` (the stored double is below 0.615). The exact decimal
//! expansion is computed from the double's mantissa with [`BigUint`].

use crate::Str;
use crate::tsx::bigint::BigUint;

/// The exact decimal expansion of a finite, positive double:
/// `value = 0.<digits> * 10^point` (no leading zeros, trailing zeros removed).
#[derive(Clone, Debug)]
pub(crate) struct ExactDecimal {
    pub digits: Vec<u8>,
    /// Decimal exponent: `0.digits * 10^point`.
    pub point: i64,
}

/// The exact decimal expansion of `value` (positive); `None` when the value
/// is zero or not finite.
pub(crate) fn exact_decimal(value: f64) -> Option<ExactDecimal> {
    if !value.is_finite() || value == 0.0 {
        return None;
    }
    let bits = value.abs().to_bits();
    let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & 0x000f_ffff_ffff_ffff;
    let (mantissa, exponent) = if exponent_bits == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1 << 52), exponent_bits - 1075)
    };
    let mut big = BigUint::from_u64(mantissa);
    if exponent >= 0 {
        big.shl(exponent as usize);
        let digits = big.to_decimal().into_bytes();
        let point = digits.len() as i64;
        return Some(ExactDecimal {
            digits: strip_trailing_zeros(digits),
            point,
        });
    }
    let scale = (-exponent) as usize;
    for _ in 0..scale {
        big.mul_u64(5);
    }
    let digits = big.to_decimal().into_bytes();
    let point = digits.len() as i64 - scale as i64;
    Some(ExactDecimal {
        digits: strip_trailing_zeros(digits),
        point,
    })
}

fn strip_trailing_zeros(mut digits: Vec<u8>) -> Vec<u8> {
    while digits.len() > 1 && digits.last() == Some(&b'0') {
        digits.pop();
    }
    digits
}

/// `digits + 1` in decimal (may grow by one leading digit).
fn increment_decimal(digits: &[u8]) -> Vec<u8> {
    let mut out = digits.to_vec();
    for index in (0..out.len()).rev() {
        if out[index] == b'9' {
            out[index] = b'0';
        } else {
            out[index] += 1;
            return out;
        }
    }
    out.insert(0, b'1');
    out
}

fn zeros(count: usize) -> String {
    "0".repeat(count)
}

/// `String(x)` / `Number.prototype.toString()` (no radix): JavaScript's
/// Number::toString (shortest round-trip digits, fixed for 1e-6 <= |x| <
/// 1e21, `1e+21` style outside), from `ryu-js`.
pub fn js_float_string(x: f64) -> Str {
    if x == 0.0 {
        // `-0` prints as `0`
        return Str::Static("0");
    }
    Str::copy(ryu_js::Buffer::new().format(x))
}

/// Appends `String(x)` to `out` without an intermediate string (i18n
/// messages, plural operands): the one JS number spelling of the runtime.
pub fn write_js_number(out: &mut String, x: f64) {
    if x == 0.0 {
        out.push('0');
    } else if x.fract() == 0.0 && x.abs() < 9_007_199_254_740_992.0 {
        // safe integers: plain digits
        use std::fmt::Write;
        write!(out, "{}", x as i64).expect("writing to String");
    } else {
        out.push_str(ryu_js::Buffer::new().format(x));
    }
}

/// `Number.prototype.toFixed(digits)` (JS rounding: exact binary value, ties
/// away from zero; `-0.001.toFixed(2)` is `-0.00`).
pub fn format_fixed(x: f64, digits: f64) -> Str {
    to_fixed(x, digits)
}

/// `Number.prototype.toFixed`.
pub fn to_fixed(x: f64, digits: f64) -> Str {
    let digits = if digits.is_nan() { 0.0 } else { digits.trunc() };
    if !(0.0..=20.0).contains(&digits) || !digits.is_finite() {
        panic!("TSX toFixed digits must be 0..20");
    }
    if !x.is_finite() || x.abs() >= 1e21 {
        return js_float_string(x);
    }
    let places = digits as usize;
    let negative = x < 0.0;
    let exact = exact_decimal(x.abs());
    // Number of decimal digits of round(|x| * 10^places) before rounding.
    let keep = exact
        .as_ref()
        .map_or(0, |decimal| decimal.point + places as i64);
    let rounded = match &exact {
        None => String::from("0"),
        Some(decimal) if keep <= 0 => {
            let up = keep == 0 && decimal.digits.first().is_some_and(|digit| *digit >= b'5');
            String::from(if up { "1" } else { "0" })
        }
        Some(decimal) => {
            let keep = keep as usize;
            let mut kept: Vec<u8> = decimal.digits.iter().copied().take(keep).collect();
            while kept.len() < keep {
                kept.push(b'0');
            }
            let round_up = decimal.digits.get(keep).is_some_and(|digit| *digit >= b'5');
            let rounded = if round_up {
                increment_decimal(&kept)
            } else {
                kept
            };
            String::from_utf8(rounded).expect("decimal digits are ASCII")
        }
    };
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if places == 0 {
        out.push_str(&rounded);
    } else if rounded.len() > places {
        let split = rounded.len() - places;
        out.push_str(&rounded[..split]);
        out.push('.');
        out.push_str(&rounded[split..]);
    } else {
        out.push_str("0.");
        out.push_str(&zeros(places - rounded.len()));
        out.push_str(&rounded);
    }
    Str::from(out)
}

/// `Number.prototype.toPrecision(precision)`: `precision` significant digits
/// (JS thresholds for fixed / scientific spelling, ties away from zero).
///
/// The compiler only emits `toFixed` / `toString()` today; provided for
/// codegen completeness.
pub fn to_precision(x: f64, precision: f64) -> Str {
    if precision.is_nan() {
        return js_float_string(x);
    }
    let precision = precision.trunc();
    if !(1.0..=100.0).contains(&precision) {
        panic!("TSX toPrecision precision must be 1..100");
    }
    if !x.is_finite() {
        return js_float_string(x);
    }
    let significant = precision as usize;
    let negative = x < 0.0;
    let magnitude = x.abs();
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if magnitude == 0.0 {
        out.push('0');
        if significant > 1 {
            out.push('.');
            out.push_str(&zeros(significant - 1));
        }
        return Str::from(out);
    }
    let exact = exact_decimal(magnitude).expect("a finite positive value");
    let mut exponent = exact.point - 1;
    let mut digits: Vec<u8> = exact.digits.iter().copied().take(significant).collect();
    while digits.len() < significant {
        digits.push(b'0');
    }
    let round_up = exact
        .digits
        .get(significant)
        .is_some_and(|digit| *digit >= b'5');
    if round_up {
        digits = increment_decimal(&digits);
        if digits.len() > significant {
            // 99…9 + 1: one more digit, so `1` followed by zeros.
            digits = std::iter::once(b'1')
                .chain(std::iter::repeat_n(b'0', significant - 1))
                .collect();
            exponent += 1;
        }
    }
    let digits = String::from_utf8(digits).expect("decimal digits are ASCII");
    if exponent < -6 || exponent >= significant as i64 {
        out.push_str(&digits[..1]);
        if significant > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        if exponent >= 0 {
            out.push('+');
        }
        out.push_str(&exponent.to_string());
    } else if exponent < 0 {
        out.push_str("0.");
        out.push_str(&zeros((-exponent - 1) as usize));
        out.push_str(&digits);
    } else {
        let split = (exponent + 1) as usize;
        out.push_str(&digits[..split]);
        if digits.len() > split {
            out.push('.');
            out.push_str(&digits[split..]);
        }
    }
    Str::from(out)
}

/// `Number.prototype.toString(radix)`: base 10 uses [`js_float_string`],
/// other bases spell the integer part exactly and the fraction with up to 52
/// digits (enough for the dyadic bases; non-terminating expansions like base 3
/// are rounded off, JS engines differ in their last digit too).
///
/// Provided for codegen completeness.
pub fn to_string_radix(x: f64, radix: f64) -> Str {
    let radix = if radix.is_nan() { 10.0 } else { radix.trunc() };
    if !(2.0..=36.0).contains(&radix) {
        panic!("TSX toString radix must be 2..36");
    }
    let radix = radix as u32;
    if radix == 10 {
        return js_float_string(x);
    }
    if x.is_nan() {
        return Str::Static("NaN");
    }
    if x.is_infinite() {
        return Str::Static(if x < 0.0 { "-Infinity" } else { "Infinity" });
    }
    let negative = x < 0.0;
    let magnitude = x.abs();
    let digits = "0123456789abcdefghijklmnopqrstuvwxyz";
    if magnitude == 0.0 {
        return Str::Static("0");
    }
    // Integer part, exactly.
    let integral = magnitude.floor();
    let mut decimal = String::new();
    match exact_decimal(integral) {
        None => decimal.push('0'),
        Some(exact) if exact.point > 0 => {
            for index in 0..exact.point as usize {
                decimal.push(exact.digits.get(index).copied().unwrap_or(b'0') as char);
            }
        }
        Some(_) => decimal.push('0'),
    }
    let mut big = BigUint::from_decimal(&decimal);
    let mut integer_digits = Vec::new();
    while !big.is_zero() {
        let remainder = big.div_rem_u64(u64::from(radix));
        integer_digits.push(digits.as_bytes()[remainder as usize] as char);
    }
    integer_digits.reverse();
    if integer_digits.is_empty() {
        integer_digits.push('0');
    }
    // Fraction part.
    let mut fraction = magnitude - integral;
    let mut fraction_digits = String::new();
    let mut guard = 0;
    while fraction != 0.0 && guard < 52 {
        fraction *= f64::from(radix);
        let digit = fraction.floor();
        fraction -= digit;
        fraction_digits.push(digits.as_bytes()[digit as usize] as char);
        guard += 1;
    }
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.extend(integer_digits);
    if !fraction_digits.is_empty() {
        out.push('.');
        out.push_str(&fraction_digits);
    }
    Str::from(out)
}

/// Formats `x` as a JS float (re-exported convenience for the codegen).
pub fn format_float(x: f64) -> Str {
    js_float_string(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tsx::bigint::BigUint;

    fn eq(expected: &str, got: Str) {
        assert_eq!(expected, got.as_str());
    }

    #[test]
    fn exact_decimal_expansion_of_binary_values() {
        let half = exact_decimal(0.5).expect("finite");
        assert_eq!(half.digits, b"5");
        assert_eq!(half.point, 0);
        let quarter = exact_decimal(0.25).expect("finite");
        assert_eq!(quarter.digits, b"25");
        assert_eq!(quarter.point, 0);
        let one = exact_decimal(1.0).expect("finite");
        assert_eq!(one.digits, b"1");
        assert_eq!(one.point, 1);
        assert!(exact_decimal(0.0).is_none());
        assert!(exact_decimal(f64::NAN).is_none());
        assert!(exact_decimal(f64::INFINITY).is_none());
        // 0.1 is not 1/10 in binary: the expansion is long
        let tenth = exact_decimal(0.1).expect("finite");
        assert!(tenth.digits.len() > 40);
        assert_eq!(tenth.point, 0);
    }

    #[test]
    fn js_number_spelling_of_floats() {
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
            (0.3, "0.3"),
            (1.5e-6, "0.0000015"),
            (-2.5, "-2.5"),
        ] {
            eq(expected, js_float_string(value));
        }
    }

    #[test]
    fn to_fixed_rounds_the_exact_binary_value() {
        for (value, digits, expected) in [
            (1000000000000000128.0, 0.0, "1000000000000000128"),
            (1e20, 2.0, "100000000000000000000.00"),
            (0.615, 2.0, "0.61"),
            (1.005, 2.0, "1.00"),
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
            (0.5, 0.0, "1"),
            (-0.5, 0.0, "-1"),
            (2.5, 0.0, "3"),
            (1.5, 0.0, "2"),
            (9.99, 1.0, "10.0"),
            (100.0, 2.0, "100.00"),
            (0.005, 2.0, "0.01"),
            (0.0004, 2.0, "0.00"),
        ] {
            eq(expected, to_fixed(value, digits));
        }
    }

    #[test]
    fn to_precision_follows_javascript() {
        for (value, precision, expected) in [
            (0.0, 3.0, "0.00"),
            (-0.0, 2.0, "0.0"),
            (123.456, 4.0, "123.5"),
            (0.000123, 2.0, "0.00012"),
            (123456.0, 2.0, "1.2e+5"),
            (1e21, 3.0, "1.00e+21"),
            (9.99, 2.0, "10"),
            (999.0, 2.0, "1.0e+3"),
            (1.25, 3.0, "1.25"),
            (-1.25, 2.0, "-1.3"),
            (0.5, 1.0, "0.5"),
            (-0.5, 1.0, "-0.5"),
            (100.0, 5.0, "100.00"),
            (1e-7, 2.0, "1.0e-7"),
            (f64::NAN, 2.0, "NaN"),
            (f64::INFINITY, 2.0, "Infinity"),
            (1.0, 1.0, "1"),
        ] {
            eq(expected, to_precision(value, precision));
        }
    }

    #[test]
    fn to_string_radix_spells_other_bases() {
        eq("ff", to_string_radix(255.0, 16.0));
        eq("0.1", to_string_radix(0.5, 2.0));
        eq("-a.8", to_string_radix(-10.5, 16.0));
        eq("1010", to_string_radix(10.0, 2.0));
        eq("z", to_string_radix(35.0, 36.0));
        eq("1.8", to_string_radix(1.5, 16.0));
        eq("NaN", to_string_radix(f64::NAN, 16.0));
        eq("Infinity", to_string_radix(f64::INFINITY, 16.0));
        eq("0", to_string_radix(0.0, 2.0));
        eq("1e+21", to_string_radix(1e21, 10.0));
        eq(
            "18446744073709552000",
            to_string_radix(18446744073709551616.0, 10.0),
        );
        eq(
            "10000000000000000",
            to_string_radix(18446744073709551616.0, 16.0),
        );
        eq("3635c9adc5dea00000", to_string_radix(1e21, 16.0));
    }

    #[test]
    fn bigint_helpers_round_trip() {
        assert_eq!(BigUint::from_decimal("123456789").to_decimal(), "123456789");
        let mut value = BigUint::from_decimal("18446744073709551616");
        assert_eq!(value.div_rem_u64(16), 0);
        assert_eq!(value.to_decimal(), "1152921504606846976");
    }
}
