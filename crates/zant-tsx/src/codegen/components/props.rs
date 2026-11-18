//! Reading a component's own props and forwarding props / callbacks to child components.

use super::*;

/// Reads a prop of the component being generated: `self.props.todo`, or the
/// whole object for a non-destructured `props`.
pub fn read_prop(g: &mut Generator<'_>, n: &str) -> Result<Option<Value>> {
    let Some(index) = g.current_component else {
        return Ok(None);
    };
    let component = g.components[index].clone();
    let Some(props) = component.props else {
        return Ok(None);
    };
    for (i, d) in component.destructure.iter().enumerate() {
        if String::from_utf8_lossy(&d.local.name) != n {
            continue;
        }
        g.component_props_read = true;
        let ty = field_type(g, props, &d.name).expect("destructured prop");
        let code = format!("self.props.{}", rust_text::id_at(g, &d.name, 0)?);
        if let Some(default) = component
            .destructure_defaults
            .get(i)
            .and_then(|default| default.as_ref())
        {
            return Ok(Some(functions::prop(
                g,
                default,
                ty,
                &code,
                &component.func,
            )?));
        }
        return Ok(Some(crate::codegen::scope::owned_read(g, ty, code)));
    }
    if component.destructure.is_empty() && component.param == n {
        g.component_props_read = true;
        return Ok(Some(crate::codegen::scope::owned_read(
            g,
            props,
            "self.props".to_owned(),
        )));
    }
    Ok(None)
}

/// `on_toggle(todo.id)` / `props.on_toggle(todo.id)` inside a child: calls the
/// callback prop. Null when the callee is not a callback prop.
pub(super) fn props_callback(g: &mut Generator<'_>, m: &ast::Member) -> Result<bool> {
    let ast::ExprData::Ident(object) = &m.object.data else {
        return Ok(false);
    };
    let name = String::from_utf8_lossy(object).into_owned();
    let Some(object) = read_prop(g, &name)? else {
        return Ok(false);
    };
    let Some(entry) = model::info(g, object.ty) else {
        return Ok(false);
    };
    if entry.kind != model::InfoKind::Object {
        return Ok(false);
    }
    let Some(ty) = field_type(g, object.ty, &m.name.name) else {
        return Ok(false);
    };
    Ok(callback_info(g, ty).is_some())
}

pub fn try_call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<Value>> {
    if g.current_component.is_none() {
        return Ok(None);
    }
    let callee_is_prop = match &call.callee.data {
        ast::ExprData::Ident(name) => {
            scope::local(c, name).is_none()
                && read_prop(g, &String::from_utf8_lossy(name))?.is_some()
        }
        // `props.onToggle(...)`, not an array method on a prop (`items.filter(...)`)
        ast::ExprData::Member(m) => match &m.object.data {
            ast::ExprData::Ident(object) => {
                scope::local(c, object).is_none() && props_callback(g, m)?
            }
            _ => false,
        },
        _ => false,
    };
    if !callee_is_prop {
        return Ok(None);
    }
    crate::codegen::expr::no_type_args(g, loc, call)?;
    let callee = expr::emit(g, c, &call.callee, None, false)?;
    let Some(entry) = callback_info(g, callee.ty) else {
        return Ok(None);
    };
    let optional = model::optional_child(g, callee.ty).is_some();
    if optional && !call.optional {
        return fail(
            g,
            loc,
            "optional callback props may be absent; call them as `onX?.(...)` or check first: `if (onX) onX(...)`",
        );
    }
    if c.render {
        return fail(
            g,
            loc,
            "callback props can only be called in event handlers",
        );
    }
    if call.args.len() != entry.params.len() {
        return fail(
            g,
            loc,
            format!("the callback takes {} arguments", entry.params.len()),
        );
    }
    let mut args: Vec<String> = Vec::new();
    for (arg, ty) in call.args.iter().zip(&entry.params) {
        args.push(expr::emit(g, c, arg, Some(*ty), false)?.code);
    }
    // The callback runs synchronously: the runtime `ActionCx` of this
    // handler is passed straight through (the handler takes it because it
    // calls a callback prop — see `action_cx`).
    let Some(cx) = c.cx_var.clone() else {
        return fail(
            g,
            loc,
            "a callback prop can only be called in an event handler",
        );
    };
    let kind = action_kind(&entry.params);
    let mut joined = cx.clone();
    for (value, ty) in args.iter().zip(&entry.params) {
        // `ui::text_action` hands the handler `&str`
        let value = if kind == Some(tags::Kind::TextAction) && *ty == Type::String {
            format!("({value}).as_str()")
        } else {
            value.clone()
        };
        joined.push_str(", ");
        joined.push_str(&value);
    }
    // `onX?.(…)` on an optional callback: like TypeScript, nothing runs
    // (and the arguments are not evaluated) when it is absent
    let (target, wrap) = if optional {
        ("_callback".to_owned(), true)
    } else {
        (format!("({})", callee.code), false)
    };
    let invoke = match kind {
        Some(_) => format!("{target}.call({joined})"),
        None => format!("{target}({joined})"),
    };
    Ok(Some(Value {
        ty: Type::Void,
        code: if wrap {
            format!(
                "if let Some(_callback) = &({}) {{ {invoke}; }}",
                callee.code
            )
        } else {
            invoke
        },
        ..Value::default()
    }))
}

