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

use super::components;
use super::expr;
use super::model;
use super::rust_text;
use super::scope;
use super::stmt;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

mod calls;
mod emission;
mod params;
#[cfg(test)]
mod tests;

pub use self::calls::*;
pub use self::emission::*;
pub use self::params::*;

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
    /// The type arguments of a generic function's instance (`T` → `f64`),
    /// bound while its body is generated (`generic_functions`).
    pub type_env: Vec<(Vec<u8>, Type)>,
    /// A throw can escape it: it returns `Result<ret, tsx::Thrown>`
    /// (`throws`, `exceptions`).
    pub throws: bool,
}

// ── parameter defaults ───────────────────────────────────────────────

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
    if !f.type_params.is_empty() {
        return fail(g, f.loc, "generic functions are not supported yet");
    }
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
        if p.rest && model::element(g, ty).is_none() {
            return fail(
                g,
                p.name.loc,
                "a rest parameter needs an array type: `...items: T[]`",
            );
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
        type_env: Vec::new(),
        throws: false,
    })
}

/// Registers this file's functions (before anything is emitted, so they can
/// call each other in any order).
pub fn collect(g: &mut Generator<'_>, module: &ast::Module) -> Result<()> {
    for top in &module.functions {
        if !is_function(&top.func) {
            if !top.func.type_params.is_empty() {
                return fail(g, top.func.loc, "generic components are not supported yet");
            }
            continue;
        }
        if !top.func.type_params.is_empty() {
            // a template: each call site's type arguments get an instance
            super::generic_functions::register(g, &top.func)?;
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
        let mut function = signature(g, &top.func, rust, false)?;
        function.throws = g.throwing.throws(&g.module_stem, &top.func.name);
        g.functions.push(function);
    }
    Ok(())
}
