//! JavaScript numeric built-ins for generated TSX code
//! (port of `src/tsx_builtins.zig`).
//!
//! The TSX compiler maps `Math.*`, `Number(...)`, `parseInt`, `parseFloat`,
//! `String(...)`, `.toString()` and `.toFixed()` onto these helpers so the
//! generated Rust follows JavaScript's rules instead of Rust's `FromStr` /
//! `f64` methods (NaN and signed-zero edge cases, `Math.round(-0.5) === -0`,
//! JS decimal spelling, `parseInt`'s radix argument).
//!
//! Numbers are `f64` (TS has one number type); the two integer-preserving
//! cases keep their Rust integer type: [`unary`] on an integer argument and
//! [`extrema`] over a slice of integers (like the Zig `anytype` functions).

use std::cell::Cell;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::Str;
use crate::core::unicode::decode_at;
use crate::tsx::Any;
use crate::tsx::bigint::BigUint;

use super::builtins_format::js_float_string;
pub use super::builtins_format::{to_fixed, to_precision, to_string_radix};

/// Unary `Math` operations (`src/tsx_builtins.zig`'s `Unary`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unary {
    Floor,
    Ceil,
    Round,
    Trunc,
    Abs,
    Sign,
    Sqrt,
    Cbrt,
    Exp,
    Log,
    Log2,
    Log10,
    Sin,
    Cos,
    Tan,
    Atan,
}

/// The `Number.isNaN` / `Number.isFinite` / `Number.isInteger` predicates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Predicate {
    IsNaN,
    IsFinite,
    IsInteger,
}

// ------------------------------------------------------------- Math unary

/// One operand of `unary`: `f64` implements every operation, integers only the
/// integer-preserving ones (Zig rejected the others at compile time; here an
/// unsupported pair panics, and the codegen never emits it).
pub trait UnaryOperand: Copy {
    fn unary(self, op: Unary) -> Self;
}

impl UnaryOperand for f64 {
    fn unary(self, op: Unary) -> f64 {
        match op {
            Unary::Floor => self.floor(),
            Unary::Ceil => self.ceil(),
            Unary::Trunc => self.trunc(),
            Unary::Abs => self.abs(),
            // JS Math.round: halves go toward +∞, but -0 keeps its sign.
            Unary::Round => {
                if !self.is_finite() || self == 0.0 {
                    self
                } else {
                    let lo = self.floor();
                    let n = if self - lo < 0.5 { lo } else { lo + 1.0 };
                    if n == 0.0 { 0.0f64.copysign(self) } else { n }
                }
            }
            // Math.sign: NaN and ±0 keep the argument; otherwise ±1.
            Unary::Sign => {
                if self.is_nan() || self == 0.0 {
                    self
                } else {
                    1.0f64.copysign(self)
                }
            }
            Unary::Sqrt => self.sqrt(),
            Unary::Cbrt => self.cbrt(),
            Unary::Exp => self.exp(),
            Unary::Log => self.ln(),
            Unary::Log2 => self.log2(),
            Unary::Log10 => self.log10(),
            Unary::Sin => self.sin(),
            Unary::Cos => self.cos(),
            Unary::Tan => self.tan(),
            Unary::Atan => self.atan(),
        }
    }
}

macro_rules! unary_int_signed {
    ($($ty:ty),* $(,)?) => {
        $(
            impl UnaryOperand for $ty {
                fn unary(self, op: Unary) -> $ty {
                    match op {
                        Unary::Floor | Unary::Ceil | Unary::Round | Unary::Trunc => self,
                        // Signed minimum overflows, just like other native
                        // integer arithmetic (Zig `-x`).
                        Unary::Abs => {
                            if self < 0 { -self } else { self }
                        }
                        _ => panic!("this Math operation requires f64"),
                    }
                }
            }
        )*
    };
}

macro_rules! unary_int_unsigned {
    ($($ty:ty),* $(,)?) => {
        $(
            impl UnaryOperand for $ty {
                fn unary(self, op: Unary) -> $ty {
                    match op {
                        Unary::Floor | Unary::Ceil | Unary::Round | Unary::Trunc | Unary::Abs => self,
                        _ => panic!("this Math operation requires f64"),
                    }
                }
            }
        )*
    };
}

unary_int_signed!(i8, i16, i32, i64, isize);
unary_int_unsigned!(u8, u16, u32, u64, usize);

/// `Math.floor` / `ceil` / `round` / `trunc` / `abs` / `sign` / … with JS
/// semantics; integer arguments keep their type for the preserving operations.
pub fn unary<T: UnaryOperand>(op: Unary, x: T) -> T {
    x.unary(op)
}

