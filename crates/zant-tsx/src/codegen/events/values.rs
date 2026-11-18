//! Event values: the signature of a handler and the action value an event prop gets.

use super::*;

/// Validate the action signature of a registered method.
pub fn signature(
    g: &mut Generator<'_>,
    m: &Method,
    kind: Kind,
    loc: ast::Loc,
    inline_: bool,
) -> Result<()> {
    let Some(function) = m.func.as_ref() else {
        return fail(
            g,
            loc,
            format!("handler must have signature {}", action_signature(kind)),
        );
    };
    let count = if kind == Kind::Action { 0 } else { 1 };
    if m.ret != Type::Void
        || function.params.len() > count
        || (!inline_ && function.params.len() != count)
    {
        return fail(
            g,
            loc,
            format!("handler must have signature {}", action_signature(kind)),
        );
    }
    for param in &function.params {
        if let Some(reference) = param.ty.as_ref()
            && types::type_ref(g, reference, false)? != action_type(kind)
        {
            return fail(
                g,
                param.name.loc,
                format!("handler must have signature {}", action_signature(kind)),
            );
        }
    }
    Ok(())
}

/// Validate the action signature and reuse one adapter per setter and action kind.
pub fn event(g: &mut Generator<'_>, e: &ast::Expr, prop: &tags::Prop, loc: ast::Loc) -> Result<()> {
    // `ready ? save : undefined`: each present branch is a handler of its own
    // (the branch is picked at render time, `optional_event_value`)
    if let ast::ExprData::Cond(cond) = &e.data
        && optional_event(prop.kind)
    {
        for branch in [&cond.then, &cond.else_] {
            if !absent(branch) {
                event(g, branch, prop, loc)?;
            }
        }
        return Ok(());
    }
    // a child component's callback prop forwarded to a ui event is emitted
    // directly (`onClick={onSave}`), nothing to register
    if (optional_event(prop.kind)
        && crate::codegen::components::optional_forwarded_prop(g, e, prop.kind, loc)?)
        || crate::codegen::components::forwarded_prop(g, e, prop.kind, loc)?
    {
        return Ok(());
    }
    for previous in &g.events {
        if previous.loc == e.loc && previous.kind == prop.kind {
            return Ok(());
        }
    }
    let index: usize;
    match &e.data {
        ast::ExprData::Ident(name) => {
            if let Some(setter) = scope::setter_index(g, name) {
                let ty = g.states[setter].ty;
                let valid = match prop.kind {
                    Kind::TextAction => ty == Type::String,
                    Kind::BoolAction => ty == Type::Boolean,
                    Kind::IndexAction => ty.integer() || ty.float(),
                    _ => false,
                };
                if !valid {
                    return fail(
                        g,
                        e.loc,
                        format!(
                            "setter `{}` does not match {}",
                            String::from_utf8_lossy(name),
                            action_signature(prop.kind)
                        ),
                    );
                }
                index = match g
                    .methods
                    .iter()
                    .position(|m| m.setter == Some(setter) && m.action == Some(prop.kind))
                {
                    Some(index) => index,
                    None => {
                        let name = scope::unique_method(
                            g,
                            &format!(
                                "_{}",
                                crate::codegen::jsx::snake_case(&String::from_utf8_lossy(name))
                            ),
                        )?;
                        g.methods.push(Method {
                            name,
                            setter: Some(setter),
                            action: Some(prop.kind),
                            ..Method::default()
                        });
                        g.methods.len() - 1
                    }
                };
            } else if let Some(function) = inline_function(g, name) {
                // reads render locals: emitted here like an inline arrow
                let arrow = ast::Expr {
                    loc: e.loc,
                    data: ast::ExprData::Arrow(function),
                };
                index = register_arrow(g, prop, &arrow)?;
            } else if let (None, Some(function)) = (
                scope::source_method(g, name),
                functions::find_index(g, name),
            ) {
                // a plain function: handled as `(v) => fn(v)`, or `(v) => fn()`
                // when it ignores the event's value (TypeScript accepts a
                // callback with fewer parameters)
                let pass_value = !g.functions[function].params.is_empty();
                let arrow = function_arrow(e, prop.kind, pass_value);
                index = register_arrow(g, prop, &arrow)?;
            } else {
                let Some(method_index) = scope::source_method(g, name) else {
                    return fail(
                        g,
                        e.loc,
                        "event handler must be a component function, plain function, setter, or inline arrow",
                    );
                };
                let m = g.methods[method_index].clone();
                let ignores_value = prop.kind != Kind::Action
                    && m.func
                        .as_ref()
                        .is_some_and(|function| function.params.is_empty());
                if ignores_value {
                    // `function save() {}` on `onChange`: `(v) => save()`
                    let arrow = function_arrow(e, prop.kind, false);
                    index = register_arrow(g, prop, &arrow)?;
                    g.events.push(Event {
                        loc: e.loc,
                        kind: prop.kind,
                        method: index,
                    });
                    return Ok(());
                }
                signature(g, &m, prop.kind, loc, false)?;
                if let Some(previous) = m.action
                    && previous != prop.kind
                {
                    return fail(
                        g,
                        e.loc,
                        "handler is used with incompatible action signatures",
                    );
                }
                g.methods[method_index].action = Some(prop.kind);
                index = method_index;
            }
        }
        ast::ExprData::Arrow(_) => {
            index = register_arrow(g, prop, e)?;
        }
        ast::ExprData::Member(_) => {
            return fail(
                g,
                e.loc,
                "functions stored in objects can't be handlers yet; pass the function itself \
                 (`onClick={save}`) or call it in an inline arrow (`onClick={() => save()}`)",
            );
        }
        _ => {
            return fail(
                g,
                e.loc,
                "event handler must be a component function, plain function, setter, or inline arrow",
            );
        }
    }
    g.events.push(Event {
        loc: e.loc,
        kind: prop.kind,
        method: index,
    });
    Ok(())
}

