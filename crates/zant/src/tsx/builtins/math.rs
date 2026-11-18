//! Numeric helpers: `**`, int32 conversion, bitwise operators, relative indexes, `Math.atan2` / `Math.hypot`.

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

/// ECMAScript ToInt32: the number truncated and wrapped to 32 bits
/// (NaN and infinities are 0), as bitwise operators read their operands.
pub fn to_int32(x: f64) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    x.trunc().rem_euclid(4_294_967_296.0) as u32 as i32
}

/// A JavaScript bitwise or shift operator on numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bitwise {
    And,
    Or,
    Xor,
    /// `<<`
    Shl,
    /// `>>` (sign-propagating)
    Shr,
    /// `>>>` (zero-fill; the result is unsigned)
    Ushr,
}

/// `a & b`, `a | 0`, `1 << n`, `x >>> 0`, … with JavaScript semantics
/// (32-bit operands, shift counts modulo 32).
pub fn bitwise(op: Bitwise, a: f64, b: f64) -> f64 {
    let x = to_int32(a);
    let y = to_int32(b);
    let count = (y as u32) & 31;
    f64::from(match op {
        Bitwise::And => x & y,
        Bitwise::Or => x | y,
        Bitwise::Xor => x ^ y,
        Bitwise::Shl => x.wrapping_shl(count),
        Bitwise::Shr => x >> count,
        Bitwise::Ushr => return f64::from((x as u32) >> count),
    })
}

/// A start position from a JavaScript relative index (`indexOf(x, from)`,
/// `slice`): truncated, negative counts from the end, clamped to
/// `0..=len`; NaN is 0.
pub fn relative_index(from: f64, len: usize) -> usize {
    let len_f = len as f64;
    let n = if from.is_nan() { 0.0 } else { from.trunc() };
    let start = if n < 0.0 {
        (len_f + n).max(0.0)
    } else {
        n.min(len_f)
    };
    start as usize
}

/// `~x`.
pub fn bit_not(x: f64) -> f64 {
    f64::from(!to_int32(x))
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
