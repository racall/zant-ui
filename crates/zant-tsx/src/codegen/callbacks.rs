//! Inline array callbacks become Rust closures inside iterator adapters.
//! Each callback is emitted as a closure (`.map(|item| …)`, `.filter_map(…)`,
//! `.fold(…)`): its parameters bind as scoped locals, and the result type of a
//! block body is inferred from its `return` statements.
//!
//! The closure's parameter *pattern* is passed separately from the arrow's
//! parameter order: `enumerate()` adapters hand `(index, item)` tuples while a
//! TSX arrow declares `(item, index)` (see `array_methods`).

use crate::ast;

use super::aggregate;
use super::expr;
use super::scope;
use super::stmt;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

/// Emit an inline arrow as a Rust closure.
///
/// `arguments` are the argument types in the arrow's parameter order; the
/// closure's own parameter list is `pattern` (a name, or a tuple pattern such
/// as `(index, item)`). Returns the closure source and its result type.
pub fn closure(
    g: &mut Generator<'_>,
    c: &scope::Context,
    callback: &ast::Expr,
    arguments: &[Type],
    pattern: &str,
    expected: Option<Type>,
) -> Result<(String, Type)> {
    let ast::ExprData::Arrow(function) = &callback.data else {
        return fail(g, callback.loc, "array callback must be an inline arrow");
    };
    if function.params.len() > arguments.len() {
        return fail(
            g,
            callback.loc,
            format!(
                "array callback expects at most {} parameters",
                arguments.len()
            ),
        );
    }
    // elements built here repeat once per item (`nesting`: keys)
    g.loop_depth += 1;
    let result = closure_body(g, c, callback, function, arguments, pattern, expected);
    g.loop_depth -= 1;
    result
}

fn closure_body(
    g: &mut Generator<'_>,
    outer: &scope::Context,
    callback: &ast::Expr,
    function: &ast::Function,
    arguments: &[Type],
    pattern: &str,
    expected: Option<Type>,
) -> Result<(String, Type)> {
    // The closure body is emitted lexically inside the enclosing body (a
    // render, a handler, or another closure), so the enclosing locals are in
    // scope and a Rust closure captures them directly; keep them visible.
    let mut visible: Vec<(String, Type, String)> = Vec::new();
    visible.extend(outer.active.iter().map(|index| {
        let binding = &outer.bindings[*index];
        // a narrowed local (`current && (…)`) keeps its proven type
        match super::narrowing::find(outer, binding.name.as_bytes()) {
            Some(value) => (binding.name.clone(), value.ty, value.code),
            None => (binding.name.clone(), binding.ty, binding.rust.clone()),
        }
    }));
    let mut c = scope::Context {
        render: outer.render,
        ret: Type::Void,
        capture_source: outer.capture_source.clone(),
        // a closure nested in a handler still reads the handler's captures
        // (`cap.field`) instead of re-capturing them
        capture_method: outer.capture_method,
        // emitted inside the handler: its runtime context stays visible
        cx_var: outer.cx_var.clone(),
        ..scope::Context::default()
    };
    for (param, ty) in function.params.iter().zip(arguments) {
        if param.optional || !param.destructure.is_empty() {
            return fail(
                g,
                param.name.loc,
                "array callback parameters must be simple names",
            );
        }
        if let Some(reference) = param.ty.as_ref()
            && types::type_ref(g, reference, false)? != *ty
        {
            return fail(
                g,
                reference.loc,
                format!("callback parameter must have type {}", ty.rust(g)),
            );
        }
        let binding = scope::bind(g, &mut c, &param.name.name, *ty, false)?;
        // closure parameters are already names: no `let` is emitted, but an
        // unused one still needs its `let _ = …;` discard in the body
        c.params.push(binding);
    }
    // enclosing locals after the parameters, so a parameter shadows one with
    // the same name; their initializer is none (they are already declared)
    for (name, ty, code) in visible {
        if c.bindings.iter().any(|binding| binding.name == name) {
            continue;
        }
        let binding = scope::bind(g, &mut c, name.as_bytes(), ty, false)?;
        c.bindings[binding].name = name.clone();
        c.bindings[binding].rust = code;
    }
    let annotation = match function.ret.as_ref() {
        Some(reference) => Some(types::type_ref(g, reference, false)?),
        None => expected,
    };
    let params = if arguments.is_empty() {
        "||".to_owned()
    } else {
        format!("|{pattern}|")
    };
    let mut ty = annotation.unwrap_or(Type::Void);
    let body = match &function.body {
        ast::Body::Expr(value) => {
            let value = expr::emit(g, &mut c, value, annotation, false)?;
            ty = annotation.unwrap_or_else(|| value.ty.concrete());
            let value = types::coerce(g, value, ty, callback.loc)?;
            let finished = scope::finish(g, &c, format!("{}\n", value.code))?;
            format!("{params} {{\n{finished}}}")
        }
        ast::Body::Block(statements) => {
            if !stmt::returns_all(statements) {
                return fail(g, callback.loc, "array callback must return on every path");
            }
            let label = aggregate::temporary(g, &mut c, b"_callback")?;
            match annotation {
                Some(annotation) => {
                    c.ret = annotation;
                    c.return_label = Some(label.clone());
                }
                None => {
                    c.return_label = Some(label.clone());
                    c.return_values = Some(Vec::new());
                }
            }
            let mut body = stmt::statements(g, &mut c, statements, false)?;
            if annotation.is_none() {
                let returned = c.return_values.take().unwrap_or_default();
                if returned.is_empty() {
                    return fail(g, callback.loc, "callback must return a value");
                }
                ty = returned[0].value.ty;
                for value in &returned[1..] {
                    ty = types::unify(g, ty, value.value.ty, value.loc)?;
                }
                ty = ty.concrete();
                for (index, value) in returned.into_iter().enumerate() {
                    let value = types::coerce(g, value.value, ty, value.loc)?;
                    body = body.replace(&format!("\u{5}{index}\u{6}"), &value.code);
                }
            }
            let finished = scope::finish(g, &c, body)?;
            // `|x| { 'callback: { … } }`: the labeled block is the value
            format!("{params} {{\n'{label}: {{\n{finished}}}\n}}")
        }
    };
    Ok((body, ty))
}

/// A zero-argument inline arrow as an immediately invoked closure (`useMemo`
/// bodies and store selectors).
pub fn inline(
    g: &mut Generator<'_>,
    c: &scope::Context,
    callback: &ast::Expr,
    expected: Option<Type>,
) -> Result<Value> {
    let (closure, ty) = closure(g, c, callback, &[], "", expected)?;
    Ok(Value {
        ty,
        code: format!("({closure})()"),
        ..Value::default()
    })
}
