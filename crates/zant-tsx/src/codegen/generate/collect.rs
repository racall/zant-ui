//! The scan before a component is emitted: hooks, state, component functions and render locals.

use super::*;

/// Collect top-level hooks and component functions before resolving JSX event
/// references.
pub fn collect(g: &mut Generator<'_>, f: &ast::Function) -> Result<()> {
    let ast::Body::Block(statements) = &f.body else {
        return fail(g, f.loc, "a component body must be a block");
    };
    for s in statements {
        match &s.data {
            ast::StmtData::Decl(d) => collect_declaration(g, s.loc, d)?,
            ast::StmtData::Func(func) => collect_method(g, &func.name, func)?,
            ast::StmtData::Expr(e) => {
                if let Some(name) = hooks::call_name(e) {
                    if name == "useEffect" {
                        hooks::collect_effect(g, e)?;
                    }
                    if (g.extensions.is_app_hook)(&name) {
                        crate::codegen::app_hooks::collect(g, e)?;
                    }
                }
            }
            _ => {}
        }
    }
    for s in statements {
        render_names(g, s);
    }
    mark_inline(g)?;
    // desktop hooks may take a function that reads render locals (marked above)
    for s in statements {
        let e = match &s.data {
            ast::StmtData::Decl(d) => match d.init.as_deref() {
                Some(e) => e,
                None => continue,
            },
            ast::StmtData::Expr(e) => e,
            _ => continue,
        };
        if let Some(name) = hooks::call_name(e)
            && (g.extensions.is_desktop_hook)(&name)
        {
            crate::codegen::desktop_hooks::collect(g, e)?;
        }
    }
    for state in g.states.clone() {
        if scope::method_index(g, state.name.as_bytes()).is_some() || state.name == "render" {
            return fail(
                g,
                f.loc,
                format!("state `{}` conflicts with a method; rename it", state.name),
            );
        }
    }
    Ok(())
}

