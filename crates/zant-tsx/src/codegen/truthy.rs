//! JavaScript truthiness: conditions (`if (name)`, `!count`,
//! `disabled={!title}`), and `||` / `&&` that return values
//! (`name || "Untitled"`, `user && user.name`).
//!
//! Falsy: `false`, `""`, `0`, `NaN`, `null` / `undefined`. Everything else is
//! truthy, objects and arrays included (an empty array too).

use crate::ast;

use super::expr;
use super::model::{self, InfoKind};
use super::narrowing;
use super::rust_text::{self, Precedence};
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

/// `value` as a boolean condition.
pub fn test(g: &mut Generator<'_>, value: Value, loc: ast::Loc) -> Result<Value> {
    let ty = value.ty;
    if ty == Type::Boolean {
        return Ok(value);
    }
    if model::optional_child(g, ty).is_some() {
        return narrowing::truthy(g, value);
    }
    let operand = rust_text::operand(&value, Precedence::Suffix, false);
    let code = if ty == Type::String {
        format!("!{operand}.is_empty()")
    } else if ty.float() || ty == Type::FloatLiteral {
        format!(
            "{{ let _number: f64 = ({}) as f64; _number != 0.0 && !_number.is_nan() }}",
            value.code
        )
    } else if ty.numeric() {
        format!("({}) != 0", value.code)
    } else if ty == Type::Null {
        "false".to_owned()
    } else if model::enum_type(g, ty) {
        format!("!{operand}.as_str().is_empty()")
    } else if matches!(ty, Type::Void | Type::Node | Type::OptionalNode)
        || model::info(g, ty).is_some_and(|info| info.kind == InfoKind::Union)
        || super::exceptions::is_thrown(g, ty)
    {
        return fail(
            g,
            loc,
            "compare this value explicitly in a condition (for example `x !== null` or `typeof x === \"string\"`)",
        );
    } else {
        // objects, arrays, records, functions: always truthy (still evaluated)
        format!("{{ let _ = &{operand}; true }}")
    };
    Ok(Value {
        ty: Type::Boolean,
        code,
        precedence: Precedence::Suffix,
        ..Value::default()
    })
}

/// `a || b` where `a` is not a boolean (`name || "Untitled"`): the left
/// value when it is truthy, else the right one. `lhs` is `a`, generated once.
pub fn or_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    b: &ast::Binary,
    lhs: Value,
    expected: Option<Type>,
) -> Result<Value> {
    let lhs = types::concrete(g, lhs)?;
    let child = model::optional_child(g, lhs.ty);
    // a literal takes the left side's type (`n || 42`); anything else keeps
    // its own (`nick || (user && user.name)` stays optional)
    let target = expected.or(child).unwrap_or(lhs.ty).concrete();
    let context = types::numeric_literals(&b.rhs).then_some(target);
    let rhs = expr::emit(g, c, &b.rhs, context, false)?;
    let mut ty = types::unify(g, child.unwrap_or(lhs.ty), rhs.ty, b.rhs.loc)?.concrete();
    // `a || maybe`: still optional
    if model::optional_child(g, rhs.ty).is_some() && model::optional_child(g, ty).is_none() {
        ty = model::optional(g, ty)?;
    }
    // tested on a copy: the value itself is kept (`clone` of a string /
    // list is a reference count)
    let condition = test(g, Value::new(lhs.ty, "_or.clone()"), b.lhs.loc)?;
    // a truthy optional is present
    let kept = match child {
        Some(inner) if model::optional_child(g, ty).is_none() => Value::new(inner, "_or.unwrap()"),
        _ => Value::new(lhs.ty, "_or"),
    };
    let kept = types::coerce(g, kept, ty, b.lhs.loc)?;
    let rhs = types::coerce(g, rhs, ty, b.rhs.loc)?;
    Ok(Value {
        ty,
        code: format!(
            "{{ let _or = {}; if {} {{ {} }} else {{ {} }} }}",
            lhs.code, condition.code, kept.code, rhs.code
        ),
        precedence: Precedence::Primary,
        ..Value::default()
    })
}

/// `user && user.name` (not in JSX): the right value when the optional left
/// side is truthy, else absent. `None` when the left side is not optional
/// (JavaScript's result would mix two types).
pub fn and_value(
    g: &mut Generator<'_>,
    lhs: &Value,
    condition: &Value,
    rhs: Value,
) -> Result<Option<Value>> {
    if model::optional_child(g, lhs.ty).is_none() {
        // `p && p.name` where `p` is an object, array, record or function:
        // always truthy, so the value is the right side
        if always_truthy(g, lhs.ty) {
            return Ok(Some(Value {
                ty: rhs.ty,
                code: format!("{{ let _ = &{}; {} }}", lhs.code, rhs.code),
                precedence: Precedence::Primary,
                ..Value::default()
            }));
        }
        return Ok(None);
    }
    let (ty, some) = match model::optional_child(g, rhs.ty) {
        Some(_) => (rhs.ty, rhs.code),
        None => (
            model::optional(g, rhs.ty.concrete())?,
            format!("Some({})", rhs.code),
        ),
    };
    Ok(Some(Value {
        ty,
        code: format!("if {} {{ {some} }} else {{ None }}", condition.code),
        precedence: Precedence::Conditional,
        ..Value::default()
    }))
}

/// A value that is never falsy: objects, arrays, records, maps, sets,
/// tuples and functions (not optional).
fn always_truthy(g: &Generator<'_>, ty: Type) -> bool {
    model::info(g, ty).is_some_and(|info| {
        matches!(
            info.kind,
            InfoKind::Object
                | InfoKind::Array
                | InfoKind::Record
                | InfoKind::Map
                | InfoKind::Set
                | InfoKind::Tuple
                | InfoKind::Function
                | InfoKind::Callback
        )
    }) && !super::exceptions::is_thrown(g, ty)
}
