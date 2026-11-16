//! Places: the targets of in-place writes (`o.f = v`, `xs[i] = v`,
//! `xs.push(v)`), rooted at a local binding.
//!
//! Zant keeps TypeScript objects and arrays as values (copy-on-write, the
//! "mutable value semantics" of Swift structs and Hylo) rather than shared
//! references. The two models agree whenever a value is changed only through
//! the one variable that holds it, so a write is accepted only when its root
//! local is
//!
//! - **fresh**: its initializer built a new value (a literal, a spread copy,
//!   `map` / `filter` / `slice` / … or a plain function's result), not a
//!   parameter, a loop variable, or another variable's value; and
//! - **unseen**: it has not been used as a whole value since (passed, stored,
//!   copied or returned), which would not see a later change.
//!
//! Anything else is a diagnostic naming the copy to make instead, so a
//! program never silently behaves differently from TypeScript.

use crate::ast;

use super::expr;
use super::model;
use super::rust_text;
use super::scope;
use super::types::{self, Type};
use super::{Generator, Result, fail};

/// A Rust place expression (assignable, `&mut`-borrowable) and its type.
#[derive(Clone, Debug)]
pub struct Place {
    pub code: String,
    pub ty: Type,
}

/// Array / object methods whose result is a new value, never the receiver.
const FRESH_METHODS: &[&[u8]] = &[
    b"map",
    b"filter",
    b"slice",
    b"concat",
    b"flat",
    b"flatMap",
    b"split",
    b"toSorted",
    b"toReversed",
    b"toSpliced",
    b"with",
    b"keys",
    b"values",
    b"entries",
    b"from",
    b"fromEntries",
];

/// Whether evaluating `e` produces a value no other variable holds.
pub fn fresh(e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::Array(_) | ast::ExprData::Object(_) => true,
        ast::ExprData::Call(call) => match &call.callee.data {
            ast::ExprData::Member(member) => FRESH_METHODS.contains(&member.name.name.as_slice()),
            // a plain function's result: a value it built or a copy
            ast::ExprData::Ident(_) => true,
            _ => false,
        },
        ast::ExprData::Cond(cond) => fresh(&cond.then) && fresh(&cond.else_),
        ast::ExprData::Binary(binary) if matches!(binary.op.as_slice(), b"??" | b"||") => {
            fresh(&binary.lhs) && fresh(&binary.rhs)
        }
        ast::ExprData::As(cast) => fresh(&cast.expr),
        ast::ExprData::NonNull(inner) => fresh(inner),
        _ => false,
    }
}

/// Marks the next read of `object` (when it is a plain name) as a projection
/// (`xs.length`, `xs[0]`, `xs.map(…)`): the value is looked at, not copied
/// away, so a later write stays visible to that read.
pub fn project(c: &mut scope::Context, object: &ast::Expr) {
    c.projection = matches!(object.data, ast::ExprData::Ident(_));
}

/// The place `target` names, for writing.
pub fn resolve(g: &mut Generator<'_>, c: &mut scope::Context, target: &ast::Expr) -> Result<Place> {
    match &target.data {
        ast::ExprData::Ident(name) => root(g, c, name, target.loc),
        ast::ExprData::Member(member) => {
            if member.optional {
                return fail(
                    g,
                    target.loc,
                    "`?.` cannot be assigned through; check the value first",
                );
            }
            let object = resolve(g, c, &member.object)?;
            if model::optional_child(g, object.ty).is_some() {
                return fail(
                    g,
                    member.object.loc,
                    "this value is optional; check it (`if (x)`) or assign the whole value",
                );
            }
            if model::element(g, object.ty).is_some() {
                return fail(
                    g,
                    member.name.loc,
                    "array properties cannot be assigned; use push / splice (or `length = 0` to empty it)",
                );
            }
            let ty = model::field_type(g, object.ty, &member.name.name, member.name.loc)?;
            let field = match model::field_rust(g, object.ty, &member.name.name) {
                Some(field) => field,
                None => rust_text::id_at(g, &member.name.name, member.name.loc)?,
            };
            Ok(Place {
                code: format!("{}.{field}", object.code),
                ty,
            })
        }
        ast::ExprData::Index(index) => {
            if index.optional {
                return fail(
                    g,
                    target.loc,
                    "`?.[]` cannot be assigned through; check the array first",
                );
            }
            let object = resolve(g, c, &index.object)?;
            let Some(child) = model::element(g, object.ty) else {
                return fail(
                    g,
                    index.object.loc,
                    "only arrays can be indexed for writing",
                );
            };
            let at = position(g, c, &index.index)?;
            Ok(Place {
                code: format!("{}[{at}]", object.code),
                ty: child,
            })
        }
        _ => fail(
            g,
            target.loc,
            "only variables, their fields and array items can be changed",
        ),
    }
}

/// `tsx::index(i)` for an array index expression.
pub fn position(g: &mut Generator<'_>, c: &mut scope::Context, e: &ast::Expr) -> Result<String> {
    let value = expr::emit(g, c, e, None, false)?;
    if model::optional_child(g, value.ty).is_some() {
        return fail(g, e.loc, "optional array index needs ?? or !");
    }
    if !value.ty.numeric() {
        return fail(g, e.loc, "array index must be numeric");
    }
    Ok(format!("tsx::index({})", types::concrete(g, value)?.code))
}