/// Events whose field is an `Option<ui::…Action>` the value may leave empty.
pub(super) fn optional_event(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Action | Kind::TextAction | Kind::BoolAction | Kind::IndexAction
    )
}

/// `undefined` / `null` where a handler goes: no handler.
pub(super) fn absent(e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::NullLit => true,
        ast::ExprData::Ident(name) => name == b"undefined",
        _ => false,
    }
}

/// The `Option<ui::…Action>` of an event attribute. As in React the value is
/// evaluated at render time: `cond ? a : b` picks its branch then, an
/// `undefined` / `null` branch and an absent optional callback prop leave the
/// event empty.
pub fn optional_event_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    kind: Kind,
) -> Result<String> {
    if absent(e) {
        return Ok("None".to_owned());
    }
    if let ast::ExprData::Cond(cond) = &e.data {
        let test = expr::condition(g, c, &cond.cond)?.code;
        let then = optional_event_value(g, c, &cond.then, kind)?;
        let otherwise = optional_event_value(g, c, &cond.else_, kind)?;
        return Ok(format!("if {test} {{ {then} }} else {{ {otherwise} }}"));
    }
    if crate::codegen::components::optional_forwarded_prop(g, e, kind, e.loc)? {
        return Ok(expr::emit(g, c, e, None, false)?.code);
    }
    Ok(format!("Some({})", event_value(g, c, e, kind)?))
}

/// `ui::action(..)` for a registered handler expression.
pub fn event_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    kind: Kind,
) -> Result<String> {
    // A child component's callback prop forwarded to a ui event already is
    // the action value (`onClick={props.onSave}`).
    if crate::codegen::components::forwarded_prop(g, e, kind, e.loc)? {
        return Ok(expr::emit(g, c, e, None, false)?.code);
    }
    let events = g.events.clone();
    for event in events {
        if event.loc != e.loc || event.kind != kind {
            continue;
        }
        let method = g.methods[event.method].clone();
        if method.capture_name.is_some() {
            crate::codegen::captures::prepare(g, c, event.method)?;
        }
        let method = g.methods[event.method].clone();
        let root = g.current_component.is_none();
        if root {
            // the root shape is `ui::action(|s: &mut App| s._on_click1())`;
            // a handler that reads render locals gets a capture struct first
            let Some(class) = rust_text::id(g.name.as_bytes()) else {
                return fail(g, e.loc, "component name is not valid UTF-8");
            };
            let target = rust_text::id_at(g, method.name.as_bytes(), e.loc)?;
            let (params, call_args) = match kind {
                Kind::Action => (String::new(), String::new()),
                Kind::TextAction => (", v: &str".to_owned(), "v".to_owned()),
                Kind::BoolAction => (", v: bool".to_owned(), "v".to_owned()),
                Kind::IndexAction => (", v: usize".to_owned(), "v".to_owned()),
                _ => {
                    return fail(
                        g,
                        e.loc,
                        "event handler must be a component function, setter, or inline arrow",
                    );
                }
            };
            if method.captures.is_empty() {
                return Ok(format!(
                    "ui::{}(|s: &mut {class}{params}| s.{target}({call_args}))",
                    action_builder(kind)
                ));
            }
            let capture = crate::codegen::captures::value(g, &method)?;
            let arguments = if call_args.is_empty() {
                "&_capture".to_owned()
            } else {
                format!("&_capture, {call_args}")
            };
            return Ok(format!(
                "ui::{}({{\nlet _capture = {capture};\nmove |s: &mut {class}{params}| s.{target}({arguments})\n}})",
                action_builder(kind)
            ));
        }
        // a child component: the shared action emitter upgrades its local
        // state slot and passes captures
        return crate::codegen::generate::action_value(g, c, event.method, Some(kind));
    }
    fail(
        g,
        e.loc,
        "event handler must be a component function, setter, or inline arrow",
    )
}