/// The type and callback entry of `e` when it is this component's callback
/// prop (`on_save` or `props.on_save`).
pub(super) fn forwarded_callback(
    g: &mut Generator<'_>,
    e: &ast::Expr,
) -> Result<Option<(Type, model::Info)>> {
    if g.current_component.is_none() {
        return Ok(None);
    }
    let ty = match &e.data {
        ast::ExprData::Ident(name) => match read_prop(g, &String::from_utf8_lossy(name))? {
            Some(value) => value.ty,
            None => return Ok(None),
        },
        ast::ExprData::Member(m) => {
            let ast::ExprData::Ident(object) = &m.object.data else {
                return Ok(None);
            };
            let Some(object) = read_prop(g, &String::from_utf8_lossy(object))? else {
                return Ok(None);
            };
            match field_type(g, object.ty, &m.name.name) {
                Some(ty) => ty,
                None => return Ok(None),
            }
        }
        _ => return Ok(None),
    };
    Ok(callback_info(g, ty).map(|entry| (ty, entry)))
}

/// Whether `e` is this component's callback prop (`on_save` or
/// `props.on_save`) with the signature of the ui event `kind`.
pub fn forwarded_prop(
    g: &mut Generator<'_>,
    e: &ast::Expr,
    kind: tags::Kind,
    loc: ast::Loc,
) -> Result<bool> {
    let Some((ty, entry)) = forwarded_callback(g, e)? else {
        return Ok(false);
    };
    if model::optional_child(g, ty).is_some() {
        return fail(
            g,
            loc,
            "an optional callback prop is passed to this event as is only where the event may be absent \
             (`onClick={onClose}` on a built-in element); call it instead: `() => onClose?.()`",
        );
    }
    if action_kind(&entry.params) != Some(kind) {
        return fail(
            g,
            loc,
            "this callback prop does not have the event's signature",
        );
    }
    Ok(true)
}

/// Whether `e` is this component's optional callback prop
/// (`onClose?: () => void`) with the signature of the ui event `kind`: like
/// React, an absent callback leaves the event absent.
pub fn optional_forwarded_prop(
    g: &mut Generator<'_>,
    e: &ast::Expr,
    kind: tags::Kind,
    loc: ast::Loc,
) -> Result<bool> {
    let Some((ty, entry)) = forwarded_callback(g, e)? else {
        return Ok(false);
    };
    if model::optional_child(g, ty).is_none() {
        return Ok(false);
    }
    if action_kind(&entry.params) != Some(kind) {
        return fail(
            g,
            loc,
            "this callback prop does not have the event's signature",
        );
    }
    Ok(true)
}

/// A forwarded callback prop wrapped as the ui action of an event
/// (`on_click: Some(ui::Action::new(move |cx| callback.call(cx)))`).
pub fn forwarded_action_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    kind: tags::Kind,
) -> Result<String> {
    let callee = expr::emit(g, c, e, None, true)?;
    Ok(match kind {
        tags::Kind::Action => format!(
            "ui::Action::new({{\n    let callback = {};\n    move |cx| callback.call(cx)\n}})",
            callee.code
        ),
        tags::Kind::TextAction => format!(
            "ui::TextAction::new({{\n    let callback = {};\n    move |cx, value| callback.call(cx, value)\n}})",
            callee.code
        ),
        tags::Kind::BoolAction => format!(
            "ui::BoolAction::new({{\n    let callback = {};\n    move |cx, checked| callback.call(cx, checked)\n}})",
            callee.code
        ),
        tags::Kind::IndexAction => format!(
            "ui::IndexAction::new({{\n    let callback = {};\n    move |cx, index| callback.call(cx, index)\n}})",
            callee.code
        ),
        _ => callee.code,
    })
}

// ── call sites ────────────────────────────────────────────────────────
