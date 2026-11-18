//! Array methods with a callback (`map`, `filter`, `forEach`, `some`, `reduce`, …) as loops.

use super::*;

pub(super) fn arity(
    g: &mut Generator<'_>,
    loc: ast::Loc,
    count: usize,
    min: usize,
    max: usize,
) -> Result<()> {
    if count < min || count > max {
        return fail(
            g,
            loc,
            format!("array method expects {min}..{max} arguments"),
        );
    }
    Ok(())
}

/// The arrow of an array callback (`None` for a non-arrow, reported later).
pub(super) fn arrow(call: &ast::Call) -> Option<&ast::Function> {
    match &call.args[0].data {
        ast::ExprData::Arrow(function) => Some(function),
        _ => None,
    }
}

/// The name a callback parameter is bound to (`_item` when the arrow leaves it
/// out), so the emitted closure pattern and the body agree.
pub(super) fn parameter_name(function: Option<&ast::Function>, index: usize) -> String {
    match function.and_then(|f| f.params.get(index)) {
        Some(param) => String::from_utf8_lossy(&param.name.name).into_owned(),
        None => format!("_arg{index}"),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn loop_method(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    kind: Kind,
    call: &ast::Call,
    source: &str,
    child: Type,
    expected: Option<Type>,
) -> Result<(Type, String)> {
    let reduce = kind == Kind::Reduce;
    arity(g, loc, call.args.len(), 1, if reduce { 2 } else { 1 })?;
    // `xs.reduce((a, b) => a + b)`: the first item starts the fold (a
    // TypeError for an empty array, as in JavaScript)
    let from_first = reduce && call.args.len() == 1;
    let mut initial: Option<Value> = None;
    if from_first {
        initial = Some(Value::new(child, "_first"));
    } else if reduce {
        let value = expr::emit(g, c, &call.args[1], None, false)?;
        initial = Some(types::coerce(
            g,
            value.clone(),
            value.ty.concrete(),
            call.args[1].loc,
        )?);
    }
    let target = match kind {
        Kind::Map => match expected {
            Some(ty) => model::element(g, ty),
            None => None,
        },
        Kind::Reduce => initial.as_ref().map(|value| value.ty),
        _ => Some(Type::Boolean),
    };
    let function = arrow(call);
    let params = function.map(|f| f.params.len()).unwrap_or(0);
    let needs_index = if reduce {
        params > 2
    } else {
        kind == Kind::FindIndex || params > 1
    };
    let item = parameter_name(function, if reduce { 1 } else { 0 });
    let index = parameter_name(function, if reduce { 2 } else { 1 });
    let result = parameter_name(function, 0);
    let initial_ty = initial.as_ref().map(|value| value.ty);
    // The arrow's parameter order: `(item[, index])` or, for `reduce`,
    // `(accumulator, item[, index])`.
    let typed = |index: Option<Type>| -> Vec<Type> {
        let mut arguments = Vec::new();
        if reduce {
            arguments.push(initial_ty.expect("reduce initial"));
        }
        arguments.push(child);
        if let Some(ty) = index {
            arguments.push(ty);
        }
        arguments
    };
    let callback_types = typed(if needs_index { Some(Type::Usize) } else { None });
    // The iterator adapter's tuple order is `(index, item)`, the TSX arrow's is
    // `(item, index)`.
    // Closure parameters need explicit types: the call's argument types are
    // inferred from the wrapper call, not from a closure used immediately.
    let item_ty = child.rust(g).to_owned();
    let reduce_initial_ty = initial_ty.map(|ty| ty.rust(g).to_owned());
    let pattern = if reduce {
        match &reduce_initial_ty {
            Some(initial) if needs_index => {
                format!("{result}: {initial}, ({index}, {item}): (usize, {item_ty})")
            }
            Some(initial) => format!("{result}: {initial}, {item}: {item_ty}"),
            None => format!("{result}, {item}"),
        }
    } else if needs_index {
        format!("({index}, {item}): (usize, {item_ty})")
    } else {
        format!("{item}: {item_ty}")
    };
    let _ = &pattern;
    let skip = if from_first { ".skip(1)" } else { "" };
    let adapted = if needs_index {
        format!("{source}.as_slice().iter().cloned().enumerate(){skip}")
    } else {
        format!("{source}.as_slice().iter().cloned(){skip}")
    };
    Ok(match kind {
        Kind::Map => {
            let (closure, ty) =
                callbacks::closure(g, c, &call.args[0], &callback_types, &pattern, target)?;
            (
                model::array(g, ty)?,
                format!("tsx::List::from_vec({adapted}.map({closure}).collect::<Vec<_>>())"),
            )
        }
        Kind::Filter | Kind::Find | Kind::FindIndex => {
            // the predicate closure consumes its arguments: clone the item we
            // keep and bind the predicate's parameters by value
            let (closure, _) = callbacks::closure(
                g,
                c,
                &call.args[0],
                &callback_types,
                &pattern,
                Some(Type::Boolean),
            )?;
            let argument = if needs_index {
                format!("({index}, {item}.clone())")
            } else {
                format!("{item}.clone()")
            };
            let wrapper = if needs_index {
                format!("({index}, {item})")
            } else {
                item.clone()
            };
            match kind {
                Kind::Filter => {
                    let ty = model::array(g, child)?;
                    (
                        ty,
                        format!(
                            "tsx::List::from_vec({adapted}.filter_map(|{wrapper}| {{\n\
                             let _keep: bool = ({closure})({argument});\n\
                             if _keep {{ Some({item}) }} else {{ None }}\n\
                             }}).collect::<Vec<_>>())"
                        ),
                    )
                }
                Kind::Find => {
                    let ty = model::optional(g, child)?;
                    (
                        ty,
                        format!(
                            "{adapted}.find_map(|{wrapper}| {{\n\
                             let _hit: bool = ({closure})({argument});\n\
                             if _hit {{ Some({item}) }} else {{ None }}\n\
                             }})"
                        ),
                    )
                }
                _ => (
                    Type::Isize,
                    format!(
                        "{adapted}.find_map(|{wrapper}| {{\n\
                         let _hit: bool = ({closure})({argument});\n\
                         if _hit {{ Some({index}) }} else {{ None }}\n\
                         }}).map_or(-1, |index| index as isize)"
                    ),
                ),
            }
        }
        Kind::Some | Kind::Every => {
            let (closure, _) = callbacks::closure(
                g,
                c,
                &call.args[0],
                &callback_types,
                &pattern,
                Some(Type::Boolean),
            )?;
            let adapter = if kind == Kind::Some { "any" } else { "all" };
            (Type::Boolean, format!("{adapted}.{adapter}({closure})"))
        }
        Kind::Reduce => {
            let initial = initial.expect("reduce initial");
            let ty = initial.ty;
            let (closure, _) =
                callbacks::closure(g, c, &call.args[0], &callback_types, &pattern, Some(ty))?;
            let fold = format!("{adapted}.fold({}, {closure})", initial.code);
            if from_first {
                // a `Result`: `emit` passes it through the exception path
                (
                    ty,
                    format!(
                        "match {source}.as_slice().first().cloned() {{\n\
                         Some(_first) => Ok({fold}),\n\
                         None => Err(zant::tsx::Thrown::error(zant::tsx::Error::new(\"TypeError\", \
                         zant::Str::Static(\"Reduce of empty array with no initial value\")))),\n}}"
                    ),
                )
            } else {
                (ty, fold)
            }
        }
        _ => unreachable!("non-loop array method"),
    })
}
