//! Render and handler statements, local mutation, setters, and functional
//! updates.
//!
//! Declarations are emitted as placeholders and expanded by
//! [`scope::finish`], which picks `let` or `let mut`.

use crate::ast;

use super::expr;
use super::loops;
use super::model;
use super::narrowing;
use super::rust_text;
use super::scope;
use super::types::{self, Type};
use super::{Generator, Result, fail};

pub fn statements(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    ss: &[Box<ast::Stmt>],
    top: bool,
) -> Result<String> {
    let mut out = String::new();
    let mark = c.narrowed.len();
    for s in ss {
        out.push_str(&statement(g, c, s, top)?);
        if loops::exits(s) {
            break;
        }
        // `if (!entry) return …;` narrows `entry` for the rest of the block
        if let ast::StmtData::If(v) = &s.data
            && v.else_.is_none()
            && leaves(&v.then)
        {
            narrowing::push(g, c, &v.cond, false)?;
        }
    }
    narrowing::restore(c, mark);
    Ok(out)
}

/// The statement always leaves the block (return, break, continue).
fn leaves(s: &ast::Stmt) -> bool {
    let body: Vec<Box<ast::Stmt>> = match &s.data {
        ast::StmtData::Block(ss) => ss.clone(),
        _ => vec![Box::new(s.clone())],
    };
    if returns_all(&body) {
        return true;
    }
    body.last()
        .is_some_and(|last| matches!(last.data, ast::StmtData::Break | ast::StmtData::Continue))
}

pub fn scoped(g: &mut Generator<'_>, c: &mut scope::Context, s: &ast::Stmt) -> Result<String> {
    let depth = c.active.len();
    let inner = match &s.data {
        ast::StmtData::Block(ss) => statements(g, c, ss, false)?,
        _ => statement(g, c, s, false)?,
    };
    c.active.truncate(depth);
    Ok(format!("{{\n{inner}}}"))
}

pub fn statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    s: &ast::Stmt,
    top: bool,
) -> Result<String> {
    match &s.data {
        ast::StmtData::For(_)
        | ast::StmtData::ForOf(_)
        | ast::StmtData::While(_)
        | ast::StmtData::Switch(_)
        | ast::StmtData::Break
        | ast::StmtData::Continue => loops::emit(g, c, s),
        ast::StmtData::Decl(d) => declaration(g, c, s.loc, d, top),
        ast::StmtData::Func(function) => {
            if c.render && top {
                return Ok(String::new());
            }
            if !c.render && super::local_functions::registered(g, function) {
                return super::local_functions::declare(g, c, s.loc, &function.name, function);
            }
            fail(
                g,
                s.loc,
                "nested functions are not supported yet (closures arrive in phase 2)",
            )
        }
        ast::StmtData::Ret(e) => return_statement(g, c, s.loc, e.as_deref()),
        ast::StmtData::Expr(e) => {
            if super::hook(g, e) {
                if let Some(hook) = g.extensions.hooks.hook_statement {
                    return hook(g, c, e, top);
                }
                return fail(g, e.loc, "hook statements need the hook support installed");
            }
            expression_statement(g, c, e)
        }
        ast::StmtData::Block(ss) => block(g, c, ss),
        ast::StmtData::If(v) => if_statement(g, c, v),
        ast::StmtData::Throw(value) => super::exceptions::throw_statement(g, c, s.loc, value),
        ast::StmtData::Try(t) => super::exceptions::try_statement(g, c, s.loc, t),
    }
}