/// Component functions that read render locals (also through another such
/// function) are emitted where they are passed, capturing that render.
pub(super) fn mark_inline(g: &mut Generator<'_>) -> Result<()> {
    let mut names: Vec<String> = Vec::new();
    // refs are fields of the component: functions read them directly
    for name in g.render_names.clone() {
        if scope::state_index(g, name.as_bytes()).is_none() {
            names.push(name);
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for index in 0..g.methods.len() {
            if !g.methods[index].user || g.methods[index].inline_only {
                continue;
            }
            let Some(function) = g.methods[index].func.clone() else {
                continue;
            };
            if !crate::codegen::free_names::function(
                &function,
                &names.iter().map(|name| name.as_str()).collect::<Vec<_>>(),
            ) {
                continue;
            }
            g.methods[index].inline_only = true;
            names.push(g.methods[index].name.clone());
            changed = true;
        }
    }
    Ok(())
}

/// The function of a component function that reads render locals.
pub fn inline_function(g: &Generator<'_>, n: &str) -> Option<ast::Function> {
    let index = scope::source_method(g, n.as_bytes())?;
    let method = &g.methods[index];
    if method.inline_only {
        method.func.as_deref().cloned()
    } else {
        None
    }
}

pub fn collect_method(g: &mut Generator<'_>, n: &[u8], f: &ast::Function) -> Result<()> {
    let name = String::from_utf8_lossy(n);
    let reserved = name == "render" || name == "build" || name == "self" || name == g.name;
    if scope::method_index(g, n).is_some() || scope::state_index(g, n).is_some() || reserved {
        return fail(
            g,
            f.loc,
            format!("duplicate or reserved method name `{name}`; rename it"),
        );
    }
    let ret = match f.ret.as_ref() {
        Some(reference) => types::type_ref(g, reference, true)?,
        None => Type::Void,
    };
    g.methods.push(crate::codegen::Method {
        name: name.into_owned(),
        func: Some(Box::new(f.clone())),
        ret,
        user: true,
        ..crate::codegen::Method::default()
    });
    Ok(())
}

pub fn collect_state(
    g: &mut Generator<'_>,
    loc: ast::Loc,
    d: &ast::Decl,
    call: &ast::Call,
) -> Result<()> {
    if !state_pattern(&d.pattern) {
        return fail(
            g,
            loc,
            "useState is destructured as const [value, setValue] = useState(...) (or const [value] = useState(...))",
        );
    }
    if !d.is_const {
        return fail(g, loc, "declare useState with const");
    }
    if call.args.len() != 1 || call.type_args.len() > 1 {
        return fail(
            g,
            loc,
            "useState takes one literal initializer and at most one type argument",
        );
    }
    let init = &call.args[0];
    // `useState(new AbortController())`: evaluated once, when the
    // component's state is created (React keeps the first value too)
    let aggregate_init = matches!(
        &init.data,
        ast::ExprData::Array(_)
            | ast::ExprData::Object(_)
            | ast::ExprData::NullLit
            | ast::ExprData::Ident(_)
    ) || matches!(&init.data, ast::ExprData::Call(call) if call.is_new);
    // `useState(Mode.Light)`: an enum member is a constant too
    let enum_init = matches!(&init.data, ast::ExprData::Member(member)
        if matches!(&member.object.data, ast::ExprData::Ident(name)
            if g.enums.iter().any(|decl| decl.name.name == *name)));
    if !types::is_literal(init) && !aggregate_init && !enum_init {
        return fail(
            g,
            init.loc,
            "useState initializer must be a literal, a constant or an enum member",
        );
    }
    let mut c = scope::Context {
        render: true,
        ret: Type::Node,
        constant: true,
        ..scope::Context::default()
    };
    let target = match call.type_args.first() {
        Some(reference) => Some(types::type_ref(g, reference, false)?),
        None => None,
    };
    let mut value = expr::emit(g, &mut c, init, target, false)?;
    let concrete = target.unwrap_or_else(|| value.ty.concrete());
    value = types::coerce(g, value, concrete, init.loc)?;
    let ast::Pattern::Array(names) = &d.pattern else {
        unreachable!("state pattern")
    };
    let n = String::from_utf8_lossy(&names[0].as_ref().expect("value name").name).into_owned();
    let setter = match names.get(1) {
        Some(name) => {
            String::from_utf8_lossy(&name.as_ref().expect("setter name").name).into_owned()
        }
        // never called: a name no source can spell
        None => format!("_{n}__setter"),
    };
    let value_conflict = scope::state_index(g, n.as_bytes()).is_some()
        || scope::setter_index(g, n.as_bytes()).is_some();
    let setter_conflict = scope::state_index(g, setter.as_bytes()).is_some()
        || scope::setter_index(g, setter.as_bytes()).is_some();
    if n == setter || value_conflict || setter_conflict {
        return fail(
            g,
            loc,
            format!("duplicate state or setter name; rename `{n}`"),
        );
    }
    g.states.push(crate::codegen::State {
        name: n,
        setter,
        ty: value.ty,
        initial: value.code,
        kind: crate::codegen::StateKind::State,
    });
    Ok(())
}

/// Collect top-level state and functions without adding derived render locals.
pub(super) fn collect_declaration(
    g: &mut Generator<'_>,
    loc: ast::Loc,
    d: &ast::Decl,
) -> Result<()> {
    let Some(init) = d.init.as_deref() else {
        return Ok(());
    };
    if crate::codegen::hook(g, init) {
        let ast::ExprData::Call(call) = &init.data else {
            unreachable!("hook call")
        };
        let ast::ExprData::Ident(name) = &call.callee.data else {
            unreachable!("hook callee")
        };
        let name = String::from_utf8_lossy(name);
        if name == "useRef" {
            return hooks::collect_ref(g, loc, d, call);
        }
        if name == "useMemo" || name == "useStore" {
            return Ok(()); // render locals (stmt.declaration)
        }
        if name == "useEffect" {
            return fail(g, loc, "useEffect returns nothing");
        }
        if (g.extensions.is_desktop_hook)(&name) {
            return Ok(()); // after `mark_inline`
        }
        return collect_state(g, loc, d, call);
    }
    if matches!(init.data, ast::ExprData::Arrow(_))
        && let ast::Pattern::Ident(name) = &d.pattern
    {
        let ast::ExprData::Arrow(function) = &init.data else {
            unreachable!("arrow")
        };
        // `(x: number) => x + 1` is a function value, not a handler
        let value_arrow = function.ret.is_none()
            && function.params.iter().all(|param| param.ty.is_some())
            && matches!(&function.body, ast::Body::Expr(body) if !matches!(
                body.data,
                ast::ExprData::Call(_) | ast::ExprData::Assign(_) | ast::ExprData::Update(_)
            ));
        if value_arrow {
            return Ok(());
        }
        return collect_method(g, &name.name, function);
    }
    Ok(())
}

/// Validate destructuring with short circuit checks before accessing its names.
pub(super) fn state_pattern(pattern: &ast::Pattern) -> bool {
    match pattern {
        // `const [value] = useState(…)` keeps a value without a setter
        ast::Pattern::Array(names) => {
            (names.len() == 1 || names.len() == 2) && names.iter().all(|name| name.is_some())
        }
        _ => false,
    }
}

/// Keep reserved import and receiver names out of the component declaration.
pub(super) fn reserved_component_name(name: &str) -> bool {
    matches!(name, "self" | "std" | "zant" | "ui" | "tsx")
}

/// Collect the render locals of a statement's declarations (`codegen.renderNames`).
pub(super) fn render_names(g: &mut Generator<'_>, s: &ast::Stmt) {
    match &s.data {
        ast::StmtData::Decl(d) => match &d.pattern {
            ast::Pattern::Ident(name) => {
                if d.init
                    .as_deref()
                    .is_none_or(|init| !matches!(init.data, ast::ExprData::Arrow(_)))
                {
                    g.render_names
                        .push(String::from_utf8_lossy(&name.name).into_owned());
                }
            }
            // `const [first, second] = pair` / `const { name } = user`: each
            // binding is a render local like `const name = user.name`. Hook
            // results (`[value, setValue] = useState(…)`, custom hooks) are
            // states, setters and methods, not render values.
            _ if d
                .init
                .as_deref()
                .and_then(hooks::call_name)
                .is_some_and(|name| name.starts_with("use")) => {}
            ast::Pattern::Array(names) => {
                for name in names.iter().flatten() {
                    g.render_names
                        .push(String::from_utf8_lossy(&name.name).into_owned());
                }
            }
            ast::Pattern::Object(bindings) => {
                for binding in bindings {
                    g.render_names
                        .push(String::from_utf8_lossy(&binding.local.name).into_owned());
                }
            }
        },
        ast::StmtData::Block(statements) => {
            for child in statements {
                render_names(g, child);
            }
        }
        ast::StmtData::If(v) => {
            render_names(g, &v.then);
            if let Some(otherwise) = v.else_.as_deref() {
                render_names(g, otherwise);
            }
        }
        _ => {}
    }
}
