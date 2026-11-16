//! Promises and `fetch` (runtime: `zant::tsx::{promise, fetch}`).
//!
//! ```tsx
//! useEffect(() => {
//!   fetch(url).then(r => r.json()).then(setItems).catch(e => setError(String(e)));
//! }, [url]);
//! ```
//!
//! A reaction (`then` / `catch` / `finally` callback) runs later, on the UI
//! thread, so it compiles like an event handler: a method of the component
//! (captures copied when the promise chain is built), called with the
//! value. Root components reach their state through the runtime context;
//! children through their state slot, or a default instance once they
//! unmounted (their updates go nowhere, like React's). A throw inside a
//! reaction rejects the promise it returns.
//!
//! `response.json()` is TypeScript's `any`: it is decoded where its type
//! appears — `as Promise<Item[]>`, an annotated reaction parameter
//! (`(items: Item[]) => …`) or a state setter (`.then(setItems)`); the
//! wrong shape rejects with a TypeError.

use crate::ast;

use super::app_state;
use super::expr;
use super::model::{self, Info, InfoKind};
use super::rust_text::{self, Precedence};
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Method, Result, fail};

const RESPONSE: &str = "zant::tsx::fetch::Response";
const HEADERS: &str = "zant::tsx::fetch::Headers";
const CONTROLLER: &str = "zant::tsx::fetch::AbortController";
const SIGNAL: &str = "zant::tsx::fetch::AbortSignal";
const ANY: &str = "zant::tsx::json::JsonValue";

fn find(g: &Generator<'_>, rust: &str) -> Option<Type> {
    g.compounds
        .iter()
        .position(|entry| entry.rust == rust)
        .map(|index| Type::Compound(index as u32))
}

fn opaque(g: &mut Generator<'_>, rust: &str) -> Result<Type> {
    if let Some(ty) = find(g, rust) {
        return Ok(ty);
    }
    model::add(
        g,
        Info {
            kind: InfoKind::Object,
            rust: rust.to_owned(),
            ..Info::default()
        },
    )
}

fn is(g: &Generator<'_>, ty: Type, rust: &str) -> bool {
    find(g, rust) == Some(ty)
}

/// `Promise<T>` (`T` void: `Promise<void>`).
pub fn promise_type(g: &mut Generator<'_>, child: Type) -> Result<Type> {
    let inner = if child == Type::Void {
        "()".to_owned()
    } else {
        child.rust(g).to_owned()
    };
    let rust = format!("zant::tsx::Promise<{inner}>");
    if let Some(ty) = find(g, &rust) {
        return Ok(ty);
    }
    model::add(
        g,
        Info {
            kind: InfoKind::Object,
            rust,
            child,
            ..Info::default()
        },
    )
}

/// The `T` of `Promise<T>`.
pub fn promise_child(g: &Generator<'_>, ty: Type) -> Option<Type> {
    model::info(g, ty)
        .filter(|info| info.rust.starts_with("zant::tsx::Promise<"))
        .map(|info| info.child)
}

/// TypeScript's `any` of `response.json()`.
pub fn any_type(g: &mut Generator<'_>) -> Result<Type> {
    opaque(g, ANY)
}

/// `Promise<T>`, `Response`, `Headers`, `AbortController`, `AbortSignal`
/// in a type position.
pub fn named_type(g: &mut Generator<'_>, reference: &ast::TypeRef) -> Result<Option<Type>> {
    if g.type_decls
        .iter()
        .any(|decl| decl.name.name == reference.name)
    {
        return Ok(None);
    }
    match reference.name.as_slice() {
        b"Promise" => {
            let [arg] = reference.args.as_slice() else {
                return fail(g, reference.loc, "`Promise<T>` takes one type argument");
            };
            let child = types::type_ref(g, arg, true)?;
            promise_type(g, child).map(Some)
        }
        b"Response" => opaque(g, RESPONSE).map(Some),
        b"Headers" => opaque(g, HEADERS).map(Some),
        b"AbortController" => opaque(g, CONTROLLER).map(Some),
        b"AbortSignal" => opaque(g, SIGNAL).map(Some),
        _ => Ok(None),
    }
}

// ── scanning ─────────────────────────────────────────────────────────

