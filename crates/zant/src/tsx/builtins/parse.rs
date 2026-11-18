//! `parseFloat`, `parseInt` and `Number(text)` with JavaScript's whitespace and prefix rules.

use super::*;

/// JS whitespace.
pub(super) fn whitespace(cp: char) -> bool {
    matches!(
        cp,
        '\u{09}'..='\u{0d}' | '\u{20}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}'
            | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

pub(super) fn trim_start(s: &str) -> &str {
    let mut lo = 0;
    while lo < s.len() {
        let decoded = decode_at(s, lo);
        if !whitespace(decoded.codepoint) {
            break;
        }
        lo += decoded.len;
    }
    &s[lo..]
}

pub(super) fn trim(s: &str) -> &str {
    let mut lo = 0;
    let mut hi = s.len();
    while lo < hi {
        let decoded = decode_at(s, lo);
        if !whitespace(decoded.codepoint) {
            break;
        }
        lo += decoded.len;
    }
    while hi > lo {
        let previous = crate::core::unicode::prev_codepoint(s, hi);
        if !whitespace(decode_at(s, previous).codepoint) {
            break;
        }
        hi = previous;
    }
    &s[lo..hi]
}

pub(super) fn digit(byte: u8) -> u8 {
    if byte.is_ascii_digit() {
        return byte - b'0';
    }
    if byte.is_ascii_alphabetic() {
        return byte.to_ascii_lowercase() - b'a' + 10;
    }
    u8::MAX
}

/// Parse an integer prefix in `radix`; `full` requires consuming all of `s`.
pub(super) fn integer(s: &str, radix: u8, full: bool) -> f64 {
    let mut n = BigUint::from_u64(0);
    let mut count = 0;
    for byte in s.bytes() {
        let d = digit(byte);
        if d >= radix {
            break;
        }
        n.mul_u64(u64::from(radix));
        n.add_u64(u64::from(d));
        count += 1;
    }
    if count == 0 || (full && count != s.len()) {
        return f64::NAN;
    }
    // Rust's decimal parser rounds correctly to f64 (inf on overflow).
    n.to_decimal().parse::<f64>().unwrap_or(f64::INFINITY)
}

/// Return the longest decimal prefix; a dangling exponent is excluded.
pub(super) fn decimal_end(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut i = 0;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    if s[i..].starts_with("Infinity") {
        return i + 8;
    }
    let mut digits = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return 0;
    }
    let end = i;
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        i += 1;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return end;
        }
    }
    i
}

/// `parseFloat`: a decimal prefix, including signed `Infinity`; no digits
/// gives NaN.
pub fn parse_float(input: &str) -> f64 {
    let s = trim_start(input);
    let end = decimal_end(s);
    if end == 0 {
        return f64::NAN;
    }
    s[..end].parse::<f64>().unwrap_or(f64::NAN)
}

/// `parseInt`: a signed integer prefix with the JS ToInt32 radix and optional
/// hexadecimal prefix.
pub fn parse_int(input: &str, radix: f64) -> f64 {
    let mut s = trim_start(input);
    let negative = s.starts_with('-');
    if s.starts_with('+') || s.starts_with('-') {
        s = &s[1..];
    }
    let r = if !radix.is_finite() {
        0.0
    } else {
        radix.trunc().rem_euclid(4_294_967_296.0)
    };
    if r != 0.0 && !(2.0..=36.0).contains(&r) {
        return f64::NAN;
    }
    let mut base: u8 = if r == 0.0 { 10 } else { r as u8 };
    if (r == 0.0 || base == 16)
        && s.len() >= 2
        && s.as_bytes()[0] == b'0'
        && s.as_bytes()[1].eq_ignore_ascii_case(&b'x')
    {
        base = 16;
        s = &s[2..];
    }
    let value = integer(s, base, false);
    if negative { -value } else { value }
}

pub(super) fn number_from_str(input: &str) -> f64 {
    let s = trim(input);
    if s.is_empty() {
        return 0.0;
    }
    if s.len() >= 2 && s.as_bytes()[0] == b'0' {
        let base: u8 = match s.as_bytes()[1] {
            b'x' | b'X' => 16,
            b'b' | b'B' => 2,
            b'o' | b'O' => 8,
            _ => 0,
        };
        if base != 0 {
            return integer(&s[2..], base, true);
        }
    }
    if decimal_end(s) != s.len() {
        return f64::NAN;
    }
    parse_float(s)
}

// ---------------------------------------------------------------- numbers
