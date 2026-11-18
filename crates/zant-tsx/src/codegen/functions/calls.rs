//! Calling a file function: lookup, argument checks and the generated call.

use super::*;

pub fn find_index(g: &Generator<'_>, name: &[u8]) -> Option<usize> {
    g.functions
        .iter()
        .position(|function| function.name.as_bytes() == name)
}

/// `bytes(n)` where `bytes` is a function of this file or an imported one.
pub fn try_call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<Value>> {
    let ast::ExprData::Ident(name) = &call.callee.data else {
        return Ok(None);
    };
    if scope::local(c, name).is_some() {
        return Ok(None);
    }
    if let Some(template) = crate::codegen::generic_functions::find(g, name) {
        return crate::codegen::generic_functions::call(g, c, loc, call, template).map(Some);
    }
    let Some(index) = find_index(g, name) else {
        return Ok(None);
    };
    let function = g.functions[index].clone();
    let name = String::from_utf8_lossy(name).into_owned();
    if !call.type_args.is_empty() {
        return fail(g, loc, format!("`{name}` takes no type arguments"));
    }
    call_with(g, c, loc, call, &function, Vec::new()).map(Some)
}

/// A call of `function` (its arguments checked against its parameters).
/// `emitted` holds argument values already generated (a generic call infers
/// its type arguments from them); the others are generated here.
pub fn call_with(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    function: &Function,
    mut emitted: Vec<Option<Value>>,
) -> Result<Value> {
    emitted.resize(call.args.len(), None);
    let name = function.func.name.clone();
    let name = String::from_utf8_lossy(&name).into_owned();
    let required = required(&function.func.params);
    let rest = function.func.params.last().is_some_and(|param| param.rest);
    let fixed = function.params.len() - usize::from(rest);
    // a spread argument fills the rest parameter (`sum(...values)`)
    if let Some(spread) = call.args[..call.args.len().min(fixed)]
        .iter()
        .find(|arg| matches!(arg.data, ast::ExprData::Spread(_)))
    {
        return fail(
            g,
            spread.loc,
            "a spread argument can only fill a rest parameter (`...items: T[]`)",
        );
    }
    if call.args.len() < required || (!rest && call.args.len() > function.params.len()) {
        let count = if rest {
            format!("at least {required}")
        } else {
            function.params.len().to_string()
        };
        return fail(g, loc, format!("`{name}` takes {count} arguments"));
    }
    let mut arguments: Vec<String> = Vec::new();
    for (index, ty) in function.params.iter().enumerate() {
        if rest && index == fixed {
            // the remaining arguments, spreads included, become one array
            // (`[a, ...b]` semantics)
            let items = call.args.get(fixed..).unwrap_or_default().to_vec();
            if let [only] = items.as_slice()
                && let ast::ExprData::Spread(inner) = &only.data
            {
                // `f(...items)` passes the list itself (lists are immutable)
                let value = expr::emit(g, c, inner, Some(*ty), false)?;
                arguments.push(types::coerce(g, value, *ty, inner.loc)?.code);
                continue;
            }
            let array = ast::Expr {
                loc: items.first().map_or(loc, |item| item.loc),
                data: ast::ExprData::Array(items),
            };
            let value = expr::emit(g, c, &array, Some(*ty), false)?;
            arguments.push(types::coerce(g, value, *ty, array.loc)?.code);
            continue;
        }
        if index >= call.args.len() {
            arguments.push(missing(g, &function.func.params[index], *ty)?);
            continue;
        }
        let value = match emitted[index].take() {
            Some(value) => value,
            None => expr::emit(g, c, &call.args[index], Some(*ty), false)?,
        };
        arguments.push(types::coerce(g, value, *ty, call.args[index].loc)?.code);
    }
    let value = Value {
        ty: function.ret,
        code: format!("{}({})", function.rust, arguments.join(", ")),
        ..Value::default()
    };
    if function.throws {
        return crate::codegen::exceptions::checked_call(g, c, value, loc);
    }
    Ok(value)
}
