//! Immutable array/object values, spreads, indexing, and optional member
//! access.
//!
//! Arrays become `tsx::List::from_vec(..)`, with a `Vec` block for spreads;
//! `??` and optional member access become `unwrap_or_else` / `.map(..)`.

use crate::ast;

use super::builtins;
use super::expr;
use super::model;
use super::rust_text::{self, Precedence};
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

mod access;
mod literals;

pub use self::access::*;
pub use self::literals::*;

pub fn unwrap(g: &Generator<'_>, ty: Type) -> Type {
    model::optional_child(g, ty).unwrap_or(ty)
}

/// Compiler temporaries are ordinary bindings, so they cannot shadow source
/// locals.
pub fn temporary(g: &mut Generator<'_>, c: &mut scope::Context, base: &[u8]) -> Result<String> {
    let binding = scope::bind(g, c, base, Type::Void, false)?;
    c.bindings[binding].name.clear();
    Ok(c.bindings[binding].rust.clone())
}

pub fn non_null(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    inner: &ast::Expr,
    expected: Option<Type>,
    child: bool,
) -> Result<Value> {
    if types::numeric_literals(inner) {
        return expr::emit(g, c, inner, expected, child);
    }
    let value = expr::emit(g, c, inner, None, child)?;
    if let Some(ty) = model::optional_child(g, value.ty) {
        return Ok(Value {
            ty,
            code: format!(
                "{}.unwrap()",
                rust_text::operand(&value, Precedence::Suffix, false)
            ),
            precedence: Precedence::Suffix,
            ..Value::default()
        });
    }
    match expected {
        Some(target) => types::coerce(g, value, target, inner.loc),
        None => Ok(value),
    }
}

pub fn coalesce(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    binary: &ast::Binary,
    expected: Option<Type>,
) -> Result<Value> {
    let value = expr::emit(g, c, &binary.lhs, None, false)?;
    if value.ty == Type::Null {
        let mut fallback = expr::emit(g, c, &binary.rhs, expected, false)?;
        if value.code == "None" {
            return Ok(fallback);
        }
        let target = expected.unwrap_or_else(|| fallback.ty.concrete());
        fallback = types::coerce(g, fallback, target, binary.rhs.loc)?;
        return Ok(Value {
            ty: fallback.ty,
            code: fallback.code,
            precedence: Precedence::LogicalOr,
            ..Value::default()
        });
    }
    let Some(child) = model::optional_child(g, value.ty) else {
        return fail(g, loc, "?? needs an optional value");
    };
    // `find(…) ?? null`: still `T | null`
    if matches!(binary.rhs.data, ast::ExprData::NullLit) {
        return Ok(value);
    }
    // an optional fallback where an optional is expected: `a ?? b` is
    // `T | null` again
    if expected.is_some_and(|ty| model::optional_child(g, ty).is_some()) {
        let fallback = expr::emit(g, c, &binary.rhs, Some(value.ty), false)?;
        let fallback = types::coerce(g, fallback, value.ty, binary.rhs.loc)?;
        return Ok(Value {
            ty: value.ty,
            code: format!(
                "{}.or_else(|| {})",
                rust_text::operand(&value, Precedence::Suffix, false),
                fallback.code
            ),
            precedence: Precedence::Suffix,
            ..Value::default()
        });
    }
    let fallback = expr::emit(g, c, &binary.rhs, Some(child), false)?;
    Ok(Value {
        ty: child,
        code: format!(
            "{}.unwrap_or_else(|| {})",
            rust_text::operand(&value, Precedence::Suffix, false),
            fallback.code
        ),
        precedence: Precedence::Suffix,
        ..Value::default()
    })
}
