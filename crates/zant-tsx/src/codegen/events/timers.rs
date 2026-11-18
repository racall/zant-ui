//! Timers: `setTimeout` / `setInterval` handlers found by the scan and the calls that start them.

use super::*;

/// `setTimeout(() => …, ms)` registers its callback as a method while the
/// component is scanned. The lowering lives here because the callback behaves
/// like an event arrow (captures, no parameters).
pub(super) fn scan_timer(g: &mut Generator<'_>, call: &ast::Call) -> Result<()> {
    if !call_ident(call, "setTimeout") && !call_ident(call, "setInterval") {
        return Ok(());
    }
    let loc = call.args.first().map_or(0, |arg| arg.loc);
    if call.args.len() != 2 {
        return fail(g, loc, "setTimeout(() => …, ms)");
    }
    let callback = &call.args[0];
    let arrow = match &callback.data {
        ast::ExprData::Arrow(function) => ast::Function {
            type_params: function.type_params.clone(),
            loc: callback.loc,
            name: Vec::new(),
            params: function.params.clone(),
            ret: function.ret.clone(),
            body: function.body.clone(),
        },
        ast::ExprData::Ident(_) => {
            let forwarded = forward_arrow(callback, 0);
            let ast::ExprData::Arrow(function) = forwarded.data else {
                unreachable!("forward arrow")
            };
            *function
        }
        _ => {
            return fail(
                g,
                callback.loc,
                "the callback is an arrow or a function name",
            );
        }
    };
    if !arrow.params.is_empty() {
        return fail(g, callback.loc, "a timer callback takes no parameters");
    }
    g.counter += 1;
    let method = g.methods.len();
    app_state::with_module(|state| {
        state.desktop_callbacks.push(app_state::DesktopCallback {
            expr: (**callback).clone(),
            method,
        })
    });
    let name = scope::unique_method(g, &format!("_timeout{}", g.counter))?;
    let capture_name = scope::unique_method(g, &format!("_TimeoutCapture{}", g.counter))?;
    g.methods.push(Method {
        name,
        func: Some(Box::new(arrow)),
        action: Some(Kind::Action),
        capture_name: Some(capture_name),
        ..Method::default()
    });
    Ok(())
}

/// Values a timer keeps after the frame that scheduled it is gone: numbers,
/// booleans and enums, and optionals of them.
pub(super) fn timer_plain(g: &Generator<'_>, ty: Type) -> bool {
    if ty == Type::Boolean || ty.numeric() {
        return true;
    }
    if let Some(child) = model::optional_child(g, ty) {
        return timer_plain(g, child);
    }
    model::info(g, ty).is_some_and(|entry| entry.kind == model::InfoKind::Enumeration)
}

/// `setTimeout(…)` / `clearTimeout(id)` in a handler.
pub fn timer_call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<crate::codegen::types::Value>> {
    // `setTimeout` / `setInterval`, `clearTimeout` / `clearInterval`
    let Some(name) = ["setTimeout", "setInterval", "clearTimeout", "clearInterval"]
        .into_iter()
        .find(|name| call_ident(call, name))
    else {
        return Ok(None);
    };
    let set = name.starts_with("set");
    let interval = name.ends_with("Interval");
    if crate::codegen::builtins::shadowed(g, c, name) {
        return Ok(None);
    }
    if c.render || c.constant {
        return fail(
            g,
            loc,
            "{name} runs in event handlers and effects, not while rendering",
        );
    }
    if !set {
        if call.args.len() != 1 {
            return fail(g, loc, format!("{name}(id)"));
        }
        let id = expr::emit(g, c, &call.args[0], Some(Type::F64), false)?;
        let id = types::coerce(g, id, Type::F64, call.args[0].loc)?;
        let clear = if interval { "clear_interval" } else { "clear" };
        return Ok(Some(crate::codegen::types::Value::new(
            Type::Void,
            format!("tsx::timers::{clear}({});", id.code),
        )));
    }
    if c.no_self {
        return fail(
            g,
            loc,
            format!("{name} runs in a component's handlers (a plain function has none)"),
        );
    }
    if let Some(component) = g.current_component
        && g.components[component].props.is_some()
    {
        return fail(
            g,
            loc,
            "timers belong to the root component or a window's (components without props): they outlive other components; set it there and pass the result down",
        );
    }
    if call.args.len() != 2 {
        return fail(g, loc, format!("{name}(() => …, ms)"));
    }
    let callback = &call.args[0];
    let Some(info) = app_state::with_module(|state| {
        state
            .desktop_callbacks
            .iter()
            .find(|item| item.expr == **callback)
            .cloned()
    }) else {
        return fail(g, callback.loc, "the timer callback was not scanned");
    };
    crate::codegen::captures::prepare(g, c, info.method)?;
    let method = g.methods[info.method].clone();
    // the capture struct is built where `setTimeout` is called: re-read each
    // value in the enclosing context (`cap.doubled` inside an effect, a local
    // of the handler, …) instead of the render binding the snapshot recorded
    for (index, capture) in method.captures.iter().enumerate() {
        if let Some(value) = crate::codegen::captures::read(g, c, &capture.name)? {
            g.methods[info.method].captures[index].source = value.code;
        }
    }
    for capture in &method.captures {
        if !timer_plain(g, capture.ty) {
            return fail(
                g,
                callback.loc,
                format!(
                    "a timer callback can capture numbers, booleans and enums (`{}` is not one); keep other values in state and read them when it runs",
                    capture.name
                ),
            );
        }
    }
    let action = crate::codegen::generate::action_value(g, c, info.method, Some(Kind::Action))?;
    let ms = expr::emit(g, c, &call.args[1], Some(Type::F64), false)?;
    let ms = types::coerce(g, ms, Type::F64, call.args[1].loc)?;
    Ok(Some(crate::codegen::types::Value::new(
        Type::F64,
        format!(
            "tsx::timers::{}({}, {action})",
            if interval { "set_interval" } else { "set" },
            ms.code
        ),
    )))
}
