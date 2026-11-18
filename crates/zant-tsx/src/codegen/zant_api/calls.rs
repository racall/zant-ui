//! Emitting calls into the `zant` API, with the special cases (`keyLabel`, `titlebarInsets`).

use super::*;

pub(super) fn emit_call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    function: &Function,
    rust: &str,
) -> Result<Value> {
    if !call.type_args.is_empty() {
        return fail(
            g,
            loc,
            format!("`{}` takes no type arguments", function.name),
        );
    }
    if call.args.len() != function.params.len() {
        return fail(
            g,
            loc,
            format!(
                "`{}` takes {} argument(s), got {}",
                function.name,
                function.params.len(),
                call.args.len()
            ),
        );
    }
    if c.constant {
        return fail(
            g,
            loc,
            format!("`{}` cannot run in a top-level constant", function.name),
        );
    }
    if function.effect && c.render {
        return fail(
            g,
            loc,
            format!(
                "`{}` changes the desktop; call it in an event handler or an effect, not while rendering",
                function.name
            ),
        );
    }
    let mut arguments: Vec<String> = Vec::new();
    for (argument, kind) in call.args.iter().zip(function.params.iter()) {
        let ty = kind_type(g, *kind)?;
        let value = expr::emit(g, c, argument, Some(ty), false)?;
        let code = types::coerce(g, value, ty, argument.loc)?.code;
        // the runtime's platform functions take `&str`
        arguments.push(if *kind == Kind::String {
            format!("({code}).as_str()")
        } else {
            code
        });
    }
    Ok(Value::new(
        kind_type(g, function.ret)?,
        format!("{rust}({})", arguments.join(", ")),
    ))
}

pub(super) fn kind_type(g: &mut Generator<'_>, kind: Kind) -> Result<Type> {
    Ok(match kind {
        Kind::Void => Type::Void,
        Kind::String => Type::String,
        Kind::Boolean => Type::Boolean,
        Kind::Number => Type::F64,
        Kind::Bytes => rust_imports::bytes(g)?,
        Kind::OptString => model::optional(g, Type::String)?,
        Kind::OptBytes => {
            let bytes = bytes_owned(g)?;
            model::optional(g, bytes)?
        }
        Kind::OptImage => match g.extensions.hooks.image_type {
            Some(hook) => {
                let image = hook(g)?;
                model::optional(g, image)?
            }
            None => {
                return fail(
                    g,
                    0,
                    "`Image` needs the JSX hooks installed; `assetImage` returns one",
                );
            }
        },
    })
}

/// `tsx::Bytes` (`Rc<[u8]>`): the runtime's byte buffers (clipboard images).
pub(super) fn bytes_owned(g: &mut Generator<'_>) -> Result<Type> {
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.rust == "tsx::Bytes" {
            return Ok(Type::Compound(index as u32));
        }
    }
    model::add(
        g,
        model::Info {
            kind: model::InfoKind::Array,
            rust: "tsx::Bytes".to_owned(),
            child: Type::U8,
            copy: false,
            ..model::Info::default()
        },
    )
}

/// `keyLabel("mod+shift+s")`: the platform's label (a runtime call in Rust).
pub(super) fn key_label(g: &mut Generator<'_>, loc: ast::Loc, call: &ast::Call) -> Result<Value> {
    match call.args.first().map(|argument| &argument.data) {
        Some(ast::ExprData::String(value)) if call.args.len() == 1 => Ok(Value::new(
            Type::String,
            format!(
                "tsx::platform::key_label({})",
                rust_text::quote_at(g, value, loc)?
            ),
        )),
        _ => fail(
            g,
            loc,
            "keyLabel takes a string literal: keyLabel(\"mod+f\")",
        ),
    }
}

/// `titlebarInsets()`: `{ left, right }`, the space the system's window
/// buttons take in a hidden title bar.
pub(super) fn titlebar_insets(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Value> {
    let _ = c;
    if !call.args.is_empty() {
        return fail(g, loc, "titlebarInsets() takes no arguments");
    }
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.rust == "zant::platform::platform::TitlebarInsets" {
            return Ok(Value::new(
                Type::Compound(index as u32),
                "ui::titlebar_insets()",
            ));
        }
    }
    let ty = model::add(
        g,
        model::Info {
            kind: model::InfoKind::Object,
            rust: "zant::platform::platform::TitlebarInsets".to_owned(),
            fields: vec![
                model::Field {
                    name: b"left".to_vec(),
                    ty: Type::F32,
                    optional: false,
                    rust: None,
                },
                model::Field {
                    name: b"right".to_vec(),
                    ty: Type::F32,
                    optional: false,
                    rust: None,
                },
            ],
            copy: true,
            ..model::Info::default()
        },
    )?;
    Ok(Value::new(ty, "ui::titlebar_insets()"))
}
