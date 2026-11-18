//! `Math`, `Number` predicates and global numeric functions.

use super::*;

pub(super) fn number_predicate(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    name: &ast::Name,
) -> Result<Option<Value>> {
    let kind = match name.name.as_slice() {
        b"isNaN" => "IsNaN",
        b"isFinite" => "IsFinite",
        b"isInteger" => "IsInteger",
        _ => return Ok(None),
    };
    arity(g, loc, call, 1, 1)?;
    let mut builder = Call::default();
    let value = expr::emit(g, c, &call.args[0], None, false)?;
    let value_ty = value.ty;
    builder.add(g, c, value, value_ty.concrete(), call.args[0].loc)?;
    Ok(Some(builder.finish(
        Type::Boolean,
        format!(
            "tsx::builtins::predicate(tsx::builtins::Predicate::{kind}, {})",
            builder.args[0]
        ),
    )))
}

pub(super) fn global(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    name: &str,
) -> Result<Value> {
    arity(g, loc, call, 1, if name == "parseInt" { 2 } else { 1 })?;
    let mut builder = Call::default();
    let mut value = expr::emit(g, c, &call.args[0], None, false)?;
    if name == "String"
        && let Some(text) = crate::codegen::unions::text(g, &value, call.args[0].loc)?
    {
        value = text;
    }
    let conversion = name == "Number" || name == "String";
    if conversion && !value.ty.numeric() && value.ty != Type::String && value.ty != Type::Boolean {
        let message = if crate::codegen::model::optional_child(g, value.ty).is_some() {
            format!(
                "this value may be missing (null / undefined); give it a fallback: `{name}(x ?? 0)`"
            )
        } else {
            "conversion requires a number, string, or boolean".to_owned()
        };
        return fail(g, call.args[0].loc, message);
    }
    if name == "String" && c.constant {
        return fail(g, loc, "String cannot run in a top-level constant");
    }
    let target = if conversion {
        value.ty.concrete()
    } else {
        Type::String
    };
    builder.add(g, c, value, target, call.args[0].loc)?;
    if let Some(arg) = call.args.get(1) {
        let digits = expr::emit(g, c, arg, Some(Type::F64), false)?;
        builder.add(g, c, digits, Type::F64, arg.loc)?;
    } else if name == "parseInt" {
        builder.args.push("0.0".to_owned());
    }
    // `parse_int` / `parse_float` take `&str`
    if (name == "parseInt" || name == "parseFloat")
        && let Some(first) = builder.args.first_mut()
    {
        *first = format!("({first}).as_str()");
    }
    let helper = match name {
        "Number" => "number",
        "String" => "string",
        "parseInt" => "parse_int",
        "parseFloat" => "parse_float",
        other => other,
    };
    let ty = if name == "String" {
        Type::String
    } else {
        Type::F64
    };
    Ok(builder.finish(ty, format!("tsx::builtins::{helper}({})", builder.joined())))
}

pub(super) fn math(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Value> {
    let ast::ExprData::Member(m) = &call.callee.data else {
        unreachable!("math member")
    };
    let name = String::from_utf8_lossy(&m.name.name).into_owned();
    let unary = UNARY_NAMES.contains(&name.as_str());
    let extrema = name == "min" || name == "max";
    let variadic = extrema || name == "hypot";
    let random = name == "random";
    let binary = name == "pow" || name == "atan2";
    if !unary && !variadic && !random && !binary {
        return unknown(g, "Math", &m.name);
    }
    let min = if unary {
        1
    } else if binary {
        2
    } else {
        0
    };
    arity(g, loc, call, min, if variadic { usize::MAX } else { min })?;
    if random {
        if c.constant {
            return fail(g, loc, "random cannot run in a top-level constant");
        }
        return Ok(Value::new(Type::F64, "tsx::builtins::random()"));
    }
    if variadic
        && call
            .args
            .iter()
            .any(|arg| matches!(arg.data, ast::ExprData::Spread(_)))
    {
        return spread_math(g, c, &name, call);
    }
    let mut values: Vec<Value> = Vec::new();
    let mut peer = Type::IntLiteral;
    for arg in &call.args {
        let value = expr::emit(g, c, arg, None, false)?;
        numeric(g, &value, arg.loc)?;
        peer = Type::numeric_peer(peer, value.ty).unwrap_or(Type::F64);
        values.push(value);
    }
    let preserve = extrema || matches!(name.as_str(), "floor" | "ceil" | "round" | "trunc" | "abs");
    let ty = if preserve && peer.integer() {
        peer
    } else {
        Type::F64
    };
    let mut builder = Call::default();
    for (value, arg) in values.into_iter().zip(call.args.iter()) {
        types::check_literal(g, arg, ty)?;
        builder.add(g, c, value, ty, arg.loc)?;
    }
    let args = builder.joined();
    let code = if unary {
        let variant = unary_variant(&name);
        format!("tsx::builtins::unary(tsx::builtins::Unary::{variant}, {args})")
    } else if extrema {
        // `extrema::<T>`: `Math.max()` has no value to infer `T` from
        format!(
            "tsx::builtins::extrema::<{}>({}, &[{args}])",
            ty.rust(g),
            name == "min"
        )
    } else if variadic {
        format!("tsx::builtins::hypot(&[{args}])")
    } else if name == "pow" {
        format!("tsx::builtins::pow({args})")
    } else {
        format!("tsx::builtins::atan2({args})")
    };
    Ok(builder.finish(ty, code))
}

/// `Math.max(...xs)` / `Math.min(0, ...xs)` / `Math.hypot(...xs)`: the
/// arguments collected into one `f64` list, so an empty array gives
/// `-Infinity` / `Infinity` / `0` as in JavaScript.
pub(super) fn spread_math(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    name: &str,
    call: &ast::Call,
) -> Result<Value> {
    let mut setup = String::from("let mut _items: Vec<f64> = Vec::new();\n");
    for arg in &call.args {
        let ast::ExprData::Spread(inner) = &arg.data else {
            let value = expr::emit(g, c, arg, None, false)?;
            numeric(g, &value, arg.loc)?;
            let value = types::coerce(g, value, Type::F64, arg.loc)?;
            setup.push_str(&format!("_items.push({});\n", value.code));
            continue;
        };
        let value = expr::emit(g, c, inner, None, false)?;
        let Some(item) = model::element(g, value.ty).filter(|item| item.numeric()) else {
            return fail(
                g,
                inner.loc,
                format!("Math.{name}(...) spreads an array of numbers"),
            );
        };
        let each = types::coerce(g, Value::new(item, "(*_x)"), Type::F64, inner.loc)?;
        setup.push_str(&format!(
            "_items.extend({}.iter().map(|_x| {}));\n",
            value.code, each.code
        ));
    }
    let code = if name == "hypot" {
        "tsx::builtins::hypot(&_items)".to_owned()
    } else {
        format!("tsx::builtins::extrema::<f64>({}, &_items)", name == "min")
    };
    Ok(Value::new(Type::F64, format!("{{\n{setup}{code}\n}}")))
}

pub(super) fn unary_variant(name: &str) -> &'static str {
    match name {
        "floor" => "Floor",
        "ceil" => "Ceil",
        "round" => "Round",
        "trunc" => "Trunc",
        "abs" => "Abs",
        "sign" => "Sign",
        "sqrt" => "Sqrt",
        "cbrt" => "Cbrt",
        "exp" => "Exp",
        "log" => "Log",
        "log2" => "Log2",
        "log10" => "Log10",
        "sin" => "Sin",
        "cos" => "Cos",
        "tan" => "Tan",
        "atan" => "Atan",
        _ => "Floor",
    }
}
