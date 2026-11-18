//! Typed expressions, numeric operators, conditionals, strings, calls, and
//! `console.log`.
//!
//! Optionals lower to `Option` combinators and numbers to typed literals;
//! reads of non-`Copy` values clone.

use crate::ast;

use super::aggregate;
use super::builtins;
use super::functions;
use super::model;
use super::narrowing;
use super::rust_text::{self, Format, Precedence};
use super::scope;
use super::types::{self, Inference, Type, Value};
use super::{Generator, Result, fail};

mod calls;
mod operators;

pub use self::calls::*;
pub use self::operators::*;

const BOOLEAN_CONDITION_MESSAGE: &str =
    "conditions must be boolean; compare the value explicitly (for example, count > 0)";

pub fn condition(g: &mut Generator<'_>, c: &mut scope::Context, e: &ast::Expr) -> Result<Value> {
    let v = emit(g, c, e, None, false)?;
    // JavaScript truthiness (`if (name)`, `!count`)
    super::truthy::test(g, v, e.loc)
}

pub fn emit(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    expected: Option<Type>,
    child: bool,
) -> Result<Value> {
    if let Some(t) = expected {
        types::check_literal(g, e, t)?;
    }
    let v = match &e.data {
        ast::ExprData::Number(n) => number(n),
        ast::ExprData::String(s) => Value {
            ty: Type::String,
            code: format!("zant::Str::from({})", rust_text::quote_at(g, s, e.loc)?),
            text: Some(s.clone()),
            ..Value::default()
        },
        ast::ExprData::Boolean(b) => Value::new(Type::Boolean, if *b { "true" } else { "false" }),
        ast::ExprData::NullLit => Value::new(Type::Null, "None"),
        ast::ExprData::Ident(n) => {
            // a plain function as a value where a function type is expected
            let function = match expected {
                Some(ty) if scope::local(c, n).is_none() => {
                    super::closures::named(g, n, ty, e.loc)?
                }
                _ => None,
            };
            match function {
                Some(value) => value,
                None => scope::read(g, c, n, e.loc)?,
            }
        }
        ast::ExprData::Template(t) => template(g, c, t)?,
        ast::ExprData::Unary(u) => unary(g, c, e.loc, u, expected)?,
        ast::ExprData::Binary(b) => binary(g, c, e.loc, b, expected, child)?,
        ast::ExprData::Cond(cond) => conditional(g, c, e.loc, cond, expected, child)?,
        ast::ExprData::Member(m) => aggregate::member(g, c, m)?,
        ast::ExprData::Call(call) => call_value(g, c, e.loc, call, expected)?,
        ast::ExprData::Jsx(jsx) => match g.extensions.hooks.emit_jsx {
            Some(hook) => hook(g, c, jsx)?,
            None => return fail(g, e.loc, "JSX needs the JSX emitter installed"),
        },
        ast::ExprData::Arrow(arrow) => {
            // a function value where a function type is expected
            match expected.filter(|ty| super::closures::signature(g, *ty).is_some()) {
                Some(ty) => super::closures::arrow(g, c, e.loc, arrow, ty)?,
                None => {
                    return fail(
                        g,
                        e.loc,
                        "an arrow needs a function type here (`const f: (x: number) => number = …`, or annotate its parameters and return type)",
                    );
                }
            }
        }
        ast::ExprData::Object(properties) => aggregate::object(g, c, e.loc, properties, expected)?,
        ast::ExprData::Assign(_) | ast::ExprData::Update(_) => {
            return fail(
                g,
                e.loc,
                "assignments and updates are only supported as statements",
            );
        }
        ast::ExprData::NonNull(inner) => aggregate::non_null(g, c, inner, expected, child)?,
        ast::ExprData::As(cast) => explicit_cast(g, c, e.loc, cast, child)?,
        ast::ExprData::Array(items) => match static_bytes_expected(g, expected) {
            Some(bytes) => byte_slice(g, c, items, bytes)?,
            None => aggregate::array(g, c, e.loc, items, expected)?,
        },
        ast::ExprData::Index(ix) => aggregate::index(g, c, e.loc, ix)?,
        ast::ExprData::Spread(_) => {
            return fail(
                g,
                e.loc,
                "`...` spreads into array literals, object literals and a function's rest parameter (`...items: T[]`)",
            );
        }
    };
    match expected {
        Some(t) => types::coerce(g, v, t, e.loc),
        None => Ok(v),
    }
}

