//! `useEffect`: collected effects, their cleanups and whether a body changes state.

use super::*;

/// Registers the effect body as a method (before handlers are scanned, so its
/// captures and nested handlers resolve like an event arrow's).
pub fn collect_effect(g: &mut Generator<'_>, e: &ast::Expr) -> Result<()> {
    let ast::ExprData::Call(call) = &e.data else {
        return fail(
            g,
            e.loc,
            "useEffect takes a function and an optional dependency list",
        );
    };
    if call.args.is_empty() || call.args.len() > 2 {
        return fail(
            g,
            e.loc,
            "useEffect takes a function and an optional dependency list",
        );
    }
    let ast::ExprData::Arrow(function) = &call.args[0].data else {
        return fail(
            g,
            e.loc,
            "useEffect takes a function and an optional dependency list",
        );
    };
    if !function.params.is_empty() {
        return fail(g, function.loc, "the effect function takes no parameters");
    }
    let cleanups = g.cleanups.len();
    if let ast::Body::Block(statements) = &function.body {
        for statement in statements {
            collect_cleanups(g, statement)?;
        }
    }
    g.counter += 1;
    let method = g.methods.len();
    let cleanup = g.cleanups.len() > cleanups;
    g.effects.push(Effect {
        call: call.clone(),
        method,
        cleanup,
        consumed: false,
    });
    let name = scope::unique_method(g, &format!("_effect{}", g.counter))?;
    let capture_name = scope::unique_method(g, &format!("_EffectCapture{}", g.counter))?;
    g.methods.push(crate::codegen::Method {
        name,
        func: Some(function.clone()),
        action: Some(tags::Kind::Action),
        capture_name: Some(capture_name),
        ..crate::codegen::Method::default()
    });
    Ok(())
}

