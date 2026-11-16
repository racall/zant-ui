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
use super::model::{self, Info, InfoKind};
use super::rust_text::{self, Precedence};
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Method, Result, fail};

mod fetch;
pub mod reaction;

pub use fetch::{call, member_read};

pub(super) const RESPONSE: &str = "zant::tsx::fetch::Response";
pub(super) const HEADERS: &str = "zant::tsx::fetch::Headers";
pub(super) const CONTROLLER: &str = "zant::tsx::fetch::AbortController";
pub(super) const SIGNAL: &str = "zant::tsx::fetch::AbortSignal";
pub(super) const ANY: &str = "zant::tsx::json::JsonValue";

fn find(g: &Generator<'_>, rust: &str) -> Option<Type> {
    g.compounds
        .iter()
        .position(|entry| entry.rust == rust)
        .map(|index| Type::Compound(index as u32))
}

pub(super) fn opaque(g: &mut Generator<'_>, rust: &str) -> Result<Type> {
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

pub(super) fn is(g: &Generator<'_>, ty: Type, rust: &str) -> bool {
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
        return reaction::reaction(g, c, loc, call, name, receiver.clone(), child).map(Some);
    }
    fetch::method(g, c, loc, call, receiver, name, &operand)
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
pub(super) fn decoded(g: &mut Generator<'_>, promise: &Value, to: Type) -> Result<Value> {
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
pub(super) fn wanted(g: &mut Generator<'_>, callback: &ast::Expr) -> Result<Option<Type>> {
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
