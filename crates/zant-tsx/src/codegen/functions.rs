//! Plain top-level functions (not components): a lowercase name, typed
//! parameters and a return type, like React's convention that only
//! capitalized functions are components.
//!
//! ```tsx
//! export function bytes(n: number): string { … }
//! ```
//!
//! becomes `pub fn bytes(n: f64) -> zant::Str`, callable from components, from
//! other functions (also recursively) and from other files.
//!
//! The parameter-default helpers ([`param_type`], [`required`], [`missing`],
//! [`prop`]) live here as well: this module and [`super::expr`] are their main
//! users.

use crate::ast;

use super::expr;
use super::model;
use super::rust_text;
use super::scope;
use super::stmt;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

#[derive(Clone, Debug)]
pub struct Function {
    /// The name in this file.
    pub name: String,
    /// The Rust callee: `bytes` or `_format::bytes`.
    pub rust: String,
    pub func: Box<ast::Function>,
    pub params: Vec<Type>,
    pub ret: Type,
    pub external: bool,
}

// ── parameter defaults ───────────────────────────────────────────────

/// The default as a value of `ty`.
pub fn default_value(g: &mut Generator<'_>, e: &ast::Expr, ty: Type) -> Result<Value> {
    let mut c = scope::Context {
        render: true,
        ret: Type::Void,
        constant: true,
        ..scope::Context::default()
    };
    let value = expr::emit(g, &mut c, e, Some(ty), false)?;
    types::coerce(g, value, ty, e.loc)
}

/// A parameter's type: its annotation, else its default's.
pub fn param_type(g: &mut Generator<'_>, p: &ast::Param, owner: &[u8]) -> Result<Option<Type>> {
    if let Some(reference) = p.ty.as_ref() {
        return Ok(Some(types::type_ref(g, reference, false)?));
    }
    let Some(default) = p.default.as_deref() else {
        return Ok(None);
    };
    let mut c = scope::Context {
        render: true,
        ret: Type::Void,
        constant: true,
        ..scope::Context::default()
    };
    let value = expr::emit(g, &mut c, default, None, false)?;
    if value.ty == Type::Null || value.ty == Type::Void {
        return fail(
            g,
            default.loc,
            format!(
                "parameter `{}` of `{}` needs a type for this default",
                String::from_utf8_lossy(&p.name.name),
                String::from_utf8_lossy(owner)
            ),
        );
    }
    Ok(Some(value.ty.concrete()))
}

/// Required arguments: those before the first optional or defaulted one.
pub fn required(params: &[ast::Param]) -> usize {
    for (index, param) in params.iter().enumerate() {
        if param.optional || param.default.is_some() {
            return index;
        }
    }
    params.len()
}

/// The argument code for a parameter the call left out.
pub fn missing(g: &mut Generator<'_>, p: &ast::Param, ty: Type) -> Result<String> {
    let Some(default) = p.default.as_deref() else {
        // an optional parameter
        return Ok("None".to_owned());
    };
    Ok(default_value(g, default, ty)?.code)
}

/// `{ size = 14 }`: reading `size` gives `props.size.clone().unwrap_or(14)`.
pub fn prop(g: &mut Generator<'_>, default: &ast::Expr, ty: Type, code: &str) -> Result<Value> {
    let Some(child) = model::optional_child(g, ty) else {
        // TypeScript permits defaults on required properties. Their values
        // are already defined, so only validate the fallback's type.
        default_value(g, default, ty)?;
        return Ok(scope::owned_read(g, ty, code.to_owned()));
    };
    let fallback = default_value(g, default, child)?;
    Ok(Value {
        ty: child,
        // clone: the prop is read through `&self` (an `Option<Str>` cannot move out)
        code: format!("({code}).clone().unwrap_or({})", fallback.code),
        ..Value::default()
    })
}

// ── functions ────────────────────────────────────────────────────────

