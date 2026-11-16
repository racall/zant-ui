//! In-place array methods (`push`, `pop`, `shift`, `unshift`, `splice`,
//! `reverse`, `fill`, `sort`): the receiver is a [`place`] (a local array, or
//! an array field / item of one), changed through one `&mut` borrow after the
//! arguments are evaluated. Results follow `Array.prototype`: `push` and
//! `unshift` return the new length, `pop` / `shift` the removed item or
//! `undefined`, `splice` the removed items, and `reverse` / `fill` / `sort` the
//! array.

use crate::ast;

use super::callbacks;
use super::expr;
use super::model;
use super::place;
use super::rust_text::Format;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

/// Whether `name` is an in-place array method.
pub fn is_mutating(name: &[u8]) -> bool {
    matches!(
        name,
        b"push" | b"pop" | b"shift" | b"unshift" | b"splice" | b"reverse" | b"fill" | b"sort"
    )
}

pub fn emit(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    member: &ast::Member,
) -> Result<Value> {
    let name = String::from_utf8_lossy(&member.name.name).into_owned();
    if member.optional {
        return fail(
            g,
            loc,
            format!("`?.{name}()` changes the array in place; check the array first"),
        );
    }
    if c.constant {
        return fail(g, loc, "array methods cannot run in a top-level constant");
    }
    let target = place::resolve(g, c, &member.object)?;
    let Some(child) = model::element(g, target.ty) else {
        return fail(g, member.object.loc, format!("`{name}` needs an array"));
    };
    let array = target.ty;
    let mut setup = String::new();
    let (ty, body) = match name.as_str() {
        "push" | "unshift" => {
            let items = items(g, c, &call.args, child, &mut setup)?;
            let change = if name == "push" {
                format!("_target.extend_from({items}.as_slice());")
            } else {
                format!("_target.splice(0.0, 0, {items});")
            };
            (Type::Usize, format!("{change}\n_target.len()"))
        }
        "pop" | "shift" => {
            arity(g, loc, call, 0, 0)?;
            (model::optional(g, child)?, format!("_target.{name}()"))
        }
        "splice" => {
            if call.args.is_empty() {
                return fail(g, loc, "splice needs a start index");
            }
            let start = number(g, c, &call.args[0])?;
            setup.push_str(&format!("let _start: f64 = {start};\n"));
            let count = match call.args.get(1) {
                Some(count) => format!("Some({})", number(g, c, count)?),
                None => "None".to_owned(),
            };
            setup.push_str(&format!("let _count: Option<f64> = {count};\n"));
            let rest = call.args.get(2..).unwrap_or_default();
            let items = items(g, c, rest, child, &mut setup)?;
            (array, format!("_target.splice_js(_start, _count, {items})"))
        }
        "reverse" => {
            arity(g, loc, call, 0, 0)?;
            (array, "_target.reverse();\n_target.clone()".to_owned())
        }
        "fill" => {
            arity(g, loc, call, 1, 3)?;
            let value = expr::emit(g, c, &call.args[0], Some(child), false)?;
            let value = types::coerce(g, value, child, call.args[0].loc)?;
            setup.push_str(&format!(
                "let _value: {} = {};\n",
                child.rust(g),
                value.code
            ));
            let mut bounds = Vec::new();
            for bound in call.args.iter().skip(1) {
                bounds.push(format!("Some({})", number(g, c, bound)?));
            }
            bounds.resize(2, "None".to_owned());
            (
                array,
                format!(
                    "_target.fill(_value, {}, {});\n_target.clone()",
                    bounds[0], bounds[1]
                ),
            )
        }
        "sort" => {
            arity(g, loc, call, 0, 1)?;
            let sort = match call.args.first() {
                Some(compare) => comparator(g, c, compare, child)?,
                None => default_order(g, child, loc)?,
            };
            (array, format!("{sort};\n_target.clone()"))
        }
        _ => unreachable!("mutating array method"),
    };
    Ok(Value {
        ty,
        code: format!("{{\n{setup}let _target = &mut {};\n{body}\n}}", target.code),
        ..Value::default()
    })
}

