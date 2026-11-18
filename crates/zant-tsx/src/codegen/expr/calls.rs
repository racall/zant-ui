//! Calls: function, method and builtin calls as values and as statements.

use super::*;

/// Reject explicit type arguments on a call with a concrete signature.
pub(crate) fn no_type_args(g: &mut Generator<'_>, loc: ast::Loc, call: &ast::Call) -> Result<()> {
    if !call.type_args.is_empty() {
        let name = match &call.callee.data {
            ast::ExprData::Ident(name) => name,
            ast::ExprData::Member(m) => &m.name.name,
            _ => b"function".as_slice(),
        };
        return fail(
            g,
            loc,
            format!(
                "`{}` takes no type arguments",
                String::from_utf8_lossy(name)
            ),
        );
    }
    Ok(())
}

pub fn call_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    expected: Option<Type>,
) -> Result<Value> {
    if let Some(error) = crate::codegen::exceptions::construct(g, c, loc, call)? {
        return Ok(error);
    }
    if let Some(regexp) = crate::codegen::regexps::construct(g, c, loc, call)? {
        return Ok(regexp);
    }
    if let Some(value) = crate::codegen::promises::call(g, c, loc, call)? {
        return Ok(value);
    }
    if call.is_new {
        return crate::codegen::dicts::construct(g, c, loc, call, expected);
    }
    if let Some(value) = crate::codegen::closures::call_name(g, c, loc, call)? {
        return Ok(value);
    }
    for hook in g.extensions.call_before_functions.clone() {
        if let Some(value) = hook(g, c, loc, call)? {
            return Ok(value);
        }
    }
    if let Some(value) = functions::try_call(g, c, loc, call)? {
        return Ok(value);
    }
    for hook in g.extensions.call_before_builtins.clone() {
        if let Some(value) = hook(g, c, loc, call)? {
            return Ok(value);
        }
    }
    // the callback-prop emitter above handles `onSave?.()`; nothing else
    // here can be absent
    if call.optional {
        return fail(
            g,
            loc,
            "optional calls (`f?.()`) work on optional callback props (`onSave?.()`); call this function directly",
        );
    }
    if let Some(value) = crate::codegen::json::call(g, c, loc, call, expected)? {
        return Ok(value);
    }
    if let Some(value) = crate::codegen::dicts::object_call(g, c, loc, call)? {
        return Ok(value);
    }
    if let Some(value) = builtins::try_call(g, c, loc, call)? {
        return Ok(value);
    }
    no_type_args(g, loc, call)?;
    if matches!(call.callee.data, ast::ExprData::Member(_)) {
        return match g.extensions.array_methods {
            Some(hook) => hook(g, c, loc, call, expected),
            None => fail(
                g,
                loc,
                "array and string methods need the method emitter installed",
            ),
        };
    }
    let ast::ExprData::Ident(name) = &call.callee.data else {
        // `make(1)(2)`, `fns[0](x)`: a function value
        let callee = emit(g, c, &call.callee, None, false)?;
        if crate::codegen::closures::signature(g, callee.ty).is_some() {
            return crate::codegen::closures::call_value(g, c, loc, callee, &call.args);
        }
        return fail(
            g,
            loc,
            "only functions can be called; console.log is a statement",
        );
    };
    let name = String::from_utf8_lossy(name).into_owned();
    if scope::local(c, name.as_bytes()).is_none() && name == "useTheme" {
        if !c.render {
            return fail(
                g,
                loc,
                "useTheme() is read while rendering; capture the value in handlers",
            );
        }
        if !call.args.is_empty() {
            return fail(g, loc, "useTheme() takes no arguments");
        }
        return Ok(Value::new(model::theme_type(g)?, "ui::current_theme()"));
    }
    if scope::local(c, name.as_bytes()).is_some() {
        return fail(
            g,
            loc,
            format!("`{name}` is a local value, not a component function"),
        );
    }
    if crate::codegen::hook_name(g, &name) {
        return fail(
            g,
            loc,
            "hooks must be called at the top level of the component",
        );
    }
    if let Some(value) = crate::codegen::local_functions::call(g, c, loc, name.as_bytes(), call)? {
        return Ok(value);
    }
    if scope::setter_index(g, name.as_bytes()).is_some() {
        return fail(
            g,
            loc,
            if c.render {
                "state can only be set in event handlers"
            } else {
                "setter calls must be statements"
            },
        );
    }
    let Some(index) = scope::source_method(g, name.as_bytes()) else {
        return fail(g, loc, format!("unknown function `{name}`"));
    };
    let method = g.methods[index].clone();
    let mut receiver = "self".to_owned();
    let mut target = self_method_target(g, &name)?;
    if method.inline_only {
        let Some(hook) = g.extensions.hooks.nested else {
            return fail(
                g,
                loc,
                "component functions that read render locals need the component emitter installed",
            );
        };
        let nested = hook(g, c, loc, index)?;
        receiver = nested.receiver;
        target = nested.target;
    }
    let Some(function) = method.func.as_ref() else {
        return fail(g, loc, format!("`{name}` is not a function"));
    };
    let function = function.clone();
    // a called function that calls a callback prop takes the runtime context;
    // only a handler (which has one) can call it
    let cx = if crate::codegen::action_cx::needs_cx(g, &function) {
        match c.cx_var.clone() {
            Some(cx) => Some(cx),
            None => {
                return fail(
                    g,
                    loc,
                    format!(
                        "`{name}` calls a callback prop, so it only runs from an event handler"
                    ),
                );
            }
        }
    } else {
        None
    };
    let required = functions::required(&function.params);
    if call.args.len() < required || call.args.len() > function.params.len() {
        return fail(
            g,
            loc,
            format!("`{name}` expects {} arguments", function.params.len()),
        );
    }
    let mut arguments: Vec<String> = Vec::new();
    if method.inline_only {
        // `self.method(&_capture, …)`: the receiver is the instance, not the
        // capture struct
        arguments.push("self".to_owned());
        if receiver != "self" {
            arguments.push(format!("&({receiver})"));
        }
    }
    if let Some(cx) = &cx {
        arguments.push(cx.clone());
    }
    for (param_index, param) in function.params.iter().enumerate() {
        let parameter_type = match g.extensions.hooks.method_param {
            Some(hook) => hook(g, &method, param_index, param)?,
            None => match param.ty.as_ref() {
                Some(reference) => types::type_ref(g, reference, false)?,
                None => {
                    return fail(
                        g,
                        param.name.loc,
                        format!(
                            "parameter `{}` needs a type",
                            String::from_utf8_lossy(&param.name.name)
                        ),
                    );
                }
            },
        };
        if param_index >= call.args.len() {
            arguments.push(functions::missing(g, param, parameter_type)?);
            continue;
        }
        let value = emit(g, c, &call.args[param_index], Some(parameter_type), false)?;
        arguments.push(value.code);
    }
    Ok(Value {
        ty: method.ret,
        code: format!("{target}({})", arguments.join(", ")),
        ..Value::default()
    })
}