/// Components start with a capital letter; everything else is a function.
pub fn is_function(f: &ast::Function) -> bool {
    match f.name.first() {
        Some(first) => !first.is_ascii_uppercase(),
        None => false,
    }
}

/// Resolves the signature of `f` (types in the current type scope).
pub fn signature(
    g: &mut Generator<'_>,
    f: &ast::Function,
    rust: String,
    external: bool,
) -> Result<Function> {
    let mut params: Vec<Type> = Vec::new();
    for p in &f.params {
        if !p.destructure.is_empty() {
            return fail(g, p.name.loc, "function parameters are plain names");
        }
        let Some(mut ty) = param_type(g, p, &f.name)? else {
            return fail(
                g,
                p.name.loc,
                format!(
                    "parameter `{}` of `{}` needs a type",
                    String::from_utf8_lossy(&p.name.name),
                    String::from_utf8_lossy(&f.name)
                ),
            );
        };
        if p.optional {
            ty = model::optional(g, ty)?;
        }
        params.push(ty);
    }
    let ret = match f.ret.as_ref() {
        Some(reference) => types::type_ref(g, reference, true)?,
        None => Type::Void,
    };
    if ret == Type::Node || ret == Type::OptionalNode {
        return fail(
            g,
            f.loc,
            format!(
                "`{}` returns elements: components start with a capital letter",
                String::from_utf8_lossy(&f.name)
            ),
        );
    }
    Ok(Function {
        name: String::from_utf8_lossy(&f.name).into_owned(),
        rust,
        func: Box::new(f.clone()),
        params,
        ret,
        external,
    })
}

/// Registers this file's functions (before anything is emitted, so they can
/// call each other in any order).
pub fn collect(g: &mut Generator<'_>, module: &ast::Module) -> Result<()> {
    for top in &module.functions {
        if !is_function(&top.func) {
            continue;
        }
        if top.default {
            return fail(
                g,
                top.func.loc,
                "the default export is a component (capitalize it)",
            );
        }
        if g.functions
            .iter()
            .any(|other| other.name.as_bytes() == top.func.name)
        {
            return fail(
                g,
                top.func.loc,
                format!(
                    "duplicate function `{}`",
                    String::from_utf8_lossy(&top.func.name)
                ),
            );
        }
        let rust = rust_text::id_at(g, &top.func.name, top.func.loc)?;
        let function = signature(g, &top.func, rust, false)?;
        g.functions.push(function);
    }
    Ok(())
}

pub fn find_index(g: &Generator<'_>, name: &[u8]) -> Option<usize> {
    g.functions
        .iter()
        .position(|function| function.name.as_bytes() == name)
}

/// `bytes(n)` where `bytes` is a function of this file or an imported one.
pub fn try_call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<Value>> {
    let ast::ExprData::Ident(name) = &call.callee.data else {
        return Ok(None);
    };
    if scope::local(c, name).is_some() {
        return Ok(None);
    }
    let Some(index) = find_index(g, name) else {
        return Ok(None);
    };
    let function = g.functions[index].clone();
    let name = String::from_utf8_lossy(name).into_owned();
    if !call.type_args.is_empty() {
        return fail(g, loc, format!("`{name}` takes no type arguments"));
    }
    let required = required(&function.func.params);
    if call.args.len() < required || call.args.len() > function.params.len() {
        return fail(
            g,
            loc,
            format!("`{name}` takes {} arguments", function.params.len()),
        );
    }
    let mut arguments: Vec<String> = Vec::new();
    for (index, ty) in function.params.iter().enumerate() {
        if index >= call.args.len() {
            arguments.push(missing(g, &function.func.params[index], *ty)?);
            continue;
        }
        let value = expr::emit(g, c, &call.args[index], Some(*ty), false)?;
        arguments.push(types::coerce(g, value, *ty, call.args[index].loc)?.code);
    }
    Ok(Some(Value {
        ty: function.ret,
        code: format!("{}({})", function.rust, arguments.join(", ")),
        ..Value::default()
    }))
}