/// Keep a numeric literal contextual until a consumer determines its type.
fn number(n: &[u8]) -> Value {
    let code = String::from_utf8_lossy(n).into_owned();
    Value {
        ty: if types::float_literal(n) {
            Type::FloatLiteral
        } else {
            Type::IntLiteral
        },
        code,
        inference: Inference::Literal,
        ..Value::default()
    }
}

/// The `Uint8Array` type when `expected` is it (optionally wrapped in `?`).
fn static_bytes_expected(g: &Generator<'_>, expected: Option<Type>) -> Option<Type> {
    let ty = expected?;
    let inner = model::optional_child(g, ty).unwrap_or(ty);
    model::is_static_bytes(g, inner).then_some(inner)
}

/// A `Uint8Array` literal becomes a promoted slice, `&[137u8, 80u8, …]`,
/// which coerces to `&'static [u8]`
/// (`const icon: Uint8Array = [137, 80, 78, 71]`). Only integer literals are
/// const-promotable, so anything else is a diagnostic instead of code that
/// would not compile.
fn byte_slice(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    items: &[Box<ast::Expr>],
    ty: Type,
) -> Result<Value> {
    let mut parts: Vec<String> = Vec::new();
    for item in items {
        if !matches!(item.data, ast::ExprData::Number(_)) {
            return fail(
                g,
                item.loc,
                "a Uint8Array literal contains only integer byte literals; build other bytes at runtime",
            );
        }
        let value = emit(g, c, item, Some(Type::U8), false)?;
        parts.push(value.code);
    }
    let code = if parts.is_empty() {
        "&[]".to_owned()
    } else {
        format!("&[{}]", parts.join(", "))
    };
    Ok(Value {
        ty,
        code,
        precedence: Precedence::Unary,
        ..Value::default()
    })
}

pub fn template(g: &mut Generator<'_>, c: &mut scope::Context, t: &ast::Template) -> Result<Value> {
    if t.exprs.is_empty() {
        let text = t.quasis.first().cloned().unwrap_or_default();
        return Ok(Value {
            ty: Type::String,
            code: format!("zant::Str::from({})", rust_text::quote_at(g, &text, 0)?),
            text: Some(text),
            ..Value::default()
        });
    }
    let mut format = Format {
        compile_time: c.constant,
        ..Format::default()
    };
    for (index, quasi) in t.quasis.iter().enumerate() {
        format.text(quasi);
        if let Some(expr) = t.exprs.get(index) {
            let value = emit(g, c, expr, None, false)?;
            format.value(g, value, expr.loc)?;
        }
    }
    let code = format.finish(g, false)?;
    Ok(Value {
        ty: Type::String,
        code,
        formatted: Some(std::rc::Rc::new(format)),
        ..Value::default()
    })
}

/// Assertions change numeric representation explicitly, without enabling
/// general arrays or optionals.
fn explicit_cast(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    cast: &ast::As,
    child: bool,
) -> Result<Value> {
    let target = types::type_ref(g, &cast.ty, false)?;
    // `satisfies` checks assignability and keeps the value unchanged; `[…]
    // as T` / `{…} as T` give the literal T as its context, as in TS
    let literal = matches!(
        cast.expr.data,
        ast::ExprData::Array(_) | ast::ExprData::Object(_)
    ) || super::json::takes_context(&cast.expr);
    // `{ red: 1 } satisfies Record<string, number>`: the object keeps its
    // own type (`PALETTE.red` is a number), each field checked against the
    // record's value type
    if cast.satisfies
        && matches!(cast.expr.data, ast::ExprData::Object(_))
        && super::dicts::kind(g, target) == Some(model::InfoKind::Record)
    {
        let value = emit(g, c, &cast.expr, None, child)?;
        let value_type = model::info(g, target).map(|info| info.child);
        let fields = model::info(g, value.ty)
            .map(|info| info.fields.clone())
            .unwrap_or_default();
        if let Some(value_type) = value_type {
            for field in fields {
                types::coerce(g, Value::new(field.ty, "_field"), value_type, loc)?;
            }
        }
        return Ok(value);
    }
    // `source satisfies Base`: checked against Base, the value keeps its own
    // type (and its extra fields)
    if cast.satisfies && !literal {
        let value = emit(g, c, &cast.expr, None, child)?;
        types::coerce(g, value.clone(), target, loc)?;
        return Ok(value);
    }
    if cast.satisfies || literal {
        return emit(g, c, &cast.expr, Some(target), child);
    }
    let value = emit(g, c, &cast.expr, None, child)?;
    // `r.json() as Promise<Item[]>`
    if let Some(decoded) = super::promises::cast(g, &value, target)? {
        return Ok(decoded);
    }
    types::explicit_cast(g, value, target, loc)
}