/// `self.method` for a plain method.
pub(super) fn self_method_target(g: &mut Generator<'_>, name: &str) -> Result<String> {
    let id = rust_text::id_at(g, name.as_bytes(), 0)?;
    Ok(format!("self.{id}"))
}

/// `console.log` is accepted only as a statement and emits space-separated
/// values plus a newline.
pub fn call_statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
) -> Result<String> {
    let ast::ExprData::Call(call) = &e.data else {
        unreachable!("call statement")
    };
    if let ast::ExprData::Member(member) = &call.callee.data {
        let console = matches!(&member.object.data, ast::ExprData::Ident(n) if n == b"console");
        if console && member.name.name == b"log" {
            no_type_args(g, e.loc, call)?;
            let mut format = Format::default();
            for (index, arg) in call.args.iter().enumerate() {
                if index > 0 {
                    format.text(b" ");
                }
                let value = emit(g, c, arg, None, false)?;
                format.value(g, value, arg.loc)?;
            }
            format.text(b"\n");
            return Ok(format!("{};\n", format.finish(g, true)?));
        }
    }
    let v = emit(g, c, e, None, false)?;
    if v.ty == Type::Void {
        Ok(format!("{};\n", v.code))
    } else {
        Ok(format!("let _ = {};\n", v.code))
    }
}