/// The Rust declarations of this file's functions.
pub fn emit_all(g: &mut Generator<'_>) -> Result<String> {
    let mut out = String::new();
    for index in 0..g.functions.len() {
        if g.functions[index].external {
            continue;
        }
        out.push_str(&emit(g, index)?);
        out.push('\n');
    }
    Ok(out)
}

fn emit(g: &mut Generator<'_>, index: usize) -> Result<String> {
    let function = g.functions[index].clone();
    let mut c = scope::Context {
        render: false,
        ret: function.ret,
        no_self: true,
        ..scope::Context::default()
    };
    let mut params: Vec<String> = Vec::new();
    for (param, ty) in function.func.params.iter().zip(function.params.iter()) {
        let binding = scope::bind(g, &mut c, &param.name.name, *ty, false)?;
        c.params.push(binding);
        params.push(format!("{}: {}", c.bindings[binding].rust, ty.rust(g)));
    }
    let body = match &function.func.body {
        ast::Body::Block(ss) => {
            if function.ret != Type::Void && !stmt::returns_all(ss) {
                return fail(
                    g,
                    function.func.loc,
                    format!("`{}` must return a value on every path", function.name),
                );
            }
            stmt::statements(g, &mut c, ss, false)?
        }
        ast::Body::Expr(e) => {
            if function.ret == Type::Void {
                stmt::expression_statement(g, &mut c, e)?
            } else {
                let value = expr::emit(g, &mut c, e, Some(function.ret), false)?;
                format!("return {};\n", value.code)
            }
        }
    };
    let finished = scope::finish(g, &c, body)?;
    let return_type = if function.ret == Type::Void {
        String::new()
    } else {
        format!(" -> {}", function.ret.rust(g))
    };
    Ok(format!(
        "pub fn {}({}){return_type} {{\n{finished}}}\n",
        function.rust,
        params.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::Generator;

    fn parse(src: &str) -> ast::Module {
        let mut parser = crate::parser::Parser::init(src);
        parser.parse_module().expect("test source parses")
    }

    fn with_generator(f: impl FnOnce(&mut Generator<'_>)) {
        let mut diag = None;
        let mut g = Generator::new(&mut diag);
        f(&mut g);
    }

    #[test]
    fn component_names_start_with_an_uppercase_letter() {
        let module = parse(
            r#"function bytes(n: number): number { return n; }
function Row(props: number): void {}"#,
        );
        assert!(is_function(&module.functions[0].func));
        assert!(!is_function(&module.functions[1].func));
    }

    #[test]
    fn required_stops_at_the_first_optional_or_defaulted_parameter() {
        let module = parse(
            r#"function label(n: number, unit = "item", extra?: boolean): string { return unit; }"#,
        );
        let params = &module.functions[0].func.params;
        assert_eq!(1, required(params));
        assert!(params[1].default.is_some());
        assert!(params[2].optional);
    }

    #[test]
    fn a_missing_optional_argument_is_none_and_a_default_is_emitted() {
        let module = parse(
            r#"function label(n: number, unit = "item", extra?: boolean): string { return unit; }"#,
        );
        let params = &module.functions[0].func.params;
        with_generator(|g| {
            assert_eq!("None", missing(g, &params[2], Type::Boolean).unwrap());
            assert_eq!(
                "zant::Str::from(\"item\")",
                missing(g, &params[1], Type::String).unwrap()
            );
        });
    }

    #[test]
    fn duplicate_and_untyped_functions_are_diagnostics() {
        let module = parse(
            r#"function twice(n): number { return n; }
function twice(n: number): number { return n; }"#,
        );
        with_generator(|g| {
            assert!(collect(g, &module).is_err());
            let diag = g.diag.as_ref().expect("a diagnostic");
            assert!(
                diag.msg.contains("parameter `n` of `twice` needs a type"),
                "{}",
                diag.msg
            );
        });
    }
}