/// `Math.pow`: JS differs from libm for NaN and unit bases with infinite
/// exponents.
pub fn pow(x: f64, y: f64) -> f64 {
    if y == 0.0 {
        return 1.0;
    }
    if y.is_nan() || (x.abs() == 1.0 && y.is_infinite()) {
        return f64::NAN;
    }
    x.powf(y)
}

/// `Math.atan2` (Rust's `atan2` keeps quadrant and signed-zero semantics).
pub fn atan2(y: f64, x: f64) -> f64 {
    y.atan2(x)
}

/// `Math.hypot`: variadic, avoiding overflow / underflow; infinity wins over
/// NaN.
pub fn hypot(values: &[f64]) -> f64 {
    let mut result = 0.0f64;
    for &x in values {
        result = result.hypot(x);
    }
    result
}

/// The values `Math.min` / `Math.max` accept (Zig's `extrema` over `anytype`).
pub trait ExtremaValue: Copy + PartialOrd {
    fn extrema(minimum: bool, values: &[Self]) -> Self;
}

impl ExtremaValue for f64 {
    fn extrema(minimum: bool, values: &[Self]) -> f64 {
        let mut result = if minimum {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };
        for &x in values {
            if x.is_nan() {
                return x;
            }
            if x == 0.0 && result == 0.0 {
                // Both compare equal: JS picks -0 for min and +0 for max.
                if x.is_sign_negative() == minimum {
                    result = x;
                }
                continue;
            }
            if (minimum && x < result) || (!minimum && x > result) {
                result = x;
            }
        }
        result
    }
}

macro_rules! extrema_int {
    ($($ty:ty),* $(,)?) => {
        $(
            impl ExtremaValue for $ty {
                fn extrema(minimum: bool, values: &[$ty]) -> $ty {
                    let mut result = values[0]; // Zig indexed values[0] too
                    for &x in values {
                        if (minimum && x < result) || (!minimum && x > result) {
                            result = x;
                        }
                    }
                    result
                }
            }
        )*
    };
}

extrema_int!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

/// `Math.min` / `Math.max`: NaN propagates and the JS sign of zero wins.
pub fn extrema<T: ExtremaValue>(minimum: bool, values: &[T]) -> T {
    T::extrema(minimum, values)
}

// -------------------------------------------------------- Number predicates

/// An argument of `Number.isNaN` / `isFinite` / `isInteger`: numbers checked,
/// everything else is not a number (no string / boolean coercion).
pub trait NumberPredicate {
    fn predicate(self, kind: Predicate) -> bool;
}

macro_rules! predicate_int {
    ($($ty:ty),* $(,)?) => {
        $(
            impl NumberPredicate for $ty {
                fn predicate(self, kind: Predicate) -> bool {
                    kind != Predicate::IsNaN
                }
            }
        )*
    };
}

predicate_int!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

impl NumberPredicate for f64 {
    fn predicate(self, kind: Predicate) -> bool {
        match kind {
            Predicate::IsNaN => self.is_nan(),
            Predicate::IsFinite => self.is_finite(),
            Predicate::IsInteger => self.is_finite() && self.trunc() == self,
        }
    }
}

impl NumberPredicate for f32 {
    fn predicate(self, kind: Predicate) -> bool {
        f64::from(self).predicate(kind)
    }
}

impl NumberPredicate for bool {
    fn predicate(self, _kind: Predicate) -> bool {
        false
    }
}

impl NumberPredicate for &str {
    fn predicate(self, _kind: Predicate) -> bool {
        false
    }
}

impl NumberPredicate for Str {
    fn predicate(self, _kind: Predicate) -> bool {
        false
    }
}

impl NumberPredicate for String {
    fn predicate(self, _kind: Predicate) -> bool {
        false
    }
}

/// `Number.isNaN` / `Number.isFinite` / `Number.isInteger`.
pub fn predicate<T: NumberPredicate>(kind: Predicate, x: T) -> bool {
    x.predicate(kind)
}

// ------------------------------------------------------------- conversions

/// `Number(...)`: a JS number, or NaN when the value is not a number string
/// (exact port of the Zig conversion, including the JS radix prefixes).
pub trait NumberValue {
    fn to_js_number(self) -> f64;
}

macro_rules! number_from_int {
    ($($ty:ty),* $(,)?) => {
        $(
            impl NumberValue for $ty {
                fn to_js_number(self) -> f64 {
                    self as f64
                }
            }
        )*
    };
}

number_from_int!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

impl NumberValue for f64 {
    fn to_js_number(self) -> f64 {
        self
    }
}

impl NumberValue for f32 {
    fn to_js_number(self) -> f64 {
        f64::from(self)
    }
}

impl NumberValue for bool {
    fn to_js_number(self) -> f64 {
        if self { 1.0 } else { 0.0 }
    }
}

