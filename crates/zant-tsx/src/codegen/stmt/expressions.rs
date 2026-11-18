//! Expression statements: calls, assignments, updates and state setters.

use super::*;

pub fn expression_statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
) -> Result<String> {
    if let ast::ExprData::Unary(unary) = &e.data
        && unary.op == b"delete"
    {
        return crate::codegen::dicts::delete(g, c, &unary.operand);
    }
    if let ast::ExprData::Call(call) = &e.data {
        if let ast::ExprData::Ident(name) = &call.callee.data
            && scope::local(c, name).is_none()
            && let Some(index) = scope::setter_index(g, name)
        {
            return set(g, c, e.loc, index, &call.args);
        }
        return expr::call_statement(g, c, e);
    }
    if matches!(e.data, ast::ExprData::Assign(_) | ast::ExprData::Update(_)) {
        if let Some(hook) = g.extensions.hooks.ref_write
            && let Some(code) = hook(g, c, e)?
        {
            return Ok(code);
        }
        let target = match &e.data {
            ast::ExprData::Assign(assign) => &assign.target,
            ast::ExprData::Update(update) => &update.target,
            _ => unreachable!("assignment"),
        };
        let ast::ExprData::Ident(name) = &target.data else {
            return crate::codegen::place::assign(g, c, e, target);
        };
        let Some(binding) = scope::local(c, name) else {
            if let Some(index) = scope::state_index(g, name) {
                return fail(
                    g,
                    target.loc,
                    format!("use {}(…) to update state", g.states[index].setter),
                );
            }
            return fail(
                g,
                target.loc,
                format!("unknown local `{}`", String::from_utf8_lossy(name)),
            );
        };
        if !binding.mutable {
            return fail(
                g,
                target.loc,
                format!(
                    "`{}` is const; declare it with let to assign it",
                    String::from_utf8_lossy(name)
                ),
            );
        }
        let binding_index = scope::local_index(c, name).expect("local binding");
        c.bindings[binding_index].assigned = true;
        // the new value starts a new generation only after the right side
        // ran: `current = current.next` still reads the narrowed `current`
        let rust = c.bindings[binding_index].rust.clone();
        let ty = c.bindings[binding_index].ty;
        if let ast::ExprData::Update(update) = &e.data {
            if !ty.numeric() {
                return fail(g, e.loc, "++ and -- require a number");
            }
            let one = if ty.float() { "1.0" } else { "1" };
            c.bindings[binding_index].assignments += 1;
            return Ok(format!(
                "{rust} {} {one};\n",
                if update.op == b"++" { "+=" } else { "-=" }
            ));
        }
        let assign = match &e.data {
            ast::ExprData::Assign(assign) => assign,
            _ => unreachable!("assignment"),
        };
        if assign.op == b"=" {
            let v = expr::emit(g, c, &assign.value, Some(ty), false)?;
            // a new value: earlier uses saw the old one
            c.bindings[binding_index].fresh = crate::codegen::place::fresh(&assign.value);
            c.bindings[binding_index].escaped = false;
            c.bindings[binding_index].assignments += 1;
            return Ok(format!("{rust} = {};\n", v.code));
        }
        // Reuse the arithmetic checker so integer `/=` and `%=` keep Rust's
        // truncating semantics rather than accidentally emitting JS division.
        let lhs = Box::new(ast::Expr {
            loc: target.loc,
            data: ast::ExprData::Ident(name.clone()),
        });
        let binary = ast::Binary {
            op: assign.op[..assign.op.len() - 1].to_vec(),
            lhs,
            rhs: assign.value.clone(),
        };
        let value = expr::binary(g, c, e.loc, &binary, Some(ty), false)?;
        let value = types::coerce(g, value, ty, e.loc)?;
        c.bindings[binding_index].assignments += 1;
        return Ok(format!("{rust} = {};\n", value.code));
    }
    fail(
        g,
        e.loc,
        "only calls, assignments, and updates are supported as expression statements",
    )
}

/// Ordinary setters use snapshots; a functional update reads the current state
/// inside its own block.
pub fn set(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    index: usize,
    args: &[Box<ast::Expr>],
) -> Result<String> {
    if c.render {
        return fail(g, loc, "state can only be set in event handlers");
    }
    if args.len() != 1 {
        return fail(g, loc, "state setter takes one argument");
    }
    let state = g.states[index].clone();
    let field = scope::state_field(g, state.name.as_bytes())?;
    let arg = &args[0];
    if let ast::ExprData::Arrow(function) = &arg.data {
        if function.params.len() != 1 || !matches!(function.body, ast::Body::Expr(_)) {
            return fail(
                g,
                arg.loc,
                "functional update needs one parameter and an expression body",
            );
        }
        if let Some(reference) = function.params[0].ty.as_ref()
            && types::type_ref(g, reference, false)? != state.ty
        {
            return fail(
                g,
                reference.loc,
                "functional update parameter must match the state type",
            );
        }
        let depth = c.active.len();
        let binding = scope::bind(g, c, &function.params[0].name.name, state.ty, false)?;
        let current = if state.ty.copy(g) {
            format!("self.{field}")
        } else {
            format!("self.{field}.clone()")
        };
        c.bindings[binding].init = Some(current);
        let body = match &function.body {
            ast::Body::Expr(body) => body,
            ast::Body::Block(_) => unreachable!("checked expression body"),
        };
        let value = expr::emit(g, c, body, Some(state.ty), false)?;
        let update = state_write(g, &field, state.ty, &value.code)?;
        c.active.truncate(depth);
        return Ok(format!(
            "{{\n{}{binding}{}{update}\n}}\n",
            scope::PLACEHOLDER_START,
            scope::PLACEHOLDER_END
        ));
    }
    let value = expr::emit(g, c, arg, Some(state.ty), false)?;
    Ok(format!(
        "{}\n",
        state_write(g, &field, state.ty, &value.code)?
    ))
}

pub fn state_write(g: &mut Generator<'_>, field: &str, ty: Type, v: &str) -> Result<String> {
    let _ = (g, ty);
    Ok(format!("self.{field} = {v};"))
}
