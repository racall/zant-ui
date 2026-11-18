//! Collecting app hook calls and their statements / declarations in a component.

use super::*;

/// Registers the callback of `useX(…)` (a top-level statement of a component)
/// as a method, before handlers are scanned.
pub fn collect(g: &mut Generator<'_>, e: &ast::Expr) -> Result<()> {
    let ast::ExprData::Call(call) = &e.data else {
        unreachable!("hook call")
    };
    let Some(name) = hooks::call_name(e) else {
        return Ok(());
    };
    let hook = find(&name).expect("app hook");
    match role(g) {
        Role::Root if !hook.root => {
            return fail(
                g,
                e.loc,
                format!(
                    "{name} belongs to a window's component (`createWindow(Panel, …)`); the main window has no {name}"
                ),
            );
        }
        Role::Window if !hook.window => {
            return fail(
                g,
                e.loc,
                format!("{name} belongs to the root component (the app); call it there"),
            );
        }
        Role::Child => {
            return fail(
                g,
                e.loc,
                format!(
                    "{name} belongs to the root component or a window's component (`createWindow(Panel, …)`); in any other component it never runs, call it there"
                ),
            );
        }
        _ => {}
    }
    if call.args.len() != 1 {
        return fail(g, e.loc, format!("{name} takes one function"));
    }
    // `useKeyDown(onKey)`: a function of the component or file, called with
    // the hook's argument like the inline `(e) => onKey(e)` would be
    let forwarded;
    let function = match &call.args[0].data {
        ast::ExprData::Arrow(function) => function,
        ast::ExprData::Ident(_) => {
            forwarded = forwarding_arrow(&call.args[0], hook.param != HookParam::None);
            &forwarded
        }
        _ => {
            return fail(
                g,
                e.loc,
                format!("{name} takes a function: an inline arrow or the name of a function"),
            );
        }
    };
    if function.params.len() > usize::from(hook.param != HookParam::None) {
        let expected = match hook.param {
            HookParam::None => "no parameters",
            HookParam::Key => "(e: KeyEvent)",
            HookParam::Focused => "(focused: boolean)",
        };
        return fail(
            g,
            function.loc,
            format!("the {name} callback takes {expected}"),
        );
    }
    let method = method_name(g, hook);
    if scope::method_index(g, method.as_bytes()).is_some() {
        return fail(
            g,
            e.loc,
            format!("{name} is used twice, or `{method}` is also a function of this component"),
        );
    }
    let mut function = (**function).clone();
    // The arrow's parameter is untyped (`(e) => …`); the hook supplies it.
    // Annotating it here keeps the event-handler inference (`events`) out of
    // the app layer.
    match hook.param {
        HookParam::Key => annotate_param(g, &mut function, "KeyEvent")?,
        HookParam::Focused => annotate_param(g, &mut function, "boolean")?,
        HookParam::None => {}
    }
    g.methods.push(crate::codegen::Method {
        name: method,
        func: Some(Box::new(function)),
        ret: if hook.returns {
            Type::Boolean
        } else {
            Type::Void
        },
        ..crate::codegen::Method::default()
    });
    Ok(())
}

pub(super) fn annotate_param(
    g: &mut Generator<'_>,
    function: &mut ast::Function,
    name: &str,
) -> Result<()> {
    let Some(parameter) = function.params.first_mut() else {
        return Ok(());
    };
    if parameter.ty.is_none() {
        let _ = g;
        parameter.ty = Some(ast::TypeRef {
            args: Vec::new(),
            name: name.as_bytes().to_vec(),
            loc: parameter.name.loc,
            detail: None,
        });
    }
    Ok(())
}

/// The render-time statement: the method is emitted by the component's
/// inherent impl, so nothing is emitted here.
pub fn statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    top: bool,
) -> Result<String> {
    let Some(name) = hooks::call_name(e) else {
        return fail(g, e.loc, "hook statements are not supported here");
    };
    if desktop_hooks::is_hook(&name) {
        let value = desktop_hooks::value(g, c, e, top)?;
        return Ok(format!("let _ = {};\n", value.code));
    }
    let Some(hook) = find(&name) else {
        if name == "useEffect" {
            return hooks::effect(g, c, e, top);
        }
        return fail(g, e.loc, "keep the value of this hook: const x = …");
    };
    if !c.render || !top {
        return fail(
            g,
            e.loc,
            "hooks must be called at the top level of the component",
        );
    }
    let method = method_name(g, hook);
    if scope::method_index(g, method.as_bytes()).is_none() {
        return fail(g, e.loc, format!("{name} was not collected"));
    }
    // Nothing to emit: the hook's method is a normal method of the component's
    // inherent impl, and `lifecycle` forwards the `impl ui::App` methods to it.
    Ok(String::new())
}

/// `(e) => callee(e)` (or `() => callee()`) for a named hook callback.
pub(super) fn forwarding_arrow(callee: &ast::Expr, takes_argument: bool) -> Box<ast::Function> {
    let loc = callee.loc;
    let argument = ast::Name {
        name: b"_event".to_vec(),
        loc,
    };
    let mut params = Vec::new();
    let mut args = Vec::new();
    if takes_argument {
        params.push(ast::Param {
            name: argument.clone(),
            ty: None,
            optional: false,
            destructure: Vec::new(),
            destructure_defaults: Vec::new(),
            rest: false,
            default: None,
        });
        args.push(Box::new(ast::Expr {
            loc,
            data: ast::ExprData::Ident(argument.name),
        }));
    }
    Box::new(ast::Function {
        type_params: Vec::new(),
        loc,
        name: Vec::new(),
        params,
        ret: None,
        body: ast::Body::Expr(Box::new(ast::Expr {
            loc,
            data: ast::ExprData::Call(ast::Call {
                is_new: false,
                callee: Box::new(callee.clone()),
                type_args: Vec::new(),
                args,
                optional: false,
            }),
        })),
    })
}

/// Declaration-position hooks (`useHotkey`, `useTray`, `useMemo`, `useStore`).
pub fn declaration(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    d: &ast::Decl,
    top: bool,
) -> Result<String> {
    let Some(init) = d.init.as_deref() else {
        return fail(g, loc, "hook declarations need an initializer");
    };
    let Some(name) = hooks::call_name(init) else {
        return fail(g, loc, "hook declarations are not supported here");
    };
    if desktop_hooks::is_hook(&name) {
        return desktop_hooks::declaration(g, c, loc, d, top);
    }
    match name.as_str() {
        "useMemo" => hooks::memo(g, c, loc, d, top),
        "useStore" => stores::use_store(g, c, loc, d, top),
        _ => fail(g, loc, format!("hook `{name}` cannot be declared here")),
    }
}