fn arity(
    g: &mut Generator<'_>,
    loc: ast::Loc,
    call: &ast::Call,
    min: usize,
    max: usize,
) -> Result<()> {
    if call.args.len() < min || call.args.len() > max {
        let expected = if min == max {
            min.to_string()
        } else {
            format!("{min} to {max}")
        };
        return fail(
            g,
            loc,
            format!("this array method takes {expected} arguments"),
        );
    }
    Ok(())
}

/// A numeric argument as `f64` (JS index arguments go through ToNumber).
fn number(g: &mut Generator<'_>, c: &mut scope::Context, e: &ast::Expr) -> Result<String> {
    let value = expr::emit(g, c, e, Some(Type::F64), false)?;
    if !value.ty.numeric() {
        return fail(g, e.loc, "expected a number");
    }
    Ok(types::coerce(g, value, Type::F64, e.loc)?.code)
}

/// The items `push(…)` / `unshift(…)` / `splice(_, _, …)` add, spreads
/// included, as a `Vec` built in source order before the array is borrowed.
fn items(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    args: &[Box<ast::Expr>],
    child: Type,
    setup: &mut String,
) -> Result<String> {
    let item = child.rust(g).to_owned();
    setup.push_str(&format!("let mut _items: Vec<{item}> = Vec::new();\n"));
    for arg in args {
        if let ast::ExprData::Spread(inner) = &arg.data {
            let array = model::array(g, child)?;
            let value = expr::emit(g, c, inner, Some(array), false)?;
            let value = types::coerce(g, value, array, inner.loc)?;
            setup.push_str(&format!("_items.extend({}.iter().cloned());\n", value.code));
            continue;
        }
        let value = expr::emit(g, c, arg, Some(child), false)?;
        let value = types::coerce(g, value, child, arg.loc)?;
        setup.push_str(&format!("_items.push({});\n", value.code));
    }
    Ok("_items".to_owned())
}

/// `sort((a, b) => …)`: the arrow as a comparator closure returning a number.
fn comparator(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    compare: &ast::Expr,
    child: Type,
) -> Result<String> {
    let ast::ExprData::Arrow(function) = &compare.data else {
        return fail(
            g,
            compare.loc,
            "sort takes an inline comparator: `(a, b) => a - b`",
        );
    };
    let name = |index: usize| match function.params.get(index) {
        Some(param) => String::from_utf8_lossy(&param.name.name).into_owned(),
        None => format!("_arg{index}"),
    };
    let item = child.rust(g).to_owned();
    let pattern = format!("{}: {item}, {}: {item}", name(0), name(1));
    let (closure, ty) =
        callbacks::closure(g, c, compare, &[child, child], &pattern, Some(Type::F64))?;
    if !ty.numeric() {
        return fail(g, compare.loc, "a sort comparator returns a number");
    }
    Ok(format!("_target.sort_js({closure})"))
}

/// `sort()`: by the items' `String(…)` spellings, as `Array.prototype.sort`
/// does without a comparator; absent items sort last.
fn default_order(g: &mut Generator<'_>, child: Type, loc: ast::Loc) -> Result<String> {
    let (inner, optional) = match model::optional_child(g, child) {
        Some(inner) => (inner, true),
        None => (child, false),
    };
    if !(inner.numeric()
        || inner == Type::String
        || inner == Type::Boolean
        || model::enum_type(g, inner))
    {
        return fail(
            g,
            loc,
            "sort() without a comparator orders numbers, strings or booleans by their text; pass a comparator: `sort((a, b) => …)`",
        );
    }
    let mut format = Format::default();
    let read = if inner.copy(g) {
        "*_item".to_owned()
    } else {
        "_item.clone()".to_owned()
    };
    format.value(g, Value::new(inner, read), loc)?;
    let text = format.finish(g, false)?;
    Ok(if optional {
        format!(
            "_target.sort_default(|_item| _item.as_ref().map(|_item| {text}.as_str().to_owned()))"
        )
    } else {
        format!("_target.sort_default(|_item| Some({text}.as_str().to_owned()))")
    })
}
