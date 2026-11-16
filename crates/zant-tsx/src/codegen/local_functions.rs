//! Functions declared inside an event handler or effect:
//!
//! ```tsx
//! useEffect(() => {
//!   const controller = new AbortController();
//!   async function load() {
//!     const res = await fetch(url, { signal: controller.signal });
//!     setItems(await res.json());
//!   }
//!   load();
//!   return () => controller.abort();
//! }, [url]);
//! ```
//!
//! Such a function may set state, so it is not a Rust closure (which would
//! borrow the component): it becomes a method of the component that
//! captures the handler's locals it reads (copied when it is declared, as
//! reactions copy theirs), and `load()` calls that method in place. A
//! function that is never declared on a handler path is not emitted.

use crate::ast;

use super::captures;
use super::expr;
use super::rust_text;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Method, Result, fail};

fn find(g: &Generator<'_>, function: &ast::Function) -> Option<usize> {
    g.methods.iter().position(|method| {
        method.local_function && method.func.as_ref().is_some_and(|f| f.loc == function.loc)
    })
}

/// Registers `function` (named `name`) declared in a handler body, unless
/// it already is a component method (a top-level component function).
pub fn scan(g: &mut Generator<'_>, name: &[u8], function: &ast::Function) -> Result<()> {
    let known = g
        .methods
        .iter()
        .any(|method| method.func.as_ref().is_some_and(|f| f.loc == function.loc));
    if known {
        return Ok(());
    }
    g.counter += 1;
    let base = format!("_{}_{}", String::from_utf8_lossy(name), g.counter);
    let method_name = scope::unique_method(g, &base)?;
    let capture_name = scope::unique_method(g, &format!("_LocalCapture{}", g.counter))?;
    g.methods.push(Method {
        name: method_name,
        func: Some(Box::new(function.clone())),
        capture_name: Some(capture_name),
        local_function: true,
        ..Method::default()
    });
    Ok(())
}

/// Whether `function` was registered by [`scan`].
pub fn registered(g: &Generator<'_>, function: &ast::Function) -> bool {
    find(g, function).is_some()
}

/// The declaration in a handler: emits the method (its captures read the
/// locals visible here) and makes `name` callable in the rest of the body.
pub fn declare(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    name: &[u8],
    function: &ast::Function,
) -> Result<String> {
    let Some(index) = find(g, function) else {
        return fail(
            g,
            loc,
            "functions inside functions are supported in event handlers and effects",
        );
    };
    let mut params = Vec::new();
    for param in &function.params {
        let Some(reference) = &param.ty else {
            return fail(
                g,
                param.name.loc,
                format!(
                    "parameter `{}` needs a type",
                    String::from_utf8_lossy(&param.name.name)
                ),
            );
        };
        params.push(types::type_ref(g, reference, false)?);
    }
    if let Some(reference) = &function.ret {
        g.methods[index].ret = types::type_ref(g, reference, true)?;
    }
    g.methods[index].callback_params = Some(params);
    // visible to itself (recursion) and to the statements after it
    c.local_functions.push((name.to_vec(), index));
    captures::prepare(g, c, index)?;
    Ok(String::new())
}

/// `load(args)` for a function declared earlier in this handler; `None`
/// for other names.
pub fn call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    name: &[u8],
    call: &ast::Call,
) -> Result<Option<Value>> {
    let Some(&(_, index)) = c.local_functions.iter().rev().find(|(n, _)| n == name) else {
        return Ok(None);
    };
    let method = g.methods[index].clone();
    let Some(function) = method.func.clone() else {
        return Ok(None);
    };
    let required = super::functions::required(&function.params);
    if call.args.len() < required || call.args.len() > function.params.len() {
        return fail(
            g,
            loc,
            format!(
                "`{}` expects {} arguments",
                String::from_utf8_lossy(name),
                function.params.len()
            ),
        );
    }
    // arguments first: the call borrows the component
    let mut setup = String::new();
    let mut arguments = vec!["self".to_owned()];
    if !method.captures.is_empty() {
        let capture = captures::value_in(g, c, &method)?;
        setup.push_str(&format!("let _capture = {capture};\n"));
        arguments.push("&_capture".to_owned());
    }
    if super::action_cx::needs_cx(g, &function) {
        match c.cx_var.clone() {
            Some(cx) => arguments.push(cx),
            None => {
                return fail(
                    g,
                    loc,
                    format!(
                        "`{}` calls a callback prop, so it only runs from an event handler",
                        String::from_utf8_lossy(name)
                    ),
                );
            }
        }
    }
    let types = method.callback_params.clone().unwrap_or_default();
    for (position, param) in function.params.iter().enumerate() {
        let ty = types.get(position).copied().unwrap_or(Type::Void);
        let value = match call.args.get(position) {
            Some(arg) => {
                let value = expr::emit(g, c, arg, Some(ty), false)?;
                types::coerce(g, value, ty, arg.loc)?.code
            }
            None => super::functions::missing(g, param, ty)?,
        };
        setup.push_str(&format!("let _arg{position} = {value};\n"));
        arguments.push(format!("_arg{position}"));
    }
    let target = rust_text::id_at(g, method.name.as_bytes(), loc)?;
    Ok(Some(Value::new(
        method.ret,
        format!("{{\n{setup}Self::{target}({})\n}}", arguments.join(", ")),
    )))
}