/// Emit a typed local or skip a registered top-level hook or component
/// function.
fn declaration(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    d: &ast::Decl,
    top: bool,
) -> Result<String> {
    let Some(e) = d.init.as_deref() else {
        return fail(g, loc, "local declarations need an initializer");
    };
    if super::hook(g, e) {
        if let ast::ExprData::Call(call) = &e.data
            && let ast::ExprData::Ident(name) = &call.callee.data
            && (name.as_slice() == b"useMemo" || name.as_slice() == b"useStore")
        {
            return match g.extensions.hooks.hook_declaration {
                Some(hook) => hook(g, c, loc, d, top),
                None => fail(
                    g,
                    loc,
                    "`useMemo` / `useStore` need the hook support installed",
                ),
            };
        }
        if let ast::ExprData::Call(call) = &e.data
            && let ast::ExprData::Ident(name) = &call.callee.data
            && (g.extensions.is_desktop_hook)(&String::from_utf8_lossy(name))
        {
            return match g.extensions.hooks.hook_declaration {
                Some(hook) => hook(g, c, loc, d, top),
                None => fail(g, loc, "desktop hooks need the application hooks installed"),
            };
        }
        if c.render && top {
            return Ok(String::new());
        }
        return fail(
            g,
            e.loc,
            "hooks must be called at the top level of the component",
        );
    }
    if let ast::Pattern::Object(bindings) = &d.pattern {
        return object_declaration(g, c, d, e, bindings);
    }
    if let ast::Pattern::Array(names) = &d.pattern {
        return tuple_declaration(g, c, loc, d, e, names);
    }
    let ast::Pattern::Ident(pattern) = &d.pattern else {
        return fail(g, loc, "destructuring is only supported for useState");
    };
    if let ast::ExprData::Arrow(arrow) = &e.data {
        // at a component's top level a `void` arrow is a handler (a method);
        // one with typed parameters that returns a value is a function value
        let value_type = if c.render && top && d.ty.is_none() {
            let mark = g.diag.is_none();
            match super::closures::arrow_type(g, c, arrow) {
                Ok(ty) => ty,
                Err(_) if mark => {
                    *g.diag = None;
                    None
                }
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        if c.render && top && value_type.is_none() {
            return Ok(String::new());
        }
        // `const f = (x: number): number => …`: a function value
        let annotated = match (d.ty.as_ref(), value_type) {
            (Some(t), _) => Some(types::type_ref(g, t, false)?),
            (None, Some(ty)) => Some(ty),
            (None, None) => super::closures::arrow_type(g, c, arrow)?,
        };
        let Some(ty) = annotated.filter(|ty| super::closures::signature(g, *ty).is_some()) else {
            // a procedure (it sets state, it is async): a local function
            if !c.render && super::local_functions::registered(g, arrow) {
                return super::local_functions::declare(g, c, e.loc, &pattern.name, arrow);
            }
            return fail(
                g,
                e.loc,
                "a local function needs typed parameters and a return value (`(x: number): number => …`)",
            );
        };
        let value = super::closures::arrow(g, c, e.loc, arrow, ty)?;
        let binding = scope::bind(g, c, &pattern.name, ty, !d.is_const)?;
        c.bindings[binding].init = Some(value.code);
        c.bindings[binding].fresh = true;
        return Ok(format!(
            "{}{}{}",
            scope::PLACEHOLDER_START,
            binding,
            scope::PLACEHOLDER_END
        ));
    }
    let target = match d.ty.as_ref() {
        Some(t) => Some(types::type_ref(g, t, false)?),
        None => None,
    };
    let mut v = expr::emit(g, c, e, target, false)?;
    let concrete_ty = target.unwrap_or_else(|| v.ty.concrete());
    v = types::coerce(g, v, concrete_ty, e.loc)?;
    if v.ty == Type::Void || v.ty == Type::Null {
        return fail(g, e.loc, "local must have a value with a concrete type");
    }
    let binding = scope::bind(g, c, &pattern.name, v.ty, !d.is_const)?;
    c.bindings[binding].init = Some(v.code);
    c.bindings[binding].fresh = super::place::fresh(e);
    Ok(format!(
        "{}{}{}",
        scope::PLACEHOLDER_START,
        binding,
        scope::PLACEHOLDER_END
    ))
}

/// `const [a, b] = pair`: a tuple's elements, from one evaluation.
fn tuple_declaration(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    d: &ast::Decl,
    e: &ast::Expr,
    names: &[Option<ast::Name>],
) -> Result<String> {
    let target =
        d.ty.as_ref()
            .map(|ty| types::type_ref(g, ty, false))
            .transpose()?;
    let mut value = expr::emit(g, c, e, target, false)?;
    if let Some(target) = target {
        value = types::coerce(g, value, target, e.loc)?;
    }
    let Some(items) = super::model::tuple_items(g, value.ty) else {
        return fail(
            g,
            loc,
            "array destructuring needs a tuple (`[string, number]`); index arrays instead (`items[0]`)",
        );
    };
    if names.len() > items.len() {
        return fail(g, loc, format!("this tuple has {} elements", items.len()));
    }
    let whole = scope::bind(g, c, b"_tuple", value.ty, false)?;
    c.bindings[whole].init = Some(value.code);
    let whole_rust = c.bindings[whole].rust.clone();
    let mut out = format!(
        "{}{whole}{}",
        scope::PLACEHOLDER_START,
        scope::PLACEHOLDER_END
    );
    for (position, name) in names.iter().enumerate() {
        let Some(name) = name else { continue };
        let ty = items[position];
        let read = if ty.copy(g) {
            format!("{whole_rust}.{position}")
        } else {
            format!("{whole_rust}.{position}.clone()")
        };
        let binding = scope::bind(g, c, &name.name, ty, !d.is_const)?;
        c.bindings[binding].init = Some(read);
        out.push_str(&format!(
            "{}{binding}{}",
            scope::PLACEHOLDER_START,
            scope::PLACEHOLDER_END
        ));
    }
    Ok(out)
}

/// Read a typed object's fields from one evaluation of the initializer.
fn object_declaration(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    d: &ast::Decl,
    e: &ast::Expr,
    bindings: &[ast::ObjectBinding],
) -> Result<String> {
    let target =
        d.ty.as_ref()
            .map(|ty| types::type_ref(g, ty, false))
            .transpose()?;
    let mut value = expr::emit(g, c, e, target, false)?;
    if let Some(target) = target {
        value = types::coerce(g, value, target, e.loc)?;
    }
    if model::info(g, value.ty).is_none_or(|info| info.kind != model::InfoKind::Object) {
        return fail(
            g,
            e.loc,
            "object destructuring needs an object with a known type",
        );
    }
    let source = scope::bind(g, c, b"_destructure", value.ty, false)?;
    c.bindings[source].init = Some(value.code);
    // This compiler-generated temporary is not a TSX local.
    c.active.pop();
    let mut out = format!(
        "{}{source}{}",
        scope::PLACEHOLDER_START,
        scope::PLACEHOLDER_END
    );
    for binding in bindings {
        let ty = model::field_type(g, value.ty, &binding.key.name, binding.key.loc)?;
        let field = match model::field_rust(g, value.ty, &binding.key.name) {
            Some(field) => field,
            None => rust_text::id_at(g, &binding.key.name, binding.key.loc)?,
        };
        let code = format!("{}.{}", c.bindings[source].rust, field);
        let read = scope::owned_read(g, ty, code);
        let local = scope::bind(g, c, &binding.local.name, ty, !d.is_const)?;
        c.bindings[local].init = Some(read.code);
        out.push_str(&format!(
            "{}{local}{}",
            scope::PLACEHOLDER_START,
            scope::PLACEHOLDER_END
        ));
    }
    Ok(out)
}

/// Validate the return value against the surrounding render or handler
/// result type.
fn return_statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    e: Option<&ast::Expr>,
) -> Result<String> {
    if let Some(x) = e
        && matches!(x.data, ast::ExprData::Arrow(_))
        && c.effect_field.is_some()
    {
        return match g.extensions.hooks.cleanup_return {
            Some(hook) => hook(g, c, x),
            None => fail(g, loc, "effect cleanups need the hook support installed"),
        };
    }
    if let Some(x) = e {
        if let Some(label) = c.return_label.clone() {
            if c.return_values.is_some() {
                let value = expr::emit(g, c, x, None, false)?;
                let index = c
                    .return_values
                    .as_ref()
                    .map(|values| values.len())
                    .unwrap_or(0);
                if let Some(values) = c.return_values.as_mut() {
                    values.push(scope::ReturnValue { value, loc: x.loc });
                }
                // parenthesized: a block value after a label reads like a
                // labeled loop (rustc's break_with_label_and_loop)
                return Ok(format!("break '{label} ({}{index}{});\n", '\u{5}', '\u{6}'));
            }
            let value = expr::emit(g, c, x, Some(c.ret), false)?;
            if let Some(code) = super::exceptions::return_code(c, Some(&value.code)) {
                return Ok(code);
            }
            return Ok(format!("break '{label} ({});\n", value.code));
        }
        if c.ret == Type::Void {
            if !matches!(x.data, ast::ExprData::Call(_)) {
                return fail(g, x.loc, "void functions cannot return a value");
            }
            if let ast::ExprData::Call(call) = &x.data
                && let ast::ExprData::Ident(name) = &call.callee.data
                && scope::setter_index(g, name).is_some()
            {
                let done = void_return(c);
                return Ok(format!("{}{done}", expression_statement(g, c, x)?));
            }
            let v = expr::emit(g, c, x, Some(Type::Void), false)?;
            return Ok(format!("{};\n{}", v.code, void_return(c)));
        }
        let v = expr::emit(g, c, x, Some(c.ret), false)?;
        if let Some(code) = super::exceptions::return_code(c, Some(&v.code)) {
            return Ok(code);
        }
        return Ok(format!("return {};\n", v.code));
    }
    if c.return_label.is_some() {
        return fail(g, loc, "array callbacks must return a value");
    }
    if c.ret != Type::Void {
        return fail(
            g,
            loc,
            format!("return needs a value of type {}", c.ret.rust(g)),
        );
    }
    Ok(void_return(c))
}

/// `return;` from here (through a `finally`, out of a throwing function).
fn void_return(c: &scope::Context) -> String {
    super::exceptions::return_code(c, None).unwrap_or_else(|| "return;\n".to_owned())
}

/// Restore the enclosing lexical scope after emitting a nested block.
fn block(g: &mut Generator<'_>, c: &mut scope::Context, ss: &[Box<ast::Stmt>]) -> Result<String> {
    let depth = c.active.len();
    let inner = statements(g, c, ss, false)?;
    c.active.truncate(depth);
    Ok(format!("{{\n{inner}}}\n"))
}

/// Emit boolean branches with a separate scope for each branch.
fn if_statement(g: &mut Generator<'_>, c: &mut scope::Context, v: &ast::If) -> Result<String> {
    let cond = expr::condition(g, c, &v.cond)?;
    let mut mark = narrowing::push(g, c, &v.cond, true)?;
    let then = scoped(g, c, &v.then)?;
    narrowing::restore(c, mark);
    mark = narrowing::push(g, c, &v.cond, false)?;
    let otherwise = match v.else_.as_deref() {
        Some(otherwise) => format!(" else {}", scoped(g, c, otherwise)?),
        None => String::new(),
    };
    narrowing::restore(c, mark);
    Ok(format!("if {} {then}{otherwise}\n", cond.code))
}

pub fn expression_statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
) -> Result<String> {
    if let ast::ExprData::Unary(unary) = &e.data
        && unary.op == b"delete"
    {
        return super::dicts::delete(g, c, &unary.operand);
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
            return super::place::assign(g, c, e, target);
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
        c.bindings[binding_index].assignments += 1;
        let rust = c.bindings[binding_index].rust.clone();
        let ty = c.bindings[binding_index].ty;
        if let ast::ExprData::Update(update) = &e.data {
            if !ty.numeric() {
                return fail(g, e.loc, "++ and -- require a number");
            }
            let one = if ty.float() { "1.0" } else { "1" };
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
            c.bindings[binding_index].fresh = super::place::fresh(&assign.value);
            c.bindings[binding_index].escaped = false;
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

pub fn stmt_returns(s: &ast::Stmt) -> bool {
    match &s.data {
        ast::StmtData::Ret(_) => true,
        ast::StmtData::Switch(v) => loops::returns(v),
        ast::StmtData::For(v) => {
            let unconditional = match v.cond.as_deref() {
                None => true,
                Some(cond) => matches!(cond.data, ast::ExprData::Boolean(true)),
            };
            unconditional && !loops::jumps(&v.body) && stmt_returns(&v.body)
        }
        ast::StmtData::While(v) => {
            let unconditional = matches!(v.cond.data, ast::ExprData::Boolean(true));
            (v.do_first || unconditional) && !loops::jumps(&v.body) && stmt_returns(&v.body)
        }
        ast::StmtData::Block(ss) => returns_all(ss),
        ast::StmtData::If(v) => match v.else_.as_deref() {
            Some(otherwise) => stmt_returns(&v.then) && stmt_returns(otherwise),
            None => false,
        },
        // a throw never falls through (TypeScript's `never`)
        ast::StmtData::Throw(_) => true,
        // every way out returns or throws: the protected block and the
        // catch block do, or the finally block does
        ast::StmtData::Try(t) => try_returns(t),
        _ => false,
    }
}

/// Every way out of the `try` returns or throws: the protected block and
/// the catch block do, or the finally block does.
pub fn try_returns(t: &ast::Try) -> bool {
    t.finally.as_deref().is_some_and(returns_all)
        || (returns_all(&t.body) && t.catch.as_ref().is_none_or(|c| returns_all(&c.body)))
}

pub fn returns_all(ss: &[Box<ast::Stmt>]) -> bool {
    for s in ss {
        if stmt_returns(s) {
            return true;
        }
        if loops::exits(s) {
            return false;
        }
    }
    false
}

/// `let _ = x;`: marks a binding as used; shared by tests.
pub fn discard(binding: &str) -> String {
    format!("let _ = {binding};\n")
}
