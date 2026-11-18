//! Action kinds: their Rust types and builders, state setters and handler parameter types.

use super::*;

pub fn is_action(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Action | Kind::TextAction | Kind::BoolAction | Kind::IndexAction
    )
}

pub fn action_type(kind: Kind) -> Type {
    match kind {
        Kind::Action => Type::Void,
        Kind::TextAction => Type::String,
        Kind::BoolAction => Type::Boolean,
        Kind::IndexAction => Type::Usize,
        _ => Type::Void,
    }
}

/// The `ui::*` constructor of an action kind.
pub fn action_builder(kind: Kind) -> &'static str {
    match kind {
        Kind::Action => "action",
        Kind::TextAction => "text_action",
        Kind::BoolAction => "bool_action",
        Kind::IndexAction => "index_action",
        _ => "action",
    }
}

/// The error text of an expected action signature.
pub fn action_signature(kind: Kind) -> &'static str {
    match kind {
        Kind::Action => "() => void",
        Kind::TextAction => "(value: string) => void",
        Kind::BoolAction => "(checked: boolean) => void",
        Kind::IndexAction => "(index: usize) => void",
        _ => "() => void",
    }
}

/// `fn _setName(&mut self, value: &str)` for a `useState` setter used as a
/// handler. The component state is a plain field.
pub fn emit_setter(g: &mut Generator<'_>, m: &Method) -> Result<String> {
    let Some(setter) = m.setter else {
        return fail(g, 0, "setter method without a state");
    };
    let state = g.states[setter].clone();
    let field = scope::state_field(g, state.name.as_bytes())?;
    let name = rust_text::id_at(g, m.name.as_bytes(), 0)?;
    let kind = m.action.unwrap_or(Kind::Action);
    let (parameter, value) = match kind {
        Kind::TextAction => (
            "&str",
            if state.ty == Type::String {
                "zant::Str::copy(value)".to_owned()
            } else {
                return fail(
                    g,
                    0,
                    format!(
                        "setter `{}` takes a string but its state is not a string",
                        m.name
                    ),
                );
            },
        ),
        Kind::BoolAction => ("bool", "value".to_owned()),
        Kind::IndexAction => (
            "usize",
            if state.ty == Type::Usize {
                "value".to_owned()
            } else if state.ty.integer() {
                format!(
                    "value.try_into().expect(\"index setter does not fit {}\")",
                    state.ty.rust(g)
                )
            } else if state.ty.float() {
                format!("value as {}", state.ty.rust(g))
            } else {
                return fail(g, 0, format!("setter `{}` cannot take an index", m.name));
            },
        ),
        _ => {
            return fail(g, 0, format!("setter `{}` has no action signature", m.name));
        }
    };
    Ok(format!(
        "fn {name}(&mut self, value: {parameter}) {{\nself.{field} = {value};\n}}\n"
    ))
}

/// The type of a method's parameter `index`: from a callback prop's signature
/// when it has one, else as [`parameter_type`].
pub fn method_param_type(
    g: &mut Generator<'_>,
    m: &Method,
    index: usize,
    param: &ast::Param,
) -> Result<Type> {
    let Some(params) = m.callback_params.as_ref() else {
        return parameter_type(g, param, m.action);
    };
    if index >= params.len() {
        return fail(
            g,
            param.name.loc,
            format!("the callback passes {} arguments", params.len()),
        );
    }
    if let Some(reference) = param.ty.as_ref() {
        let ty = types::type_ref(g, reference, false)?;
        if ty != params[index] {
            return fail(
                g,
                param.name.loc,
                "parameter type does not match the callback prop",
            );
        }
        return Ok(ty);
    }
    Ok(params[index])
}

/// Prefer an annotation, then infer a parameter from its registered action kind.
pub fn parameter_type(
    g: &mut Generator<'_>,
    param: &ast::Param,
    kind: Option<Kind>,
) -> Result<Type> {
    if param.optional || !param.destructure.is_empty() {
        return fail(
            g,
            param.name.loc,
            "optional or destructured parameters are not supported yet",
        );
    }
    if let Some(ty) = functions::param_type(g, param, b"this function")? {
        return Ok(ty);
    }
    let Some(action) = kind else {
        return fail(
            g,
            param.name.loc,
            format!(
                "parameter `{}` needs a type annotation",
                String::from_utf8_lossy(&param.name.name)
            ),
        );
    };
    Ok(action_type(action))
}
