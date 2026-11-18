//! `useRef`, `useMemo` and `useEffect` (`docs/tsx.md`, "第三阶段").
//! They compile to plain state fields:
//!
//! - `const r = useRef(0)`: a field read and written directly (`self.r`); a
//!   write reads the live value, no setter and no handler snapshot.
//! - `const v = useMemo(() => f(a), [a, b])`: an `Option<(deps, value)>` field;
//!   the body runs only when the dependency tuple changed.
//! - `useEffect(() => { … }, [a])`: a method queued with `ui::effect` when the
//!   tuple changed (`[]`: once on mount; no list: after every build). A
//!   `return () => …` cleanup keeps its `ui::Action` in a second field and is
//!   queued before the body re-runs (`ui::effect` in order, since Rust has no
//!   `tsx.Effect`).

use crate::ast;
use crate::tags;

use super::aggregate;
use super::callbacks;
use super::captures;
use super::expr;
use super::model;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

mod effects;
mod memo;
mod refs;
#[cfg(test)]
mod tests;

pub use self::effects::*;
pub use self::memo::*;
pub use self::refs::*;

/// A hidden field of the component (memo cache, effect dependencies).
#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub rust_type: String,
}

#[derive(Clone, Debug)]
pub struct Effect {
    /// The `useEffect(…)` call expression (`ast::Call` equality is structural:
    /// the Rust AST has no stable node ids).
    pub call: ast::Call,
    pub method: usize,
    /// Has `return () => …` (a cleanup `ui::Action` field).
    pub cleanup: bool,
    pub consumed: bool,
}

/// A cleanup arrow and its method.
#[derive(Clone, Debug)]
pub struct Cleanup {
    pub arrow: ast::Expr,
    pub method: usize,
}

/// `useRef` / `useMemo` / `useEffect` (never renamed).
pub fn extra_hook(name: &str) -> bool {
    name == "useRef" || name == "useMemo" || name == "useEffect"
}

/// The called name of a hook expression (`useEffect` of `useEffect(…)`).
pub fn call_name(e: &ast::Expr) -> Option<String> {
    let ast::ExprData::Call(call) = &e.data else {
        return None;
    };
    let ast::ExprData::Ident(name) = &call.callee.data else {
        return None;
    };
    Some(String::from_utf8_lossy(name).into_owned())
}

// ── useRef ────────────────────────────────────────────────────────────

/// The Rust field name of a hidden hook field (`_memo1`).
pub fn field_name(g: &mut Generator<'_>, field: &Field) -> Result<String> {
    scope::state_field(g, field.name.as_bytes())
}
