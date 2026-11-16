/// UTS #35 operands derived from an absolute decimal value.
///
/// Integer and fraction digits are retained exactly, including visible trailing
/// zeros: `n` is the absolute value, `i` its integer part, `v` / `w` the fraction
/// lengths with / without trailing zeros, `f` / `t` those digits as integers,
/// and `c` (also called `e` in rules) the compact exponent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Operands {
    pub(super) integer: String,
    pub(super) fraction: String,
    pub(super) c: u16,
    pub(super) finite: bool,
}

impl Operands {
    /// Parse a signed decimal spelling, preserving its visible fraction digits.
    ///
    /// Accepts scientific `e` / `E` notation and CLDR compact samples such as
    /// `1.20050c3` (equivalent to `1200.50`, with `c = 3`). Scientific notation
    /// expands the decimal with `c = 0`; only compact `c` sets that operand.
    /// Exponents must fit in i16; compact exponents must be nonnegative.
    /// Invalid spellings, NaN, and infinity return `None`.
    pub fn from_decimal(text: &str) -> Option<Self> {
        let text = text.strip_prefix(['+', '-']).unwrap_or(text);
        let (mantissa, exponent, compact) = match text.find(['e', 'E', 'c']) {
            Some(at) => {
                let exponent = text[at + 1..].parse::<i16>().ok()?;
                let compact = text.as_bytes()[at] == b'c';
                if compact && exponent < 0 {
                    return None;
                }
                (&text[..at], exponent, compact)
            }
            None => (text, 0, false),
        };
        let (integer, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        if integer.is_empty()
            || !integer.bytes().all(|b| b.is_ascii_digit())
            || !fraction.bytes().all(|b| b.is_ascii_digit())
            || (mantissa.contains('.') && fraction.is_empty())
        {
            return None;
        }
        let mut digits = format!("{integer}{fraction}");
        let point = integer.len() as i64 + i64::from(exponent);
        let (integer, fraction) = if point <= 0 {
            (
                "0".into(),
                format!("{}{digits}", "0".repeat((-point) as usize)),
            )
        } else if point as usize >= digits.len() {
            digits.push_str(&"0".repeat(point as usize - digits.len()));
            (digits, String::new())
        } else {
            let fraction = digits.split_off(point as usize);
            (digits, fraction)
        };
        let integer = integer.trim_start_matches('0');
        Some(Self {
            integer: if integer.is_empty() { "0" } else { integer }.into(),
            fraction,
            c: if compact { exponent as u16 } else { 0 },
            finite: true,
        })
    }

    /// Use ECMAScript `String(value)` spelling before deriving the operands.
    ///
    /// This uses the shortest round-trip decimal, removes integer `.0`, and
    /// switches to exponents below 1e-6 or at/above 1e21. Negative zero is `0`.
    /// NaN and either infinity produce operands selecting `Other` in all locales.
    pub fn from_f64(value: f64) -> Self {
        Self::from_decimal(&js_string(value)).unwrap_or_else(|| Self {
            integer: String::new(),
            fraction: String::new(),
            c: 0,
            finite: false,
        })
    }

    // Equality ranges contain integers only. A nonintegral n never belongs to
    // them, even after modulo. None also represents integers larger than u64;
    // modulo is computed digit by digit before narrowing, without precision loss.
    pub(super) fn integer_value(&self, operand: u8, modulus: Option<u64>) -> Option<u64> {
        let trimmed = self.fraction.trim_end_matches('0');
        let digits = match operand {
            b'n' if !trimmed.is_empty() => return None,
            b'n' | b'i' => self.integer.as_str(),
            b'f' => self.fraction.as_str(),
            b't' => trimmed,
            b'v' | b'w' | b'c' | b'e' => {
                let value = match operand {
                    b'v' => self.fraction.len() as u64,
                    b'w' => trimmed.len() as u64,
                    _ => u64::from(self.c),
                };
                return Some(modulus.map_or(value, |m| value % m));
            }
            _ => unreachable!("parser validates operands"),
        };
        digits.bytes().try_fold(0_u64, |value, digit| {
            let next = u128::from(value) * 10 + u128::from(digit - b'0');
            u64::try_from(modulus.map_or(next, |m| next % u128::from(m))).ok()
        })
    }
}

pub(super) fn js_string(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value == 0.0 {
        return "0".into();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-Infinity"
        } else {
            "Infinity"
        }
        .into();
    }
    // serde_json supplies shortest, correctly rounded digits (including even
    // tie breaking). Normalize only notation to ECMAScript's decimal thresholds.
    let raw = serde_json::to_string(&value.abs()).expect("finite number serializes");
    let (mantissa, exponent) = raw.split_once('e').map_or((raw.as_str(), 0), |(m, e)| {
        (m, e.parse::<i32>().expect("serialized exponent"))
    });
    let mut digits = mantissa.replace('.', "");
    let leading = digits.bytes().take_while(|&b| b == b'0').count();
    let point = mantissa.find('.').unwrap_or(mantissa.len()) as i32 + exponent - leading as i32;
    digits.drain(..leading);
    let digits = digits.trim_end_matches('0');
    let sign = if value.is_sign_negative() { "-" } else { "" };
    if !(1e-6..1e21).contains(&value.abs()) {
        let fraction = if digits.len() == 1 {
            "".into()
        } else {
            format!(".{}", &digits[1..])
        };
        format!("{sign}{}{fraction}e{:+}", &digits[..1], point - 1)
    } else if point <= 0 {
        format!("{sign}0.{}{digits}", "0".repeat((-point) as usize))
    } else if point as usize >= digits.len() {
        format!(
            "{sign}{digits}{}",
            "0".repeat(point as usize - digits.len())
        )
    } else {
        format!(
            "{sign}{}.{}",
            &digits[..point as usize],
            &digits[point as usize..]
        )
    }
}