/// `return () => { … }` in an effect body (also inside `if`s).
pub(super) fn collect_cleanups(g: &mut Generator<'_>, s: &ast::Stmt) -> Result<()> {
    match &s.data {
        ast::StmtData::For(v) => collect_cleanups(g, &v.body),
        ast::StmtData::ForOf(v) => collect_cleanups(g, &v.body),
        ast::StmtData::While(v) => collect_cleanups(g, &v.body),
        ast::StmtData::Switch(v) => {
            for case in &v.cases {
                for child in &case.body {
                    collect_cleanups(g, child)?;
                }
            }
            Ok(())
        }
        ast::StmtData::Ret(Some(value)) => {
            let ast::ExprData::Arrow(function) = &value.data else {
                return Ok(());
            };
            if !function.params.is_empty() {
                return fail(g, function.loc, "the cleanup function takes no parameters");
            }
            g.counter += 1;
            let method = g.methods.len();
            g.cleanups.push(Cleanup {
                arrow: (**value).clone(),
                method,
            });
            let name = scope::unique_method(g, &format!("_cleanup{}", g.counter))?;
            let capture_name = scope::unique_method(g, &format!("_CleanupCapture{}", g.counter))?;
            g.methods.push(crate::codegen::Method {
                name,
                func: Some(function.clone()),
                action: Some(tags::Kind::Action),
                cleanup: true,
                capture_name: Some(capture_name),
                ..crate::codegen::Method::default()
            });
            Ok(())
        }
        ast::StmtData::Ret(None) => Ok(()),
        ast::StmtData::If(v) => {
            collect_cleanups(g, &v.then)?;
            if let Some(otherwise) = v.else_.as_deref() {
                collect_cleanups(g, otherwise)?;
            }
            Ok(())
        }
        ast::StmtData::Try(t) => {
            for child in t
                .body
                .iter()
                .chain(t.catch.iter().flat_map(|c| c.body.iter()))
                .chain(t.finally.iter().flatten())
            {
                collect_cleanups(g, child)?;
            }
            Ok(())
        }
        ast::StmtData::Throw(_) => Ok(()),
        ast::StmtData::Block(statements) => {
            for child in statements {
                collect_cleanups(g, child)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// `return () => …` inside an effect: keep the cleanup in the effect's field;
/// the next dependency change queues it before the body runs again.
pub fn cleanup_return(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    arrow: &ast::Expr,
) -> Result<String> {
    let index = g
        .cleanups
        .iter()
        .position(|item| item.arrow == *arrow)
        .unwrap_or(0);
    let method_index = g.cleanups[index].method;
    captures::prepare(g, c, method_index)?;
    let method = g.cleanups[index].method;
    let field = c.effect_field.clone().unwrap_or_default();
    let action = crate::codegen::generate::action_value(g, c, method, Some(tags::Kind::Action))?;
    Ok(format!("self.{field}.set({action});\nreturn;\n"))
}

/// The statement queuing the effect when its dependency list changed.
pub fn effect(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    top: bool,
) -> Result<String> {
    if !c.render || !top {
        return fail(
            g,
            e.loc,
            "hooks must be called at the top level of the component",
        );
    }
    let ast::ExprData::Call(call) = &e.data else {
        unreachable!("effect call")
    };
    let Some(index) = g
        .effects
        .iter()
        .position(|item| item.call == *call && !item.consumed)
    else {
        return fail(
            g,
            e.loc,
            "the effect was not collected; use useEffect(...) in the render body",
        );
    };
    g.effects[index].consumed = true;
    let info = g.effects[index].clone();
    let deps = match call.args.get(1) {
        Some(deps) => Some(dep_list(g, c, deps)?),
        None => None,
    };
    let mut cleanup_field: Option<String> = None;
    if info.cleanup {
        let deps_ty = match &deps {
            Some(deps) => deps.ty.clone(),
            None => "()".to_owned(),
        };
        let field = add_field(g, "_effect", "tsx::Cleanup".to_owned())?;
        let _ = deps_ty;
        cleanup_field = Some(scope::state_field(g, field.as_bytes())?);
    }
    // the effect's cleanup field must be known while its body is emitted
    // (`return () => …` stores the cleanup there), before `captures::prepare`
    // emits and caches the method
    if let Some(field) = &cleanup_field {
        g.methods[info.method].effect_field = Some(field.clone());
    }
    captures::prepare(g, c, info.method)?;
    if changes_state(
        g,
        g.methods[info.method].func.as_ref().map(|f| f.body.clone()),
    ) {
        // ask for another build first (an early `return` can't skip it)
        if let Some(emitted) = g.methods[info.method].emitted.clone()
            && let Some(open) = emitted.find("{\n")
        {
            g.methods[info.method].emitted = Some(format!(
                "{}ui::request_rebuild();\n{}",
                &emitted[..open + 2],
                &emitted[open + 2..]
            ));
        }
    }
    let action =
        crate::codegen::generate::action_value(g, c, info.method, Some(tags::Kind::Action))?;
    let cleanup_field = cleanup_field.clone();
    match (&deps, &cleanup_field) {
        (Some(deps), Some(cleanup)) => {
            let field = add_field(g, "_deps", format!("Option<{}>", deps.ty))?;
            let slot = scope::state_field(g, field.as_bytes())?;
            Ok(format!(
                "let _deps: {ty} = {value};\n\
                 if self.{slot}.as_ref() != Some(&_deps) {{\n\
                 self.{slot} = Some(_deps);\n\
                 if let Some(_cleanup) = self.{cleanup}.take() {{\n\
                 ui::effect(_cleanup);\n\
                 }}\n\
                 ui::effect({action});\n\
                 }}",
                ty = deps.ty,
                value = deps.value,
            ))
        }
        (Some(deps), None) => {
            let field = add_field(g, "_deps", format!("Option<{}>", deps.ty))?;
            let slot = scope::state_field(g, field.as_bytes())?;
            Ok(format!(
                "let _deps: {ty} = {value};\n\
                 if self.{slot}.as_ref() != Some(&_deps) {{\n\
                 self.{slot} = Some(_deps);\n\
                 ui::effect({action});\n\
                 }}",
                ty = deps.ty,
                value = deps.value,
            ))
        }
        (None, _) => Ok(format!("ui::effect({action});")),
    }
}

/// Whether an effect body may change state: it calls a setter, a component
/// function, a callback prop or an imported function (ref writes and array
/// methods don't count).
pub(super) fn changes_state(g: &Generator<'_>, body: Option<ast::Body>) -> bool {
    let Some(body) = body else {
        return false;
    };
    match body {
        ast::Body::Expr(e) => expr_changes(g, &e),
        ast::Body::Block(statements) => statements.iter().any(|s| stmt_changes(g, s)),
    }
}

pub(super) fn stmt_changes(g: &Generator<'_>, s: &ast::Stmt) -> bool {
    match &s.data {
        ast::StmtData::For(v) => {
            v.init.as_deref().is_some_and(|init| stmt_changes(g, init))
                || v.cond.as_deref().is_some_and(|cond| expr_changes(g, cond))
                || v.update
                    .as_deref()
                    .is_some_and(|update| expr_changes(g, update))
                || stmt_changes(g, &v.body)
        }
        ast::StmtData::ForOf(v) => expr_changes(g, &v.iterable) || stmt_changes(g, &v.body),
        ast::StmtData::While(v) => expr_changes(g, &v.cond) || stmt_changes(g, &v.body),
        ast::StmtData::Switch(v) => {
            if expr_changes(g, &v.value) {
                return true;
            }
            v.cases.iter().any(|case| {
                case.value
                    .as_deref()
                    .is_some_and(|value| expr_changes(g, value))
                    || case.body.iter().any(|child| stmt_changes(g, child))
            })
        }
        ast::StmtData::Break | ast::StmtData::Continue => false,
        ast::StmtData::Decl(d) => d.init.as_deref().is_some_and(|e| expr_changes(g, e)),
        ast::StmtData::Func(_) => false,
        ast::StmtData::Ret(e) => e.as_deref().is_some_and(|v| expr_changes(g, v)),
        ast::StmtData::If(v) => {
            expr_changes(g, &v.cond)
                || stmt_changes(g, &v.then)
                || v.else_.as_deref().is_some_and(|el| stmt_changes(g, el))
        }
        ast::StmtData::Expr(e) => expr_changes(g, e),
        ast::StmtData::Block(statements) => statements.iter().any(|child| stmt_changes(g, child)),
        ast::StmtData::Throw(e) => expr_changes(g, e),
        ast::StmtData::Try(t) => t
            .body
            .iter()
            .chain(t.catch.iter().flat_map(|c| c.body.iter()))
            .chain(t.finally.iter().flatten())
            .any(|child| stmt_changes(g, child)),
    }
}

pub(super) fn expr_changes(g: &Generator<'_>, e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::Call(call) => {
            match &call.callee.data {
                // console.log and array methods are pure
                ast::ExprData::Member(m) => {
                    if let ast::ExprData::Ident(name) = &m.object.data
                        && name.as_slice() != b"console"
                    {
                        let props = g
                            .components
                            .get(g.current_component.unwrap_or(usize::MAX))
                            .map(|comp| comp.param.clone())
                            .unwrap_or_default();
                        if !props.is_empty() && name.as_slice() == props.as_bytes() {
                            return true;
                        }
                    }
                }
                ast::ExprData::Ident(_) => return true,
                _ => {}
            }
            if expr_changes(g, &call.callee) {
                return true;
            }
            call.args.iter().any(|arg| expr_changes(g, arg))
        }
        ast::ExprData::Template(t) => t.exprs.iter().any(|x| expr_changes(g, x)),
        ast::ExprData::Unary(u) => expr_changes(g, &u.operand),
        ast::ExprData::Binary(b) => expr_changes(g, &b.lhs) || expr_changes(g, &b.rhs),
        ast::ExprData::Cond(c) => {
            expr_changes(g, &c.cond) || expr_changes(g, &c.then) || expr_changes(g, &c.else_)
        }
        ast::ExprData::Member(m) => expr_changes(g, &m.object),
        ast::ExprData::Arrow(f) => changes_state(g, Some(f.body.clone())),
        ast::ExprData::Object(properties) => properties.iter().any(|p| expr_changes(g, &p.value)),
        ast::ExprData::Array(items) => items.iter().any(|x| expr_changes(g, x)),
        ast::ExprData::Spread(inner) | ast::ExprData::NonNull(inner) => expr_changes(g, inner),
        ast::ExprData::Index(i) => expr_changes(g, &i.object) || expr_changes(g, &i.index),
        ast::ExprData::As(a) => expr_changes(g, &a.expr),
        ast::ExprData::Assign(a) => expr_changes(g, &a.value),
        ast::ExprData::Update(_)
        | ast::ExprData::Number(_)
        | ast::ExprData::String(_)
        | ast::ExprData::Boolean(_)
        | ast::ExprData::NullLit
        | ast::ExprData::Ident(_)
        | ast::ExprData::Jsx(_) => false,
    }
}
