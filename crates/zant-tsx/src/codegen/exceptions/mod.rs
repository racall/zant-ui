//! `throw`, `try` / `catch` / `finally`, `Error` values.
//!
//! Lowered the way Swift lowers `throws` (no unwinding; see
//! `zant::tsx::error`):
//!
//! - A function that can let a throw escape (`throws`, inferred) returns
//!   `Result<T, tsx::Thrown>`; its `return x` is `return Ok(x)` and a call of
//!   it is `f(..)?` — or, inside a `try`, a `match` whose `Err` arm jumps to
//!   the handler.
//! - A `try` region is a labeled block; `throw` inside it is
//!   `break 'try Err(value)`. With a `finally`, every way out of the region
//!   (normal end, throw, `return`, a `break` / `continue` to an outer loop)
//!   is a `tsx::Completion` value — the ECMAScript spec's completion record:
//!   the `finally` block runs, then the completion resumes. An abrupt
//!   `finally` (its own `return` / `throw`) overrides it.
//! - A throw nobody catches leaves an event handler / effect / timer as an
//!   uncaught error report (the app keeps running, like a browser); a
//!   component cannot throw while it renders (a diagnostic), and neither can
//!   an arrow passed as a value (catch inside it).
//!
//! `catch (e)` binds `e: unknown` (`tsx::Thrown`): `e instanceof Error`
//! narrows it to an `Error` (`e.message`, `e.name`), `typeof e === "string"`
//! to a string; `String(e)` / `` `${e}` `` show any thrown value.

use crate::ast;

use super::aggregate;
use super::expr;
use super::model::{self, Field, Info, InfoKind};
use super::narrowing::{self, Narrow};
use super::rust_text::{self, Precedence};
use super::scope;
use super::stmt;
use super::types::{Type, Value};
use super::{Generator, Result, fail};

mod errors;
mod throws;
mod try_catch;

pub use self::errors::*;
pub use self::throws::*;
pub use self::try_catch::*;

const THROWN: &str = "zant::tsx::Thrown";

const ERROR: &str = "zant::tsx::Error";

const COMPLETION: &str = "zant::tsx::Completion";

/// The error constructors (`new TypeError("…")`); `instanceof` tests them.
pub const ERROR_NAMES: [&str; 7] = [
    "Error",
    "TypeError",
    "RangeError",
    "SyntaxError",
    "ReferenceError",
    "EvalError",
    "URIError",
];

/// Where a throw outside every `try` of a body goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThrowMode {
    /// Nowhere: a diagnostic (render, arrows passed as values).
    #[default]
    Forbid,
    /// Out of the function (`return Err(..)`): a throwing function.
    Propagate,
    /// Reported as uncaught, then the handler returns (event handlers).
    Report,
}

/// A `try` region being emitted.
#[derive(Clone, Debug)]
pub struct Frame {
    pub label: String,
    /// A `finally` follows: exits leave through a `Completion`.
    pub finally: bool,
    /// `c.jump_labels.len()` when the region opened: labels below it are
    /// outside the region.
    pub jump_depth: usize,
    /// The jumps leaving it, numbered (`Completion::Jump(n)`): `(break?,
    /// label)`.
    pub jumps: Vec<(bool, String)>,
}

// ── types ────────────────────────────────────────────────────────────