/// `p.then(cb)` / `.catch(cb)` / `.finally(cb)`: registers the callback as
/// a method (before emission, like timer callbacks). Any receiver: one that
/// turns out not to be a promise leaves the method unused.
pub fn scan(g: &mut Generator<'_>, call: &ast::Call) -> Result<()> {
    let ast::ExprData::Member(member) = &call.callee.data else {
        return Ok(());
    };
    let arity = match member.name.name.as_slice() {
        b"then" | b"catch" => 1,
        b"finally" => 0,
        _ => return Ok(()),
    };
    let Some(callback) = call.args.first() else {
        return Ok(());
    };
    let arrow = match &callback.data {
        ast::ExprData::Arrow(function) => (**function).clone(),
        ast::ExprData::Ident(_) => {
            let forwarded = super::events::forward_arrow(callback, arity);
            let ast::ExprData::Arrow(function) = forwarded.data else {
                unreachable!("forward arrow")
            };
            *function
        }
        _ => return Ok(()),
    };
    g.counter += 1;
    let method = g.methods.len();
    app_state::with_module(|state| {
        state.desktop_callbacks.push(app_state::DesktopCallback {
            expr: (**callback).clone(),
            method,
        })
    });
    let name = scope::unique_method(g, &format!("_reaction{}", g.counter))?;
    let capture_name = scope::unique_method(g, &format!("_ReactionCapture{}", g.counter))?;
    g.methods.push(Method {
        name,
        func: Some(Box::new(arrow)),
        capture_name: Some(capture_name),
        promise_callback: true,
        ..Method::default()
    });
    Ok(())
}

// ── calls ────────────────────────────────────────────────────────────

/// `fetch(…)`, `new AbortController()`, `AbortSignal.timeout(ms)`.
pub fn call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<Value>> {
    match &call.callee.data {
        ast::ExprData::Ident(name) if name == b"fetch" && scope::local(c, name).is_none() => {
            fetch(g, c, loc, call).map(Some)
        }
        ast::ExprData::Ident(name)
            if name == b"AbortController" && call.is_new && scope::local(c, name).is_none() =>
        {
            let ty = opaque(g, CONTROLLER)?;
            Ok(Some(Value::new(ty, format!("{CONTROLLER}::new()"))))
        }
        ast::ExprData::Member(member)
            if member.name.name == b"timeout"
                && matches!(&member.object.data, ast::ExprData::Ident(n) if n == b"AbortSignal") =>
        {
            let [ms] = call.args.as_slice() else {
                return fail(g, loc, "`AbortSignal.timeout(ms)`");
            };
            let ms = expr::emit(g, c, ms, Some(Type::F64), false)?;
            let ms = types::coerce(g, ms, Type::F64, call.args[0].loc)?;
            let ty = opaque(g, SIGNAL)?;
            Ok(Some(Value::new(
                ty,
                format!("{SIGNAL}::timeout({})", ms.code),
            )))
        }
        _ => Ok(None),
    }
}

