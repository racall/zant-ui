//! Promise reactions (`then` / `catch` / `finally` callbacks): the call
//! site (a closure that reaches the component and calls the reaction
//! method) and the method body (`Result<value, Thrown>`, its value type
//! inferred from what it returns).

use crate::ast;

use super::super::app_state;
use super::super::expr;
use super::super::rust_text::{self, Precedence};
use super::super::scope;
use super::super::stmt;
use super::super::types::{self, Type, Value};
use super::super::{Generator, Result, fail};
use super::{ANY, decoded, is, promise_child, promise_type, wanted};

/// The body of reaction method `f`: a value-returning reaction without a
/// declared type gets the type of what it returns; a void one ends in
/// `Ok(())`.
pub fn body(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    f: &ast::Function,
    declared: Type,
) -> Result<(String, Type)> {
    let mut ret = declared;
    let body = match &f.body {
        // `.then(todo => setTitle(todo.title))`, `console.log(…)`: a statement
        ast::Body::Expr(e)
            if ret == Type::Void
                && matches!(&e.data, ast::ExprData::Call(call)
                    if matches!(&call.callee.data, ast::ExprData::Ident(name) if scope::setter_index(g, name).is_some())
                        || matches!(&call.callee.data, ast::ExprData::Member(member)
                            if matches!(&member.object.data, ast::ExprData::Ident(name) if name == b"console"))) =>
        {
            stmt::expression_statement(g, c, e)?
        }
        // `.then(r => r.json())`: the value's own type
        ast::Body::Expr(e) if ret == Type::Void => {
            let value = expr::emit(g, c, e, None, false)?;
            if value.ty == Type::Void {
                format!("{};\n", value.code)
            } else {
                let value = types::concrete(g, value)?;
                ret = value.ty;
                format!("return Ok({});\n", value.code)
            }
        }
        // `.then(r => { …; return r.text(); })`: the returns' common type
        ast::Body::Block(statements) if ret == Type::Void && returns_value(statements) => {
            let (body, ty) = reaction_block(g, c, f.loc, statements)?;
            ret = ty;
            body
        }
        ast::Body::Block(statements) => {
            super::super::generate::method_block(g, c, f.loc, statements)?
        }
        ast::Body::Expr(e) => super::super::generate::method_expression(g, c, e)?,
    };
    let body = if ret == Type::Void
        && !matches!(&f.body, ast::Body::Block(statements) if stmt::returns_all(statements))
    {
        // falling off the end of a void reaction
        format!("{body}Ok(())\n")
    } else {
        body
    };
    Ok((body, ret))
}

/// A `return value` in `statements`, outside nested functions.
fn returns_value(statements: &[Box<ast::Stmt>]) -> bool {
    use crate::ast_walk::{self, StmtChild};
    fn child(item: StmtChild<'_>) -> bool {
        match item {
            StmtChild::Stmt(s) => {
                if matches!(&s.data, ast::StmtData::Ret(Some(_))) {
                    return true;
                }
                let mut found = false;
                ast_walk::stmt_children(s, &mut |inner| found = found || child(inner));
                found
            }
            // arrows are not children: their returns are their own
            StmtChild::Expr(_) => false,
        }
    }
    statements.iter().any(|s| child(StmtChild::Stmt(s)))
}

/// A promise reaction's block whose returns give its value: a labeled
/// block collects them (as array callbacks do), then `Ok(value)`.
fn reaction_block(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    statements: &[Box<ast::Stmt>],
) -> Result<(String, Type)> {
    if !stmt::returns_all(statements) {
        return fail(g, loc, "this callback must return a value on every path");
    }
    let label = super::super::aggregate::temporary(g, c, b"_reaction")?;
    c.return_label = Some(label.clone());
    c.return_values = Some(Vec::new());
    let mut body = stmt::statements(g, c, statements, false)?;
    let returned = c.return_values.take().unwrap_or_default();
    c.return_label = None;
    let mut ty = returned[0].value.ty;
    for value in &returned[1..] {
        ty = types::unify(g, ty, value.value.ty, value.loc)?;
    }
    let ty = ty.concrete();
    for (index, value) in returned.into_iter().enumerate() {
        let value = types::coerce(g, value.value, ty, value.loc)?;
        body = body.replace(&format!("\u{5}{index}\u{6}"), &value.code);
    }
    Ok((
        format!(
            "let _returned: {} = '{label}: {{\n{body}}};\nreturn Ok(_returned);\n",
            ty.rust(g)
        ),
        ty,
    ))
}

