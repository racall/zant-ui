//! Array operations lower to Rust iterator adapters over `tsx::List<T>`.
//! Callbacks are emitted as closures inside `.map` / `.filter_map` / `.any` /
//! `.all` / `.fold`, and the write-on-copy list (`Rc<Vec<T>>`) is rebuilt with
//! `tsx::List::from_vec`.

use crate::ast;

use super::aggregate;
use super::callbacks;
use super::expr;
use super::model;
use super::scope;
use super::string_methods;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

mod loops;
mod plain;
#[cfg(test)]
mod tests;

use self::loops::*;
use self::plain::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Map,
    Filter,
    Find,
    FindIndex,
    Some,
    Every,
    Reduce,
    IndexOf,
    Includes,
    Slice,
    Join,
    Concat,
}

fn lookup(name: &[u8]) -> Option<Kind> {
    Some(match name {
        b"map" => Kind::Map,
        b"filter" => Kind::Filter,
        b"find" => Kind::Find,
        b"findIndex" => Kind::FindIndex,
        b"some" => Kind::Some,
        b"every" => Kind::Every,
        b"reduce" => Kind::Reduce,
        b"indexOf" => Kind::IndexOf,
        b"includes" => Kind::Includes,
        b"slice" => Kind::Slice,
        b"join" => Kind::Join,
        b"concat" => Kind::Concat,
        _ => return None,
    })
}

pub fn emit(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    expected: Option<Type>,
) -> Result<Value> {
    let ast::ExprData::Member(member) = &call.callee.data else {
        unreachable!("array method call")
    };
    if let Some(value) = super::dicts::mutating_method(g, c, loc, call, member)? {
        return Ok(value);
    }
    if super::array_mutations::is_mutating(&member.name.name) {
        return super::array_mutations::emit(g, c, loc, call, member);
    }
    super::place::project(c, &member.object);
    let value = expr::emit(g, c, &member.object, None, false)?;
    // `title?.trim()` / `items?.map(…)`: absent stays absent, like
    // TypeScript's optional chaining (the result is optional; use `??`)
    if (member.optional || super::aggregate::in_optional_chain(&member.object))
        && let Some(child) = model::optional_child(g, value.ty)
    {
        let inner = emit_on(g, c, loc, call, expected, Value::new(child, "_some"))?;
        if inner.ty == Type::Void {
            return Ok(Value {
                ty: Type::Void,
                code: format!("if let Some(_some) = {} {{ {}; }}", value.code, inner.code),
                ..Value::default()
            });
        }
        let ty = if model::optional_child(g, inner.ty).is_some() {
            inner.ty
        } else {
            model::optional(g, inner.ty)?
        };
        let map = if ty == inner.ty { "and_then" } else { "map" };
        return Ok(Value {
            ty,
            code: format!("({}).{map}(|_some| {})", value.code, inner.code),
            ..Value::default()
        });
    }
    emit_on(g, c, loc, call, expected, value)
}

/// A string or array method on the evaluated receiver `value`.
pub(super) fn emit_on(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    expected: Option<Type>,
    value: Value,
) -> Result<Value> {
    let ast::ExprData::Member(member) = &call.callee.data else {
        unreachable!("array method call")
    };
    if value.ty == Type::String {
        return string_methods::emit(g, c, loc, call, value);
    }
    if let Some(result) = super::regexps::method(g, c, loc, call, &value)? {
        return Ok(result);
    }
    if let Some(result) = super::promises::method(g, c, loc, call, &value)? {
        return Ok(result);
    }
    if let Some(result) = super::dicts::read_method(g, c, loc, call, member, &value)? {
        return Ok(result);
    }
    // `op.run(x)`: a function stored in a field
    if let Some(info) = model::info(g, value.ty).filter(|info| info.kind == model::InfoKind::Object)
        && let Some(field) = info
            .fields
            .iter()
            .find(|field| field.name == member.name.name)
        && super::closures::signature(g, field.ty).is_some()
    {
        let rust = match &field.rust {
            Some(rust) => rust.clone(),
            None => super::rust_text::id_at(g, &field.name, member.name.loc)?,
        };
        let callee = Value::new(
            field.ty,
            format!(
                "{}.{rust}.clone()",
                super::rust_text::operand(&value, super::rust_text::Precedence::Suffix, false)
            ),
        );
        return super::closures::call_value(g, c, loc, callee, &call.args);
    }
    if let Some(optional) = model::optional_child(g, value.ty) {
        let message = if optional == Type::String {
            "optional value: call the method with `?.` (`value?.trim()`), or use ?? / !"
        } else {
            "optional array: call the method with `?.` (`items?.map(…)`), or use ?? / !"
        };
        return fail(g, loc, message);
    }
    let Some(child) = model::element(g, value.ty) else {
        return fail(g, loc, "array method needs an array");
    };
    let Some(kind) = lookup(&member.name.name) else {
        return fail(
            g,
            member.name.loc,
            format!(
                "unknown array method `{}`",
                String::from_utf8_lossy(&member.name.name)
            ),
        );
    };
    if c.constant {
        return fail(g, loc, "array methods cannot run in a top-level constant");
    }
    // Evaluate the receiver once, in source order (the `_items` temporary).
    let source = aggregate::temporary(g, c, b"_items")?;
    let setup = format!("let {source}: {} = {};", value.ty.rust(g), value.code);
    let (ty, code) = match kind {
        Kind::Map
        | Kind::Filter
        | Kind::Find
        | Kind::FindIndex
        | Kind::Some
        | Kind::Every
        | Kind::Reduce => loop_method(g, c, loc, kind, call, &source, child, expected)?,
        Kind::IndexOf | Kind::Includes => search(g, c, loc, kind, call, &source, child)?,
        Kind::Slice => slice(g, c, loc, call, &source, value.ty)?,
        Kind::Join => join(g, c, loc, call, &source, child)?,
        Kind::Concat => concat(g, c, call, &source, value.ty, child)?,
    };
    let value = Value {
        ty,
        code: format!("{{\n{setup}\n{code}\n}}"),
        ..Value::default()
    };
    if kind == Kind::Reduce && call.args.len() == 1 {
        return super::exceptions::checked_call(g, c, value, loc);
    }
    Ok(value)
}
