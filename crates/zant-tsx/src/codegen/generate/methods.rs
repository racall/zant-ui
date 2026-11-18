//! Component methods: handlers, named functions and the inline methods that capture a render.

use super::*;

/// A handler bound through `self._state` needs a component built over a state
/// slot: a component classified without one (`stateful.rs`) would get an empty
/// `Weak` and drop the event silently, so that is a compile error.
pub(super) fn require_instance(
    g: &mut Generator<'_>,
    method: &crate::codegen::Method,
) -> Result<()> {
    let Some(index) = g.current_component else {
        return Ok(());
    };
    if g.components[index].stateful {
        return Ok(());
    }
    let loc = method.func.as_deref().map_or(0, |function| function.loc);
    let component = g.components[index].name.clone();
    fail(
        g,
        loc,
        format!(
            "internal: a handler of `{component}` needs the component's state slot, but \
             `{component}` was classified without one (zant-tsx `stateful.rs`); please \
             report this. Workaround: write the handler as an inline arrow function"
        ),
    )
}

/// Emit a method with inferred event parameters.
pub fn emit_method(g: &mut Generator<'_>, m: &crate::codegen::Method) -> Result<String> {
    if let Some(emitted) = &m.emitted {
        return Ok(emitted.clone());
    }
    if m.setter.is_some() {
        return match g.extensions.hooks.emit_setter {
            Some(hook) => hook(g, m),
            None => fail(
                g,
                0,
                "setter adapter methods need the event emitter installed",
            ),
        };
    }
    method(g, m, None, None)
}

/// `fn name(&mut self[, cap: &_CaptureN][, params…]) -> Ret { body }`.
pub fn method(
    g: &mut Generator<'_>,
    m: &crate::codegen::Method,
    source: Option<&scope::Context>,
    index: Option<usize>,
) -> Result<String> {
    let Some(f) = m.func.clone() else {
        return fail(g, 0, format!("method `{}` has no function body", m.name));
    };
    // a handler that calls a callback prop needs the runtime context (see
    // `action_cx`): it is the first parameter, before the event parameters
    let needs_cx = crate::codegen::action_cx::needs_cx(g, &f);
    let mut c = scope::Context {
        render: false,
        ret: m.ret,
        capture_source: source.map(|source| source.capture_source()),
        capture_method: index,
        effect_field: m.effect_field.clone(),
        cleanup: m.cleanup,
        cx_var: needs_cx.then(|| crate::codegen::action_cx::CX.to_owned()),
        // a throw nobody catches is reported; the handler returns (like a
        // browser's uncaught error, the app keeps running)
        throw_mode: if m.promise_callback {
            crate::codegen::exceptions::ThrowMode::Propagate
        } else if m.ret == Type::Void {
            crate::codegen::exceptions::ThrowMode::Report
        } else {
            crate::codegen::exceptions::ThrowMode::Forbid
        },
        ..scope::Context::default()
    };
    let mut params: Vec<String> = Vec::new();
    if needs_cx {
        params.push(format!(
            ", {}: &mut ui::ActionCx<'_>",
            crate::codegen::action_cx::CX
        ));
    }
    let mut text_parameter: Option<String> = None;
    for (i, p) in f.params.iter().enumerate() {
        let ty = match g.extensions.hooks.method_param {
            Some(hook) => hook(g, m, i, p)?,
            None => match p.ty.as_ref() {
                Some(reference) => types::type_ref(g, reference, false)?,
                None => {
                    return fail(
                        g,
                        p.name.loc,
                        format!(
                            "parameter `{}` needs a type (event parameter inference needs the event emitter installed)",
                            String::from_utf8_lossy(&p.name.name)
                        ),
                    );
                }
            },
        };
        let binding = scope::bind(g, &mut c, &p.name.name, ty, false)?;
        c.params.push(binding);
        let id = c.bindings[binding].rust.clone();
        // `ui::TextAction` passes `&str`; the TSX parameter is a string
        if m.action == Some(tags::Kind::TextAction) && ty == Type::String {
            params.push(format!(", {id}: &str"));
            text_parameter = Some(id);
        } else {
            params.push(format!(", {id}: {}", ty.rust(g)));
        }
    }
    if let Some(expected) = &m.callback_params
        && f.params.len() < expected.len()
    {
        for ty in &expected[f.params.len()..] {
            params.push(format!(", _: {}", ty.rust(g)));
        }
    } else if let Some(kind) = m.action
        && !matches!(kind, tags::Kind::Action)
        && f.params.is_empty()
    {
        let ty = match kind {
            tags::Kind::TextAction => "&str".to_owned(),
            other => components::action_value_type(other).rust(g).to_owned(),
        };
        params.push(format!(", _: {ty}"));
    }
    let mut ret = m.ret;
    let body = if m.promise_callback {
        let (body, ty) = crate::codegen::promises::reaction::body(g, &mut c, &f, m.ret)?;
        ret = ty;
        if let Some(index) = index {
            g.methods[index].ret = ret;
        }
        body
    } else {
        match &f.body {
            ast::Body::Block(statements) => method_block(g, &mut c, f.loc, statements)?,
            ast::Body::Expr(e) => method_expression(g, &mut c, e)?,
        }
    };
    let body = match &text_parameter {
        Some(id) => format!("let {id}: zant::Str = zant::Str::copy({id});\n{body}"),
        None => body,
    };
    let finished = scope::finish(g, &c, body)?;
    let name = rust_text::id_at(g, m.name.as_bytes(), 0)?;
    // captures are discovered while emitting the body, so read them back from
    // the generator rather than from `m` (captures.prepare passes a snapshot)
    let captures = match index {
        Some(index) => g.methods[index].captures.clone(),
        None => m.captures.clone(),
    };
    let receiver = if captures.is_empty() {
        "&mut self".to_owned()
    } else {
        format!(
            "&mut self, cap: &{}",
            m.capture_name.clone().unwrap_or_default()
        )
    };
    let returns = if m.promise_callback {
        let value = if ret == Type::Void {
            "()".to_owned()
        } else {
            ret.rust(g).to_owned()
        };
        format!(" -> Result<{value}, zant::tsx::Thrown>")
    } else if m.ret == Type::Void {
        String::new()
    } else {
        format!(" -> {}", m.ret.rust(g))
    };
    Ok(format!(
        "fn {name}({receiver}{}){returns} {{\n{finished}}}\n",
        params.join("")
    ))
}

/// A component method's statements, checked for complete returns after
/// emitting them (an exhaustive `switch` is only known then).
pub(in crate::codegen) fn method_block(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    statements: &[Box<ast::Stmt>],
) -> Result<String> {
    let body = stmt::statements(g, c, statements, false)?;
    if c.ret != Type::Void && !stmt::returns_all(statements) {
        return fail(g, loc, "function must return a value on every path");
    }
    Ok(body)
}

/// Treat a concise arrow as a statement or return according to its declared
/// type.
pub(in crate::codegen) fn method_expression(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
) -> Result<String> {
    if c.ret == Type::Void {
        return stmt::expression_statement(g, c, e);
    }
    let value = expr::emit(g, c, e, Some(c.ret), false)?;
    Ok(format!("return {};\n", value.code))
}
