//! JavaScript's `String(number)` spelling.

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

#[cfg(test)]
mod tests {
    use super::write_js_number;

    fn spell(x: f64) -> String {
        let mut out = String::new();
        write_js_number(&mut out, x);
        out
    }

    #[test]
    fn numbers_are_spelled_like_javascript() {
        assert_eq!(spell(0.0), "0");
        assert_eq!(spell(-0.0), "0");
        assert_eq!(spell(42.0), "42");
        assert_eq!(spell(-3.0), "-3");
        assert_eq!(spell(1.5), "1.5");
        assert_eq!(spell(1e21), "1e+21");
        assert_eq!(spell(0.000001), "0.000001");
        assert_eq!(spell(1e-7), "1e-7");
        assert_eq!(spell(f64::NAN), "NaN");
    }
}
