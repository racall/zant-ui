//! `useRef`, `useMemo` and `useEffect` (`docs/tsx.md`, "第三阶段").
//! They compile to plain state fields:
//!
//! - `const r = useRef(0)`: a field read and written directly (`self.r`); a
//!   write reads the live value, no setter and no handler snapshot.
//! - `const v = useMemo(() => f(a), [a, b])`: an `Option<(deps, value)>` field;
//!   the body runs only when the dependency tuple changed.
//! - `useEffect(() => { … }, [a])`: a method queued with `ui::effect` when the
//!   tuple changed (`[]`: once on mount; no list: after every build). A
//!   `return () => …` cleanup keeps its `ui::Action` in a second field and is
//!   queued before the body re-runs (`ui::effect` in order, since Rust has no
//!   `tsx.Effect`).

use crate::ast;
use crate::tags;

use super::aggregate;
use super::callbacks;
use super::captures;
use super::expr;
use super::model;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

/// A hidden field of the component (memo cache, effect dependencies).
#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub rust_type: String,
}

#[derive(Clone, Debug)]
pub struct Effect {
    /// The `useEffect(…)` call expression (`ast::Call` equality is structural:
    /// the Rust AST has no stable node ids).
    pub call: ast::Call,
    pub method: usize,
    /// Has `return () => …` (a cleanup `ui::Action` field).
    pub cleanup: bool,
    pub consumed: bool,
}

/// A cleanup arrow and its method.
#[derive(Clone, Debug)]
pub struct Cleanup {
    pub arrow: ast::Expr,
    pub method: usize,
}

/// `useRef` / `useMemo` / `useEffect` (never renamed).
pub fn extra_hook(name: &str) -> bool {
    name == "useRef" || name == "useMemo" || name == "useEffect"
}

/// The called name of a hook expression (`useEffect` of `useEffect(…)`).
pub fn call_name(e: &ast::Expr) -> Option<String> {
    let ast::ExprData::Call(call) = &e.data else {
        return None;
    };
    let ast::ExprData::Ident(name) = &call.callee.data else {
        return None;
    };
    Some(String::from_utf8_lossy(name).into_owned())
}

// ── useRef ────────────────────────────────────────────────────────────