fn fetch(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Value> {
    if c.render || c.constant {
        return fail(
            g,
            loc,
            "fetch runs in event handlers and effects (`useEffect(() => { fetch(…) }, [deps])`), not while rendering",
        );
    }
    if call.args.is_empty() || call.args.len() > 2 {
        return fail(g, loc, "`fetch(url, { method, headers, body, signal })`");
    }
    let url = expr::emit(g, c, &call.args[0], Some(Type::String), false)?;
    let url = types::coerce(g, url, Type::String, call.args[0].loc)?;
    let init = match call.args.get(1) {
        Some(init) => request_init(g, c, init)?,
        None => "zant::tsx::fetch::RequestInit::default()".to_owned(),
    };
    let response = opaque(g, RESPONSE)?;
    let ty = promise_type(g, response)?;
    Ok(Value::new(
        ty,
        format!(
            "zant::tsx::fetch::fetch({}.as_str(), {init})",
            rust_text::operand(&url, Precedence::Suffix, false)
        ),
    ))
}

/// `{ method, headers, body, signal }`.
fn request_init(g: &mut Generator<'_>, c: &mut scope::Context, init: &ast::Expr) -> Result<String> {
    let ast::ExprData::Object(properties) = &init.data else {
        return fail(
            g,
            init.loc,
            "write the options inline: `fetch(url, { method: \"POST\", headers: { … }, body })`",
        );
    };
    let mut fields = Vec::new();
    for property in properties {
        if property.spread {
            return fail(g, property.value.loc, "list the fetch options one by one");
        }
        match property.key.name.as_slice() {
            b"method" | b"body" => {
                let value = expr::emit(g, c, &property.value, Some(Type::String), false)?;
                let value = types::coerce(g, value, Type::String, property.value.loc)?;
                if property.key.name == b"method" {
                    fields.push(format!("method: {}", value.code));
                } else {
                    fields.push(format!("body: Some({})", value.code));
                }
            }
            b"headers" => fields.push(format!("headers: {}", headers(g, c, &property.value)?)),
            b"signal" => {
                let value = expr::emit(g, c, &property.value, None, false)?;
                if !is(g, value.ty, SIGNAL) {
                    return fail(
                        g,
                        property.value.loc,
                        "`signal` takes an AbortSignal (`controller.signal`, `AbortSignal.timeout(ms)`)",
                    );
                }
                fields.push(format!("signal: Some({})", value.code));
            }
            other => {
                return fail(
                    g,
                    property.key.loc,
                    format!(
                        "fetch option `{}` is not supported (method, headers, body, signal)",
                        String::from_utf8_lossy(other)
                    ),
                );
            }
        }
    }
    Ok(format!(
        "zant::tsx::fetch::RequestInit {{ {}, ..Default::default() }}",
        fields.join(", ")
    ))
}

/// `headers: { "Content-Type": "application/json" }` or a `Headers`.
fn headers(g: &mut Generator<'_>, c: &mut scope::Context, value: &ast::Expr) -> Result<String> {
    let ast::ExprData::Object(entries) = &value.data else {
        let headers = expr::emit(g, c, value, None, false)?;
        if !is(g, headers.ty, HEADERS) {
            return fail(
                g,
                value.loc,
                "headers are an object literal of strings or a Headers",
            );
        }
        return Ok(headers.code);
    };
    let mut code = String::from("{ let mut _headers = zant::tsx::fetch::Headers::new();\n");
    for entry in entries {
        if entry.spread {
            return fail(g, entry.value.loc, "list the headers one by one");
        }
        let name = String::from_utf8_lossy(&entry.key.name).into_owned();
        let text = expr::emit(g, c, &entry.value, Some(Type::String), false)?;
        let text = types::coerce(g, text, Type::String, entry.value.loc)?;
        code.push_str(&format!(
            "_headers.append({name:?}, {}.as_str());\n",
            rust_text::operand(&text, Precedence::Suffix, false)
        ));
    }
    code.push_str("_headers }");
    Ok(code)
}

// ── members and methods ──────────────────────────────────────────────

/// `response.ok`, `controller.signal`, `signal.aborted`, …
pub fn member_read(
    g: &mut Generator<'_>,
    ty: Type,
    operand: &str,
    name: &ast::Name,
) -> Result<Option<Value>> {
    let field = String::from_utf8_lossy(&name.name).into_owned();
    if is(g, ty, RESPONSE) {
        let (ty, code) = match field.as_str() {
            "ok" => (Type::Boolean, format!("{operand}.ok")),
            "status" => (Type::F64, format!("{operand}.status")),
            "statusText" => (Type::String, format!("{operand}.status_text.clone()")),
            "url" => (Type::String, format!("{operand}.url.clone()")),
            "redirected" => (Type::Boolean, format!("{operand}.redirected")),
            "bodyUsed" => (Type::Boolean, format!("{operand}.body_used()")),
            "headers" => (opaque(g, HEADERS)?, format!("{operand}.headers.clone()")),
            _ => {
                return fail(
                    g,
                    name.loc,
                    format!(
                        "Response has no `{field}` (ok, status, statusText, url, redirected, headers, bodyUsed, text(), json())"
                    ),
                );
            }
        };
        return Ok(Some(Value::new(ty, code)));
    }
    if is(g, ty, CONTROLLER) && field == "signal" {
        let ty = opaque(g, SIGNAL)?;
        return Ok(Some(Value::new(ty, format!("{operand}.signal.clone()"))));
    }
    if is(g, ty, SIGNAL) && field == "aborted" {
        return Ok(Some(Value::new(
            Type::Boolean,
            format!("{operand}.aborted()"),
        )));
    }
    Ok(None)
}

/// Methods of promises, responses, headers and controllers; `None` for
/// other receivers.
pub fn method(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    receiver: &Value,
) -> Result<Option<Value>> {
    let ast::ExprData::Member(member) = &call.callee.data else {
        return Ok(None);
    };
    let name = member.name.name.as_slice();
    let operand = rust_text::operand(receiver, Precedence::Suffix, false);
    if let Some(child) = promise_child(g, receiver.ty) {
        return reaction(g, c, loc, call, name, receiver.clone(), child).map(Some);
    }
    if is(g, receiver.ty, RESPONSE) {
        return match name {
            b"text" => {
                let ty = promise_type(g, Type::String)?;
                Ok(Some(Value::new(ty, format!("{operand}.text()"))))
            }
            b"json" => {
                let any = any_type(g)?;
                let ty = promise_type(g, any)?;
                Ok(Some(Value::new(ty, format!("{operand}.json_value()"))))
            }
            _ => fail(g, loc, "Response methods: text(), json()"),
        };
    }
    if is(g, receiver.ty, HEADERS) {
        let [arg] = call.args.as_slice() else {
            return fail(g, loc, "headers.get(name) / headers.has(name)");
        };
        let key = expr::emit(g, c, arg, Some(Type::String), false)?;
        let key = types::coerce(g, key, Type::String, arg.loc)?;
        let key = rust_text::operand(&key, Precedence::Suffix, false);
        return match name {
            b"get" => {
                let ty = model::optional(g, Type::String)?;
                Ok(Some(Value::new(
                    ty,
                    format!("{operand}.get({key}.as_str())"),
                )))
            }
            b"has" => Ok(Some(Value::new(
                Type::Boolean,
                format!("{operand}.has({key}.as_str())"),
            ))),
            _ => fail(g, loc, "Headers methods: get(name), has(name)"),
        };
    }
    if is(g, receiver.ty, CONTROLLER) && name == b"abort" {
        return Ok(Some(Value::new(Type::Void, format!("{operand}.abort()"))));
    }
    Ok(None)
}

/// `p as Promise<Item[]>` where `p` holds `response.json()`'s `any`.
pub fn cast(g: &mut Generator<'_>, value: &Value, target: Type) -> Result<Option<Value>> {
    let (Some(from), Some(to)) = (promise_child(g, value.ty), promise_child(g, target)) else {
        return Ok(None);
    };
    if !is(g, from, ANY) || is(g, to, ANY) {
        return Ok(None);
    }
    Ok(Some(decoded(g, value, to)?))
}

/// `promise` (holding `any`) decoded into `Promise<to>`.
fn decoded(g: &mut Generator<'_>, promise: &Value, to: Type) -> Result<Value> {
    if let Err(why) = super::json::check(g, to) {
        return fail(g, 0, why);
    }
    let ty = promise_type(g, to)?;
    Ok(Value::new(
        ty,
        format!(
            "{}.then(|_, _json: zant::tsx::json::JsonValue| zant::tsx::json::decode::<{}>(&_json))",
            rust_text::operand(promise, Precedence::Suffix, false),
            to.rust(g)
        ),
    ))
}

/// The type a reaction's first parameter asks for: its annotation, or the
/// state of a setter it forwards to (`.then(setItems)`, `.then(x =>
/// setItems(x))`).
fn wanted(g: &mut Generator<'_>, callback: &ast::Expr) -> Result<Option<Type>> {
    match &callback.data {
        ast::ExprData::Ident(name) => {
            Ok(scope::setter_index(g, name).map(|index| g.states[index].ty))
        }
        ast::ExprData::Arrow(function) => {
            let Some(param) = function.params.first() else {
                return Ok(None);
            };
            if let Some(reference) = &param.ty {
                return types::type_ref(g, reference, false).map(Some);
            }
            // `x => setItems(x)`
            if let ast::Body::Expr(body) = &function.body
                && let ast::ExprData::Call(inner) = &body.data
                && let ast::ExprData::Ident(setter) = &inner.callee.data
                && matches!(inner.args.as_slice(), [arg] if matches!(&arg.data, ast::ExprData::Ident(n) if *n == param.name.name))
            {
                return Ok(scope::setter_index(g, setter).map(|index| g.states[index].ty));
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn reaction(
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
    let thrown = super::exceptions::thrown_type(g)?;
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
            if is(g, child, ANY) {
                return fail(
                    g,
                    callback.loc,
                    "give the JSON a type: `(items: Item[]) => …`, `.then(setItems)` or `r.json() as Promise<Item[]>`",
                );
            }
            if child == Type::Void {
                vec![]
            } else {
                vec![child]
            }
        }
        b"catch" => vec![thrown],
        _ => vec![],
    };
    if let Some(function) = g.methods[info.method].func.clone()
        && let Some(reference) = &function.ret
    {
        g.methods[info.method].ret = types::type_ref(g, reference, true)?;
    }
    g.methods[info.method].callback_params = Some(params.clone());
    super::captures::prepare(g, c, info.method)?;
    let ret = g.methods[info.method].ret;
    // the runtime hands `then` / `catch` the value (`()` for a void promise)
    let value = match name {
        b"then" => Some((child, !params.is_empty())),
        b"catch" => Some((thrown, true)),
        _ => None,
    };
    let closure = closure(g, info.method, value, ret)?;
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
    index: usize,
    value: Option<(Type, bool)>,
    ret: Type,
) -> Result<String> {
    let method = g.methods[index].clone();
    let capture = if method.captures.is_empty() {
        None
    } else {
        Some(super::captures::value(g, &method)?)
    };
    let needs_cx = method
        .func
        .as_deref()
        .is_some_and(|function| super::action_cx::needs_cx(g, function));
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