pub(super) fn reaction(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    name: &[u8],
    mut receiver: Value,
    mut child: Type,
) -> Result<Value> {
    if !matches!(name, b"then" | b"catch" | b"finally") {
        return fail(g, loc, "Promise methods: then, catch, finally");
    }
    if c.render || c.constant {
        return fail(
            g,
            loc,
            "promise reactions run in event handlers and effects, not while rendering",
        );
    }
    if call.args.len() != 1 {
        return fail(
            g,
            loc,
            format!(
                "`{}(callback)` takes one callback",
                String::from_utf8_lossy(name)
            ),
        );
    }
    let callback = &call.args[0];
    let Some(info) = app_state::with_module(|state| {
        state
            .desktop_callbacks
            .iter()
            .find(|item| item.expr == **callback)
            .cloned()
    }) else {
        return fail(
            g,
            callback.loc,
            "the callback is an arrow or a function name",
        );
    };
    let thrown = super::super::exceptions::thrown_type(g)?;
    // a callback may ignore the value (`.then(() => …)`, `await save();`)
    let takes_value = g.methods[info.method]
        .func
        .as_ref()
        .is_some_and(|function| !function.params.is_empty());
    let params = match name {
        b"then" => {
            // `any` from `response.json()` meets its type here
            if is(g, child, ANY)
                && let Some(to) = wanted(g, callback)?
                && !is(g, to, ANY)
            {
                receiver = decoded(g, &receiver, to)?;
                child = to;
            }
            if is(g, child, ANY) && takes_value {
                return fail(
                    g,
                    callback.loc,
                    "give the JSON a type: `(items: Item[]) => …`, `.then(setItems)` or `r.json() as Promise<Item[]>`",
                );
            }
            if child == Type::Void || !takes_value {
                vec![]
            } else {
                vec![child]
            }
        }
        b"catch" if takes_value => vec![thrown],
        b"catch" => vec![],
        _ => vec![],
    };
    if let Some(function) = g.methods[info.method].func.clone()
        && let Some(reference) = &function.ret
    {
        g.methods[info.method].ret = types::type_ref(g, reference, true)?;
    }
    g.methods[info.method].callback_params = Some(params.clone());
    super::super::captures::prepare(g, c, info.method)?;
    let ret = g.methods[info.method].ret;
    // the runtime hands `then` / `catch` the value (`()` for a void promise)
    let value = match name {
        b"then" => Some((child, !params.is_empty())),
        b"catch" => Some((thrown, takes_value)),
        _ => None,
    };
    let closure = closure(g, c, info.method, value, ret)?;
    let operand = rust_text::operand(&receiver, Precedence::Suffix, false);
    match name {
        b"then" => match promise_child(g, ret) {
            // a callback returning a promise: adopted
            Some(inner) => {
                let ty = promise_type(g, inner)?;
                Ok(Value::new(ty, format!("{operand}.then_promise({closure})")))
            }
            None => {
                let ty = promise_type(g, ret)?;
                Ok(Value::new(ty, format!("{operand}.then({closure})")))
            }
        },
        b"catch" => {
            if ret != child {
                return fail(
                    g,
                    callback.loc,
                    "a catch callback gives the same kind of value as the promise (put `.catch` last, after the reactions that return nothing)",
                );
            }
            Ok(Value::new(
                receiver.ty,
                format!("{operand}.catch({closure})"),
            ))
        }
        _ => Ok(Value::new(
            receiver.ty,
            format!("{operand}.finally({closure})"),
        )),
    }
}

/// The reaction closure: `move |cx, value| -> Result<ret, Thrown>` calling
/// the method on the component instance.
fn closure(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    index: usize,
    value: Option<(Type, bool)>,
    ret: Type,
) -> Result<String> {
    let method = g.methods[index].clone();
    let capture = if method.captures.is_empty() {
        None
    } else {
        Some(super::super::captures::value_in(g, c, &method)?)
    };
    let needs_cx = method
        .func
        .as_deref()
        .is_some_and(|function| super::super::action_cx::needs_cx(g, function));
    let name = rust_text::id_at(g, method.name.as_bytes(), 0)?;
    let mut prefix = String::new();
    let mut arguments: Vec<String> = Vec::new();
    if let Some(capture) = &capture {
        prefix.push_str(&format!("let _capture = {capture};\n"));
        arguments.push("&_capture".to_owned());
    }
    let mut pattern = vec!["_cx: &mut ui::ActionCx<'_>".to_owned()];
    let mut values: Vec<String> = Vec::new();
    if let Some((ty, forwarded)) = value {
        let spelled = if ty == Type::Void {
            "()".to_owned()
        } else {
            ty.rust(g).to_owned()
        };
        pattern.push(format!("_value: {spelled}"));
        if forwarded {
            values.push("_value".to_owned());
        }
    }
    let value_type = if ret == Type::Void {
        "()".to_owned()
    } else {
        ret.rust(g).to_owned()
    };
    let body = match g.current_component {
        None => {
            if needs_cx {
                return fail(
                    g,
                    0,
                    "a reaction of the root component cannot call callback props",
                );
            }
            let root = rust_text::id_at(g, g.name.clone().as_bytes(), 0)?;
            arguments.extend(values);
            format!(
                "{root}::{name}(_cx.state::<{root}>(){})",
                arguments
                    .iter()
                    .map(|a| format!(", {a}"))
                    .collect::<String>()
            )
        }
        Some(component) => {
            let component =
                rust_text::id_at(g, g.components[component].name.clone().as_bytes(), 0)?;
            prefix.push_str("let _state = self._state.clone();\n");
            if needs_cx {
                arguments.push("_cx".to_owned());
            }
            arguments.extend(values);
            let arguments = arguments.join(", ");
            format!(
                "match _state.upgrade() {{\nSome(state) => state.borrow_mut().{name}({arguments}),\n\
                 None => {component}::default().{name}({arguments}),\n}}"
            )
        }
    };
    Ok(format!(
        "{{\n{prefix}move |{}| -> Result<{value_type}, zant::tsx::Thrown> {{\n{body}\n}}\n}}",
        pattern.join(", ")
    ))
}
