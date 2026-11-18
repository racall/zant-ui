//! JavaScript numeric built-ins for generated TSX code.
//!
//! The TSX compiler maps `Math.*`, `Number(...)`, `parseInt`, `parseFloat`,
//! `String(...)`, `.toString()` and `.toFixed()` onto these helpers so the
//! generated Rust follows JavaScript's rules instead of Rust's `FromStr` /
//! `f64` methods (NaN and signed-zero edge cases, `Math.round(-0.5) === -0`,
//! JS decimal spelling, `parseInt`'s radix argument).
//!
//! Numbers are `f64` (TS has one number type); the two integer-preserving
//! cases keep their Rust integer type: [`unary`] on an integer argument and
//! [`extrema`] over a slice of integers.

use std::cell::Cell;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::Str;
use crate::core::unicode::decode_at;
use crate::tsx::Any;
use crate::tsx::bigint::BigUint;

use super::builtins_format::js_float_string;
pub use super::builtins_format::{to_fixed, to_precision, to_string_radix};

mod math;
mod parse;

#[cfg(test)]
mod tests;

pub use self::math::*;
pub use self::parse::*;

/// Unary `Math` operations.
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
/// integer-preserving ones (an unsupported pair panics, and the codegen never
/// emits it).
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
                        // integer arithmetic.
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

/// The values `Math.min` / `Math.max` accept.
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
                    let mut result = values[0]; // panics on an empty slice
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
/// (including the JS radix prefixes).
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
    /// Process-wide noncryptographic PRNG.
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
