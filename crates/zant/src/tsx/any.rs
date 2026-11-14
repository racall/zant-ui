//! `zant::tsx::Any`: a dynamically typed value for the few places generated
//! code loses the TypeScript type (`console.log`, dynamic props).
//!
//! The compiler only needs such a value when it cannot infer a type; the
//! codegen uses [`Any`] for those edge cases. The variants are `Str`, `f64`
//! and `bool`.

use crate::Str;

/// A TypeScript value with no static type (TS `any`).
#[derive(Clone, Debug, PartialEq)]
pub enum Any {
    Str(Str),
    /// TS numbers are f64; integers are widened (TS `any` does the same).
    Number(f64),
    Bool(bool),
}

impl Any {
    pub fn str(value: impl Into<Str>) -> Any {
        Any::Str(value.into())
    }

    pub fn number(value: f64) -> Any {
        Any::Number(value)
    }

    pub fn bool(value: bool) -> Any {
        Any::Bool(value)
    }

    /// The value as TS `String(value)` spells it (JS float rules).
    pub fn to_js_string(&self) -> Str {
        match self {
            Any::Str(value) => value.clone(),
            Any::Number(value) => super::builtins::string(*value),
            Any::Bool(value) => Str::from(if *value { "true" } else { "false" }),
        }
    }
}

impl std::fmt::Display for Any {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Any::Str(value) => formatter.write_str(value),
            Any::Number(value) => formatter.write_str(&super::builtins::string(*value)),
            Any::Bool(value) => formatter.write_str(if *value { "true" } else { "false" }),
        }
    }
}

impl From<&str> for Any {
    fn from(value: &str) -> Any {
        Any::Str(Str::copy(value))
    }
}

impl From<String> for Any {
    fn from(value: String) -> Any {
        Any::Str(Str::from(value))
    }
}

impl From<Str> for Any {
    fn from(value: Str) -> Any {
        Any::Str(value)
    }
}

impl From<bool> for Any {
    fn from(value: bool) -> Any {
        Any::Bool(value)
    }
}

impl From<f64> for Any {
    fn from(value: f64) -> Any {
        Any::Number(value)
    }
}

impl From<f32> for Any {
    fn from(value: f32) -> Any {
        Any::Number(f64::from(value))
    }
}

macro_rules! any_from_int {
    ($($ty:ty),* $(,)?) => {
        $(
            impl From<$ty> for Any {
                fn from(value: $ty) -> Any {
                    Any::Number(value as f64)
                }
            }
        )*
    };
}

any_from_int!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_values_print_like_javascript() {
        assert_eq!(Any::from("x").to_string(), "x");
        assert_eq!(Any::from(3u32).to_string(), "3");
        assert_eq!(Any::from(1.0f64).to_string(), "1");
        assert_eq!(Any::from(-0.0f64).to_string(), "0");
        assert_eq!(Any::from(1e21f64).to_string(), "1e+21");
        assert_eq!(Any::from(f64::NAN).to_string(), "NaN");
        assert_eq!(Any::from(f64::INFINITY).to_string(), "Infinity");
        assert_eq!(Any::from(true).to_string(), "true");
        assert_eq!(Any::from(false).to_string(), "false");
    }

    #[test]
    fn any_equality_is_by_value() {
        assert_eq!(Any::from("a"), Any::from(String::from("a")));
        assert_ne!(Any::from(1u8), Any::from(true));
    }
}