/// The local binding a place starts from.
fn root(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    name: &[u8],
    loc: ast::Loc,
) -> Result<Place> {
    let text = String::from_utf8_lossy(name).into_owned();
    let Some(index) = scope::local_index(c, name) else {
        if let Some(state) = scope::state_index(g, name) {
            let setter = g.states[state].setter.clone();
            let example = if model::element(g, g.states[state].ty).is_some() {
                format!("{setter}([...{text}, item])")
            } else {
                format!("{setter}({{ ...{text}, field: value }})")
            };
            return fail(
                g,
                loc,
                format!(
                    "`{text}` is state, which is read-only: call `{setter}(…)` with a new value (`{example}`)"
                ),
            );
        }
        return fail(
            g,
            loc,
            format!(
                "`{text}` cannot be changed here: only local variables change in place (props, state, stores and values a handler captured are read-only); build a new value instead"
            ),
        );
    };
    let binding = &c.bindings[index];
    let (fresh, escaped, ty) = (binding.fresh, binding.escaped, binding.ty);
    let copy = if model::element(g, ty).is_some() {
        format!("[...{text}]")
    } else {
        format!("{{ ...{text} }}")
    };
    if !fresh {
        return fail(
            g,
            loc,
            format!(
                "`{text}` may hold the same value as another variable (it is a parameter, a loop variable, or was set from another variable or item). Arrays and objects are values in Zant, so changing `{text}` would not change that one: change a copy (`const copy = {copy}`) and use the copy, or change the original where it lives (`items[i].done = true`)"
            ),
        );
    }
    if escaped {
        return fail(
            g,
            loc,
            format!(
                "`{text}` was already used as a whole value above (passed, stored, copied or returned); arrays and objects are values in Zant, so that use would not see this change. Finish changing `{text}` before using it, or build a new value instead"
            ),
        );
    }
    let rust = c.bindings[index].rust.clone();
    c.bindings[index].assigned = true;
    Ok(Place { code: rust, ty })
}

/// `o.f = v`, `xs[i] = v`, `o.n += 1`, `xs[i]++`, `xs.length = 0`: an
/// assignment statement whose target is a field or an array item.
pub fn assign(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    target: &ast::Expr,
) -> Result<String> {
    // `xs.length = 0` empties the array (other lengths would truncate or
    // leave holes)
    if let ast::ExprData::Member(member) = &target.data
        && member.name.name == b"length"
        && let ast::ExprData::Assign(assignment) = &e.data
    {
        let array = resolve(g, c, &member.object)?;
        if model::element(g, array.ty).is_some() {
            let zero =
                matches!(&assignment.value.data, ast::ExprData::Number(n) if n.as_slice() == b"0");
            if assignment.op != b"=" || !zero {
                return fail(
                    g,
                    e.loc,
                    "only `length = 0` is supported (it empties the array); use splice to remove items",
                );
            }
            return Ok(format!("{}.clear();\n", array.code));
        }
    }
    if let ast::ExprData::Index(index) = &target.data
        && let ast::ExprData::Assign(assignment) = &e.data
        && assignment.op == b"="
    {
        // `xs[i] = v` replaces an item, or appends at `length`
        let array = resolve(g, c, &index.object)?;
        let Some(child) = model::element(g, array.ty) else {
            return fail(
                g,
                index.object.loc,
                "only arrays can be indexed for writing",
            );
        };
        let at = position(g, c, &index.index)?;
        let value = expr::emit(g, c, &assignment.value, Some(child), false)?;
        let value = types::coerce(g, value, child, assignment.value.loc)?;
        return Ok(format!(
            "{{\nlet _at: usize = {at};\nlet _value: {} = {};\n{}.assign(_at, _value);\n}}\n",
            child.rust(g),
            value.code,
            array.code
        ));
    }
    let place = resolve(g, c, target)?;
    let ty = place.ty;
    let value = match &e.data {
        ast::ExprData::Update(update) => {
            if !ty.numeric() {
                return fail(g, e.loc, "++ and -- require a number");
            }
            let one = if ty.float() { "1.0" } else { "1" };
            let op = if update.op == b"++" { "+=" } else { "-=" };
            return Ok(format!("{} {op} {one};\n", place.code));
        }
        ast::ExprData::Assign(assignment) if assignment.op == b"=" => {
            expr::emit(g, c, &assignment.value, Some(ty), false)?
        }
        ast::ExprData::Assign(assignment) => {
            // `o.n += 1`: the arithmetic checker on the current value
            let binary = ast::Binary {
                op: assignment.op[..assignment.op.len() - 1].to_vec(),
                lhs: Box::new(target.clone()),
                rhs: assignment.value.clone(),
            };
            expr::binary(g, c, e.loc, &binary, Some(ty), false)?
        }
        _ => unreachable!("assignment"),
    };
    let value = types::coerce(g, value, ty, e.loc)?;
    Ok(format!(
        "{{\nlet _value: {} = {};\n{} = _value;\n}}\n",
        ty.rust(g),
        value.code,
        place.code
    ))
}
