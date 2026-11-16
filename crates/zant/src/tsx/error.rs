//! Thrown values of TSX `throw` / `try` / `catch`.
//!
//! The compiler lowers exceptions the way Swift lowers `throws`: a function
//! whose body can let a throw escape returns `Result<T, Thrown>` and its
//! callers propagate or catch the error value; there is no unwinding, so it
//! works with `panic = "abort"` and never crosses the platform's callback
//! frames. `finally` runs through a [`Completion`] record (the ECMAScript
//! spec's completion records): how the protected block ended is kept while
//! the `finally` block runs, then resumed.
//!
//! What a `catch (e)` binding holds is [`Thrown`]: TypeScript types it
//! `unknown`, and JavaScript can throw any value, so primitives keep their
//! own form (`throw "busy"` is caught as the string `"busy"`).

use std::fmt;
use std::rc::Rc;

use crate::Str;

/// An `Error` object (`new Error("…")`, `new TypeError("…")`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Error {
    /// `"Error"`, `"TypeError"`, `"RangeError"`, `"SyntaxError"`, …
    pub name: Str,
    pub message: Str,
}

impl Error {
    pub fn new(name: &'static str, message: Str) -> Self {
        Error {
            name: Str::Static(name),
            message,
        }
    }
}

impl fmt::Display for Error {
    /// `Error.prototype.toString`: `"TypeError: bad input"`, or just the
    /// name when the message is empty.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.message.is_empty() {
            f.write_str(&self.name)
        } else if self.name.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{}: {}", self.name, self.message)
        }
    }
}

/// A thrown value, as a `catch` binding sees it.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Thrown {
    Error(Rc<Error>),
    String(Str),
    Number(f64),
    Boolean(bool),
    #[default]
    Null,
}

impl Thrown {
    pub fn error(error: Error) -> Self {
        Thrown::Error(Rc::new(error))
    }

    /// `e instanceof Error` (any error kind).
    pub fn is_error(&self) -> bool {
        matches!(self, Thrown::Error(_))
    }

    /// `e instanceof TypeError`: errors created with that constructor.
    pub fn is_error_named(&self, name: &str) -> bool {
        matches!(self, Thrown::Error(error) if &*error.name == name)
    }

    /// The error inside, after `instanceof Error` narrowed `e`.
    pub fn as_error(&self) -> Error {
        match self {
            Thrown::Error(error) => (**error).clone(),
            _ => Error::default(),
        }
    }

    /// The string inside, after `typeof e === "string"` narrowed `e`.
    pub fn as_string(&self) -> Str {
        match self {
            Thrown::String(text) => text.clone(),
            _ => Str::Static(""),
        }
    }

    /// The number inside, after `typeof e === "number"` narrowed `e`.
    pub fn as_number(&self) -> f64 {
        match self {
            Thrown::Number(value) => *value,
            _ => f64::NAN,
        }
    }

    /// `typeof e`.
    pub fn type_of(&self) -> Str {
        Str::Static(match self {
            Thrown::Error(_) | Thrown::Null => "object",
            Thrown::String(_) => "string",
            Thrown::Number(_) => "number",
            Thrown::Boolean(_) => "boolean",
        })
    }

    /// `String(e)` / `` `${e}` ``.
    pub fn to_str(&self) -> Str {
        match self {
            Thrown::String(text) => text.clone(),
            other => Str::from(other.to_string()),
        }
    }
}

impl fmt::Display for Thrown {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Thrown::Error(error) => error.fmt(f),
            Thrown::String(text) => f.write_str(text),
            Thrown::Number(value) => {
                f.write_str(&crate::tsx::builtins_format::js_float_string(*value))
            }
            Thrown::Boolean(value) => write!(f, "{value}"),
            Thrown::Null => f.write_str("null"),
        }
    }
}

impl From<Error> for Thrown {
    fn from(error: Error) -> Self {
        Thrown::error(error)
    }
}

impl From<Str> for Thrown {
    fn from(text: Str) -> Self {
        Thrown::String(text)
    }
}

impl From<f64> for Thrown {
    fn from(value: f64) -> Self {
        Thrown::Number(value)
    }
}

impl From<bool> for Thrown {
    fn from(value: bool) -> Self {
        Thrown::Boolean(value)
    }
}

/// How a `try` / `catch` block with a `finally` ended; the `finally` block
/// runs, then the completion resumes (unless `finally` itself left).
#[derive(Debug)]
pub enum Completion<R> {
    Normal,
    Return(R),
    Throw(Thrown),
    /// A `break` / `continue` to a loop outside the `try` (the generated
    /// code numbers the jumps of one `try`).
    Jump(u32),
}

/// A throw that escaped an event handler, effect or timer: reported like a
/// browser's uncaught error, and the app keeps running.
pub fn uncaught(thrown: &Thrown) {
    eprintln!("Uncaught {}", uncaught_text(thrown));
}

fn uncaught_text(thrown: &Thrown) -> String {
    match thrown {
        Thrown::String(text) => format!("{:?}", &**text),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_print_like_error_to_string() {
        let error = Error::new("TypeError", Str::from("bad input"));
        assert_eq!("TypeError: bad input", error.to_string());
        assert_eq!("Error", Error::new("Error", Str::Static("")).to_string());
        let thrown = Thrown::from(error);
        assert!(thrown.is_error());
        assert!(thrown.is_error_named("TypeError"));
        assert!(!thrown.is_error_named("RangeError"));
        assert_eq!("bad input", &*thrown.as_error().message);
        assert_eq!("object", &*thrown.type_of());
    }

    #[test]
    fn primitives_keep_their_form() {
        assert_eq!("busy", &*Thrown::from(Str::from("busy")).to_str());
        assert_eq!("42", &*Thrown::from(42.0).to_str());
        assert_eq!("1.5", &*Thrown::from(1.5).to_str());
        assert_eq!("false", &*Thrown::from(false).to_str());
        assert_eq!("null", &*Thrown::Null.to_str());
        assert_eq!("string", &*Thrown::from(Str::from("x")).type_of());
        assert!(!Thrown::from(1.0).is_error());
        assert_eq!("\"busy\"", uncaught_text(&Thrown::from(Str::from("busy"))));
    }
}