impl NumberValue for &str {
    fn to_js_number(self) -> f64 {
        number_from_str(self)
    }
}

impl NumberValue for Str {
    fn to_js_number(self) -> f64 {
        number_from_str(&self)
    }
}

impl NumberValue for String {
    fn to_js_number(self) -> f64 {
        number_from_str(&self)
    }
}

/// `Number(x)`.
pub fn number<T: NumberValue>(x: T) -> f64 {
    x.to_js_number()
}

/// `String(x)` / `.toString()`: JS spelling (integers exactly, floats with
/// JS's fixed/scientific thresholds and special values).
pub trait JsString {
    fn to_js_string(self) -> Str;
}

macro_rules! string_from_int {
    ($($ty:ty),* $(,)?) => {
        $(
            impl JsString for $ty {
                fn to_js_string(self) -> Str {
                    Str::copy(&format!("{self}"))
                }
            }
        )*
    };
}

string_from_int!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

impl JsString for f64 {
    fn to_js_string(self) -> Str {
        js_float_string(self)
    }
}

impl JsString for f32 {
    fn to_js_string(self) -> Str {
        js_float_string(f64::from(self))
    }
}

impl JsString for bool {
    fn to_js_string(self) -> Str {
        Str::Static(if self { "true" } else { "false" })
    }
}

impl JsString for Str {
    fn to_js_string(self) -> Str {
        self
    }
}

impl JsString for &str {
    fn to_js_string(self) -> Str {
        Str::copy(self)
    }
}

impl JsString for String {
    fn to_js_string(self) -> Str {
        Str::from(self)
    }
}

/// `String(x)`.
pub fn string<T: JsString>(x: T) -> Str {
    x.to_js_string()
}

/// `Number.prototype.toString(radix)`; `radix` NaN / out of 2..=36 panics like
/// a JS `RangeError`.
pub fn to_radix(x: f64, radix: f64) -> Str {
    to_string_radix(x, radix)
}

/// JS whitespace (`tsx_builtins.zig`'s `whitespace`).
fn whitespace(cp: char) -> bool {
    matches!(
        cp,
        '\u{09}'..='\u{0d}' | '\u{20}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}'
            | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

fn trim_start(s: &str) -> &str {
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

fn trim(s: &str) -> &str {
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

fn digit(byte: u8) -> u8 {
    if byte.is_ascii_digit() {
        return byte - b'0';
    }
    if byte.is_ascii_alphabetic() {
        return byte.to_ascii_lowercase() - b'a' + 10;
    }
    u8::MAX
}

/// Parse an integer prefix in `radix`; `full` requires consuming all of `s`.
fn integer(s: &str, radix: u8, full: bool) -> f64 {
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
    // Rust's decimal parser rounds correctly to f64 (inf on overflow), like
    // the Zig 1024-bit accumulation with 53-bit rounding.
    n.to_decimal().parse::<f64>().unwrap_or(f64::INFINITY)
}

/// Return the longest decimal prefix; a dangling exponent is excluded.
fn decimal_end(s: &str) -> usize {
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

fn number_from_str(input: &str) -> f64 {
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

/// `Date.now()`: wall-clock milliseconds since the Unix epoch.
pub fn date_now() -> f64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as f64,
        Err(_) => 0.0,
    }
}

/// `performance.now()`: monotonic milliseconds with sub-millisecond precision,
/// independent of the UI frame clock.
pub fn performance_now() -> f64 {
    crate::platform::platform::monotonic_us() as f64 / 1000.0
}

thread_local! {
    /// Process-wide noncryptographic PRNG (Zig used `std.Random.DefaultPrng`).
    static RANDOM_STATE: Cell<u64> = const { Cell::new(0) };
}

/// `Math.random()`: uniform in `[0, 1)`, seeded once per thread.
pub fn random() -> f64 {
    RANDOM_STATE.with(|state| {
        let mut x = state.get();
        if x == 0 {
            x = match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(duration) => duration.as_nanos() as u64,
                Err(_) => 0x9e37_79b9_7f4a_7c15,
            } | 1;
        }
        // xorshift64*
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        state.set(x);
        ((x >> 11) as f64) / ((1u64 << 53) as f64)
    })
}

// ------------------------------------------------------------------ console

/// `console.log(...)`: values space-separated plus a newline, on stderr
/// (debug output; generated apps do not log from the frame loop).
pub fn log(args: &[Any]) {
    let mut line = String::new();
    for (i, value) in args.iter().enumerate() {
        if i > 0 {
            line.push(' ');
        }
        line.push_str(&value.to_string());
    }
    eprintln!("{line}");
}

#[cfg(test)]
#[path = "builtins_tests.rs"]
mod tests;