/// `const r = useRef<T>(initial)`: a state entry of kind `.ref`.
pub fn collect_ref(
    g: &mut Generator<'_>,
    loc: ast::Loc,
    d: &ast::Decl,
    call: &ast::Call,
) -> Result<()> {
    let ast::Pattern::Ident(pattern) = &d.pattern else {
        return fail(g, loc, "useRef returns one object: const ref = useRef(…)");
    };
    if !d.is_const {
        return fail(g, loc, "declare useRef with const");
    }
    if call.args.len() != 1 || call.type_args.len() > 1 {
        return fail(
            g,
            loc,
            "useRef takes one literal initial value and at most one type argument",
        );
    }
    let init = &call.args[0];
    let literal = types::is_literal(init)
        || matches!(
            init.data,
            ast::ExprData::Array(_) | ast::ExprData::Object(_) | ast::ExprData::NullLit
        );
    if !literal {
        return fail(g, init.loc, "useRef initial value must be a literal");
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
    let name = String::from_utf8_lossy(&pattern.name).into_owned();
    if scope::state_index(g, name.as_bytes()).is_some()
        || scope::setter_index(g, name.as_bytes()).is_some()
    {
        return fail(
            g,
            loc,
            format!("duplicate state or ref name; rename `{name}`"),
        );
    }
    g.states.push(crate::codegen::State {
        name,
        setter: String::new(),
        ty: value.ty,
        initial: value.code,
        kind: crate::codegen::StateKind::Ref,
    });
    Ok(())
}

fn ref_index(g: &Generator<'_>, c: &scope::Context, e: &ast::Expr) -> Option<usize> {
    let ast::ExprData::Ident(name) = &e.data else {
        return None;
    };
    if scope::local(c, name).is_some() {
        return None;
    }
    let index = scope::state_index(g, name)?;
    if g.states[index].kind == crate::codegen::StateKind::Ref {
        Some(index)
    } else {
        None
    }
}

/// `ref.current` (live value, in render and in handlers), or null when `m` is
/// not a ref access.
pub fn ref_read(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    m: &ast::Member,
) -> Result<Option<Value>> {
    let Some(index) = ref_index(g, c, &m.object) else {
        return Ok(None);
    };
    if m.name.name != b"current" {
        return fail(g, m.name.loc, "a ref only has `.current`");
    }
    let state = g.states[index].clone();
    let field = scope::state_field(g, state.name.as_bytes())?;
    // non-`Copy` values are cloned at every read (a read never moves out
    // of the state field)
    Ok(Some(Value {
        ty: state.ty,
        code: format!("self.{field}.clone()"),
        ..Value::default()
    }))
}

/// `ref.current = v`, `ref.current += v`, `ref.current++` in handlers and
/// effects; null when the target is not a ref.
pub fn ref_write(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
) -> Result<Option<String>> {
    let target = match &e.data {
        ast::ExprData::Assign(assign) => &assign.target,
        ast::ExprData::Update(update) => &update.target,
        _ => return Ok(None),
    };
    let ast::ExprData::Member(m) = &target.data else {
        return Ok(None);
    };
    let Some(index) = ref_index(g, c, &m.object) else {
        return Ok(None);
    };
    if m.name.name != b"current" {
        return fail(g, m.name.loc, "a ref only has `.current`");
    }
    if c.render {
        return fail(
            g,
            e.loc,
            "refs are written in event handlers and effects, not while rendering",
        );
    }
    let state = g.states[index].clone();
    let field = scope::state_field(g, state.name.as_bytes())?;
    if let ast::ExprData::Update(update) = &e.data {
        if !state.ty.numeric() {
            return fail(g, e.loc, "++ and -- require a number");
        }
        let op = if update.op == b"++" { "+=" } else { "-=" };
        let one = if state.ty.float() { "1.0" } else { "1" };
        return Ok(Some(format!("self.{field} {op} {one};\n")));
    }
    let ast::ExprData::Assign(assign) = &e.data else {
        unreachable!("assignment")
    };
    let value = if assign.op == b"=" {
        expr::emit(g, c, &assign.value, Some(state.ty), false)?
    } else {
        let binary = ast::Binary {
            op: assign.op[..assign.op.len().saturating_sub(1)].to_vec(),
            lhs: target.clone(),
            rhs: assign.value.clone(),
        };
        let value = expr::binary(g, c, e.loc, &binary, Some(state.ty), false)?;
        types::coerce(g, value, state.ty, e.loc)?
    };
    Ok(Some(format!("self.{field} = {};\n", value.code)))
}

/// A bare `ref` used as a value.
pub fn bare_ref(g: &mut Generator<'_>, index: usize, loc: ast::Loc) -> Result<Value> {
    fail(
        g,
        loc,
        format!(
            "read and write a ref through `{}.current`",
            g.states[index].name
        ),
    )
}

// ── dependency lists ──────────────────────────────────────────────────

/// `[a, b]` as the Rust tuple `(a, b)` and its type `(A, B)`.
#[derive(Clone, Debug)]
pub struct DepList {
    pub value: String,
    pub ty: String,
}

pub fn dep_list(g: &mut Generator<'_>, c: &mut scope::Context, e: &ast::Expr) -> Result<DepList> {
    let ast::ExprData::Array(items) = &e.data else {
        return fail(
            g,
            e.loc,
            "the dependency list must be an array literal: [a, b]",
        );
    };
    let mut values: Vec<String> = Vec::new();
    let mut tys: Vec<String> = Vec::new();
    for item in items {
        if matches!(item.data, ast::ExprData::Spread(_)) {
            return fail(g, item.loc, "list each dependency");
        }
        let mut value = expr::emit(g, c, item, None, false)?;
        let concrete = value.ty.concrete();
        value = types::coerce(g, value, concrete, item.loc)?;
        if value.ty == Type::Node
            || value.ty == Type::OptionalNode
            || value.ty == Type::Void
            || value.ty == Type::Null
        {
            return fail(
                g,
                item.loc,
                "dependencies are values (numbers, strings, arrays, objects)",
            );
        }
        if let Some(entry) = model::info(g, value.ty)
            && entry.kind == model::InfoKind::Callback
        {
            return fail(g, item.loc, "functions can't be dependencies");
        }
        values.push(value.code);
        tys.push(value.ty.rust(g).to_owned());
    }
    let tuple = format!("({})", values.join(", "));
    let tuple = if values.len() == 1 {
        format!("({},)", values[0])
    } else {
        tuple
    };
    Ok(DepList {
        value: tuple,
        ty: if tys.is_empty() {
            "()".to_owned()
        } else if tys.len() == 1 {
            format!("({},)", tys[0])
        } else {
            format!("({})", tys.join(", "))
        },
    })
}

fn add_field(g: &mut Generator<'_>, base: &str, rust_type: String) -> Result<String> {
    g.counter += 1;
    let name = scope::unique_method(g, &format!("{base}{}", g.counter))?;
    g.hook_fields.push(Field {
        name: name.clone(),
        rust_type,
    });
    Ok(name)
}

// ── useMemo ───────────────────────────────────────────────────────────

/// `const v = useMemo(() => …, [deps])` as a render local.
pub fn memo(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    d: &ast::Decl,
    top: bool,
) -> Result<String> {
    let Some(init) = d.init.as_deref() else {
        return fail(g, loc, "useMemo needs an initializer");
    };
    let ast::ExprData::Call(call) = &init.data else {
        unreachable!("hook call")
    };
    if !c.render || !top {
        return fail(
            g,
            loc,
            "hooks must be called at the top level of the component",
        );
    }
    let ast::Pattern::Ident(pattern) = &d.pattern else {
        return fail(g, loc, "const value = useMemo(() => …, [deps])");
    };
    let arrow = match call.args.first().map(|arg| &arg.data) {
        Some(ast::ExprData::Arrow(function)) if function.params.is_empty() => function,
        _ => {
            return fail(
                g,
                loc,
                "useMemo takes a function without parameters and a dependency list",
            );
        }
    };
    if call.args.len() != 2 {
        return fail(
            g,
            loc,
            "useMemo takes a function without parameters and a dependency list",
        );
    }
    if call.type_args.len() > 1 {
        return fail(g, loc, "useMemo takes at most one type argument");
    }
    let annotation: Option<Type> = match call.type_args.first() {
        Some(reference) => Some(types::type_ref(g, reference, false)?),
        None => match d.ty.as_ref() {
            Some(reference) => Some(types::type_ref(g, reference, false)?),
            None => None,
        },
    };
    let deps = dep_list(g, c, &call.args[1])?;
    let callback = ast::Expr {
        loc: call.args[0].loc,
        data: ast::ExprData::Arrow(arrow.clone()),
    };
    let computed = callbacks::inline(g, c, &callback, annotation)?;
    let ty = computed.ty;
    if ty == Type::Node || ty == Type::OptionalNode || ty == Type::Void {
        return fail(
            g,
            loc,
            "useMemo caches values; elements are rebuilt every frame",
        );
    }
    let deps_ty = deps.ty.clone();
    let field = add_field(g, "_memo", format!("Option<({deps_ty}, {})>", ty.rust(g)))?;
    let slot = scope::state_field(g, field.as_bytes())?;
    let binding = scope::bind(g, c, &pattern.name, ty, false)?;
    let rust_ty = ty.rust(g).to_owned();
    let init = format!(
        "'{label}: {{\n\
         let _deps: {deps_ty} = {deps_value};\n\
         if self.{slot}.as_ref().is_none_or(|(deps, _)| deps != &_deps) {{\n\
         let _value: {rust_ty} = {computed};\n\
         self.{slot} = Some((_deps, _value));\n\
         }}\n\
         self.{slot}.as_ref().expect(\"memo\").1.clone()\n\
         }}",
        label = aggregate::temporary(g, c, b"_memo")?,
        deps_value = deps.value,
        computed = computed.code,
    );
    c.bindings[binding].init = Some(init);
    Ok(format!(
        "{}{}{}",
        scope::PLACEHOLDER_START,
        binding,
        scope::PLACEHOLDER_END
    ))
}

// ── useEffect ─────────────────────────────────────────────────────────

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
fn collect_cleanups(g: &mut Generator<'_>, s: &ast::Stmt) -> Result<()> {
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
    let action = super::generate::action_value(g, c, method, Some(tags::Kind::Action))?;
    Ok(format!("self.{field} = Some({action});\nreturn;\n"))
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
        let field = add_field(g, "_effect", format!("Option<{}>", "ui::Action"))?;
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
    let action = super::generate::action_value(g, c, info.method, Some(tags::Kind::Action))?;
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
fn changes_state(g: &Generator<'_>, body: Option<ast::Body>) -> bool {
    let Some(body) = body else {
        return false;
    };
    match body {
        ast::Body::Expr(e) => expr_changes(g, &e),
        ast::Body::Block(statements) => statements.iter().any(|s| stmt_changes(g, s)),
    }
}

fn stmt_changes(g: &Generator<'_>, s: &ast::Stmt) -> bool {
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
    }
}

fn expr_changes(g: &Generator<'_>, e: &ast::Expr) -> bool {
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

/// The Rust field name of a hidden hook field (`_memo1`).
pub fn field_name(g: &mut Generator<'_>, field: &Field) -> Result<String> {
    scope::state_field(g, field.name.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_hooks_are_use_ref_use_memo_and_use_effect() {
        assert!(extra_hook("useRef"));
        assert!(extra_hook("useMemo"));
        assert!(extra_hook("useEffect"));
        assert!(!extra_hook("useState"));
        assert!(!extra_hook("useStore"));
    }

    #[test]
    fn a_non_dependency_fails() {
        let mut parser = crate::parser::Parser::init(
            "function App() { const a = 1; useEffect(() => {}, [<Text>a</Text>]); return <Text>a</Text>; }\n",
        );
        let module = parser.parse_module().expect("test source parses");
        let mut diag = None;
        let mut g = Generator::new(&mut diag);
        let mut c = scope::Context {
            render: true,
            ..scope::Context::default()
        };
        let function = &module.functions[0].func;
        let statements = match &function.body {
            ast::Body::Block(statements) => statements,
            ast::Body::Expr(_) => panic!("block body"),
        };
        let decl = match &statements[1].data {
            ast::StmtData::Expr(call) => match &call.data {
                ast::ExprData::Call(call) => call.args[1].clone(),
                _ => panic!("effect call"),
            },
            _ => panic!("effect statement"),
        };
        assert!(dep_list(&mut g, &mut c, &decl).is_err());
        assert!(g.diag.is_some());
    }
}
