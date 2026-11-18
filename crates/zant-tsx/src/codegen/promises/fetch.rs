//! `fetch(url, init)`, `Response`, `Headers`, `AbortController` and
//! `AbortSignal` (runtime: `zant::tsx::fetch`).

use crate::ast;

use super::super::expr;
use super::super::model;
use super::super::rust_text::{self, Precedence};
use super::super::scope;
use super::super::types::{self, Type, Value};
use super::super::{Generator, Result, fail};
use super::{CONTROLLER, HEADERS, RESPONSE, SIGNAL, any_type, is, opaque, promise_type};

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
        // `Promise.resolve(value?)`
        ast::ExprData::Member(member)
            if member.name.name == b"resolve"
                && matches!(&member.object.data, ast::ExprData::Ident(n) if n == b"Promise" && scope::local(c, n).is_none()) =>
        {
            let (code, ty) = match call.args.as_slice() {
                [] => ("()".to_owned(), Type::Void),
                [value] => {
                    let value = expr::emit(g, c, value, None, false)?;
                    let value = types::concrete(g, value)?;
                    (value.code, value.ty)
                }
                _ => return fail(g, loc, "`Promise.resolve(value)` takes one value"),
            };
            // already a promise: itself (JS adopts it)
            if super::promise_child(g, ty).is_some() {
                return Ok(Some(Value::new(ty, code)));
            }
            let promise = promise_type(g, ty)?;
            Ok(Some(Value::new(
                promise,
                format!("zant::tsx::Promise::resolve({code})"),
            )))
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
    if let Some(feature) = super::super::app_state::missing_feature("fetch") {
        return fail(
            g,
            loc,
            super::super::app_state::feature_message("fetch", feature),
        );
    }
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

/// Methods of responses, headers and controllers; `None` for other
/// receivers.
pub(super) fn method(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    receiver: &Value,
    name: &[u8],
    operand: &str,
) -> Result<Option<Value>> {
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
