//! `RegExp`: literals (`/a+/g`, parsed as `new RegExp("a+", "g")`),
//! `new RegExp(text, flags)`, `test` / `exec`, the match arrays and the
//! string methods that take a pattern (`match`, `matchAll`, `search`,
//! `replace`, `replaceAll`, `split`). The engine is `regress` (ECMAScript
//! semantics; `zant::tsx::regexp`).
//!
//! A literal pattern is checked when compiling (a bad one is a compile
//! error) and compiled once per site, like JS engines cache literals: each
//! evaluation is a new object (its own `lastIndex`) sharing the program.
//! `new RegExp(text)` with a computed pattern can throw a `SyntaxError`.
//!
//! Differences from JavaScript, all from typing: `m[i]` is a `string` (as
//! TypeScript types it) and an unmatched capture reads as `""`; a global
//! `match` result has no `index` (NaN). `matchAll` / `replaceAll` with a
//! non-global pattern held in a variable report the TypeError as uncaught
//! and yield nothing (a literal without `g` is a compile error).

use crate::ast;

use super::aggregate;
use super::expr;
use super::model::{self, Info, InfoKind};
use super::rust_text::{self, Precedence};
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

mod strings;
#[cfg(test)]
mod tests;
mod values;

pub use self::strings::*;
pub use self::values::*;

const REGEXP: &str = "zant::tsx::RegExp";

const MATCH: &str = "zant::tsx::regexp::MatchArray";

fn find(g: &Generator<'_>, rust: &str) -> Option<Type> {
    g.compounds
        .iter()
        .position(|entry| entry.rust == rust)
        .map(|index| Type::Compound(index as u32))
}

fn opaque(g: &mut Generator<'_>, rust: &str) -> Result<Type> {
    if let Some(ty) = find(g, rust) {
        return Ok(ty);
    }
    model::add(
        g,
        Info {
            kind: InfoKind::Object,
            rust: rust.to_owned(),
            ..Info::default()
        },
    )
}

pub fn regexp_type(g: &mut Generator<'_>) -> Result<Type> {
    opaque(g, REGEXP)
}

pub fn match_type(g: &mut Generator<'_>) -> Result<Type> {
    opaque(g, MATCH)
}

pub fn is_regexp(g: &Generator<'_>, ty: Type) -> bool {
    find(g, REGEXP) == Some(ty)
}

pub fn is_match(g: &Generator<'_>, ty: Type) -> bool {
    find(g, MATCH) == Some(ty)
}

/// `RegExp` / `RegExpMatchArray` / `RegExpExecArray` in a type position.
pub fn named_type(g: &mut Generator<'_>, name: &[u8]) -> Result<Option<Type>> {
    if g.type_decls.iter().any(|decl| decl.name.name == name) {
        return Ok(None);
    }
    match name {
        b"RegExp" => regexp_type(g).map(Some),
        b"RegExpMatchArray" | b"RegExpExecArray" => match_type(g).map(Some),
        _ => Ok(None),
    }
}

/// The literal text of a string argument.
fn literal(e: &ast::Expr) -> Option<String> {
    match &e.data {
        ast::ExprData::String(bytes) => String::from_utf8(bytes.clone()).ok(),
        _ => None,
    }
}

/// `new RegExp(pattern, flags)` whose arguments are all string literals
/// (a regex literal): `Some((pattern, flags))`.
pub fn literal_regexp(call: &ast::Call) -> Option<(String, String)> {
    if !matches!(&call.callee.data, ast::ExprData::Ident(n) if n == b"RegExp") {
        return None;
    }
    let pattern = literal(call.args.first()?)?;
    let flags = match call.args.get(1) {
        Some(arg) => literal(arg)?,
        None => String::new(),
    };
    (call.args.len() <= 2).then_some((pattern, flags))
}

/// Whether `call` builds a RegExp from a computed pattern (it can throw a
/// `SyntaxError`; `throws` asks).
pub fn computed_construction(call: &ast::Call) -> bool {
    matches!(&call.callee.data, ast::ExprData::Ident(n) if n == b"RegExp")
        && literal_regexp(call).is_none()
}

/// A literal pattern, checked now and compiled once per site.
fn cached(g: &mut Generator<'_>, loc: ast::Loc, pattern: &str, flags: &str) -> Result<Value> {
    // the runtime's `RegExp` constructor compiles with the same check
    if let Err(message) = zant_base::regexp::compile(pattern, flags) {
        return fail(g, loc, format!("SyntaxError: {message}"));
    }
    let ty = regexp_type(g)?;
    Ok(Value::new(
        ty,
        format!(
            "{{ thread_local! {{ static _REGEXP: {REGEXP} = {REGEXP}::new({pattern:?}, {flags:?}).expect(\"checked when compiled\"); }} _REGEXP.with({REGEXP}::fresh) }}"
        ),
    ))
}

/// A pattern made from a string literal (`s.match("a.b")`: JavaScript
/// treats the text as a pattern).
pub fn from_text(
    g: &mut Generator<'_>,
    loc: ast::Loc,
    pattern: &str,
    flags: &str,
) -> Result<Value> {
    cached(g, loc, pattern, flags)
}

/// `new RegExp(…)` / `RegExp(…)`; `None` for other calls.
pub fn construct(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<Value>> {
    let ast::ExprData::Ident(name) = &call.callee.data else {
        return Ok(None);
    };
    if name != b"RegExp" || scope::local(c, name).is_some() {
        return Ok(None);
    }
    if call.args.is_empty() || call.args.len() > 2 {
        return fail(g, loc, "`new RegExp(pattern, flags?)`");
    }
    if let Some((pattern, flags)) = literal_regexp(call) {
        return cached(g, loc, &pattern, &flags).map(Some);
    }
    let mut parts = Vec::new();
    for arg in &call.args {
        let value = expr::emit(g, c, arg, Some(Type::String), false)?;
        if is_regexp(g, value.ty) {
            return fail(
                g,
                arg.loc,
                "copy a RegExp with `new RegExp(re.source, re.flags)`",
            );
        }
        let value = types::coerce(g, value, Type::String, arg.loc)?;
        parts.push(format!(
            "{}.as_str()",
            rust_text::operand(&value, Precedence::Suffix, false)
        ));
    }
    if parts.len() == 1 {
        parts.push("\"\"".to_owned());
    }
    let ty = regexp_type(g)?;
    let value = Value::new(ty, format!("{REGEXP}::new({})", parts.join(", ")));
    super::exceptions::checked_call(g, c, value, loc).map(Some)
}
