//! Callback prop values: absent callbacks, the choice between a handler and a forwarded prop, and the callback closure.

use super::*;

/// `undefined` / `null` where a callback goes.
pub(super) fn absent_callback(e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::NullLit => true,
        ast::ExprData::Ident(name) => name == b"undefined",
        _ => false,
    }
}

/// The value of a callback prop: like React, `cond ? a : b` picks its branch
/// at render time, and an optional prop takes `undefined` / `null` (absent)
/// or `Some(callback)`.
pub(super) fn callback_choice(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    entry: &model::Info,
    optional: bool,
    value: &ast::Expr,
) -> Result<String> {
    if absent_callback(value) {
        if optional {
            return Ok("None".to_owned());
        }
        return fail(
            g,
            value.loc,
            "this callback prop is required; make it optional (`onX?: …`) to pass undefined",
        );
    }
    if let ast::ExprData::Cond(cond) = &value.data {
        let test = expr::condition(g, c, &cond.cond)?.code;
        let then = callback_choice(g, c, entry, optional, &cond.then)?;
        let otherwise = callback_choice(g, c, entry, optional, &cond.else_)?;
        return Ok(format!("if {test} {{ {then} }} else {{ {otherwise} }}"));
    }
    // this component's own optional callback prop forwarded as is
    if optional
        && let ast::ExprData::Ident(name) = &value.data
        && let Some(prop) = read_prop(g, &String::from_utf8_lossy(name))?
        && model::optional_child(g, prop.ty).is_some()
    {
        let same = callback_info(g, prop.ty);
        if same.as_ref().is_none_or(|info| info.params != entry.params) {
            return fail(g, value.loc, "this callback has a different signature");
        }
        return Ok(prop.code);
    }
    let code = callback_value(g, c, entry, value)?;
    Ok(if optional {
        format!("Some({code})")
    } else {
        code
    })
}

/// A function, an inline arrow or a callback prop of this component, bound as
/// the callback type of the target prop.
pub(super) fn callback_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    entry: &model::Info,
    value: &ast::Expr,
) -> Result<String> {
    let method_index: usize;
    match &value.data {
        ast::ExprData::Arrow(_) => {
            let Some(index) = g
                .callback_events
                .iter()
                .find(|event| event.expr == *value)
                .map(|event| event.method)
            else {
                unreachable!("registered by `scan`")
            };
            method_index = index;
            captures::prepare(g, c, method_index)?;
        }
        ast::ExprData::Ident(name) => {
            if let Some(index) = g
                .callback_events
                .iter()
                .find(|event| event.expr == *value)
                .map(|event| event.method)
            {
                method_index = index;
                captures::prepare(g, c, method_index)?;
            } else {
                let prop = read_prop(g, &String::from_utf8_lossy(name))?;
                if let Some(prop) = prop {
                    // forwarding this component's own callback prop
                    let same = callback_info(g, prop.ty);
                    if same.as_ref().is_none_or(|info| info.params != entry.params) {
                        return fail(
                            g,
                            value.loc,
                            format!(
                                "`{}` has a different signature",
                                String::from_utf8_lossy(name)
                            ),
                        );
                    }
                    return Ok(prop.code);
                }
                let Some(index) = scope::source_method(g, name) else {
                    return fail(
                        g,
                        value.loc,
                        "callback props take a component function, an inline arrow or a callback prop",
                    );
                };
                method_index = index;
                let Some(function) = g.methods[index].func.clone() else {
                    return fail(
                        g,
                        value.loc,
                        format!(
                            "pass an arrow for setters: (v) => {}(v)",
                            String::from_utf8_lossy(name)
                        ),
                    );
                };
                if function.params.len() != entry.params.len() {
                    return fail(
                        g,
                        value.loc,
                        format!(
                            "`{}` must take {} arguments",
                            String::from_utf8_lossy(name),
                            entry.params.len()
                        ),
                    );
                }
            }
        }
        _ => {
            let v = expr::emit(g, c, value, None, false)?;
            let Some(same) = callback_info(g, v.ty) else {
                return fail(
                    g,
                    value.loc,
                    "callback props take a component function, an inline arrow or a callback prop",
                );
            };
            if same.params != entry.params {
                return fail(g, value.loc, "this callback has a different signature");
            }
            return Ok(v.code);
        }
    }
    match action_kind(&entry.params) {
        Some(kind) => crate::codegen::generate::action_value(g, c, method_index, Some(kind)),
        None => crate::codegen::generate::callback_value(g, c, method_index, &entry.params),
    }
}
