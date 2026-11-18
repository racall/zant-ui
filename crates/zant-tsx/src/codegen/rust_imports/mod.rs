//! Public functions and types of hand-written Rust files, used from TSX
//! (`import { plural } from "./format.rs"`, `import type { Peer } from
//! "./sync.rs"`). The files are parsed with `syn`.
//!
//! A project file ending in `.rs` is copied next to the generated modules and
//! declared as a Rust module by the generated entry, so its `pub fn`s are
//! callable as `crate::format::plural(..)` and its `pub struct` / `pub enum`
//! types are named `crate::format::Peer`.
//!
//! Only the small typed boundary documented in `docs/tsx.md` is mapped:
//! `bool`, the numbers, `String` / `Str` / `&str`, `Option<T>`, `Vec<T>` /
//! `List<T>` / `&[T]`, `tsx::Bytes`, structs and enums of the same file.
//! Anything else is a diagnostic naming the offending signature.

use quote::ToTokens;

use crate::ast;

use super::app_state;
use super::expr;
use super::model;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

/// A hand-written project file: its parsed `syn` tree lives in
/// [`app_state::RustSource`].
pub use super::app_state::RustSource as Source;

mod calls;
mod rust_types;

#[cfg(test)]
mod tests;

pub use self::calls::*;
pub use self::rust_types::*;

/// An imported function, after its signature has been checked and mapped to
/// TSX types.
#[derive(Clone, Debug)]
pub struct Function {
    pub local: String,
    pub target: String,
    pub params: Vec<Type>,
    pub ret: Type,
}

/// A type imported by name into this file.
#[derive(Clone, Debug)]
pub struct ImportedType {
    pub local: String,
    pub ty: Type,
}

/// Register named imports.
pub fn process(g: &mut Generator<'_>, import: &ast::Import, source: &str) -> Result<()> {
    let Some(project) = app_state::project() else {
        return fail(
            g,
            import.source_loc,
            "Rust imports need a project (zant-tsx --out / zant_build::tsx_app)",
        );
    };
    let Some(module) = project.find_rust(source) else {
        return fail(
            g,
            import.source_loc,
            format!("cannot find `{source}` among the project's .rs files"),
        );
    };
    let module_name = super::imports::module_name(&module.name);
    let alias = format!("crate::{module_name}");
    let file = module.file.clone();
    for n in &import.names {
        let imported = String::from_utf8_lossy(&n.imported.name).into_owned();
        let local = String::from_utf8_lossy(&n.local.name).into_owned();
        if n.type_only || import.type_only {
            match resolve_type(g, &alias, &file, &imported)? {
                Some(ty) => app_state::with_module(|state| {
                    state.rust_types.push(ImportedType {
                        local: local.clone(),
                        ty,
                    })
                }),
                None => {
                    return fail(
                        g,
                        n.imported.loc,
                        format!("`{imported}` is not a pub struct or pub enum of `{source}`"),
                    );
                }
            }
            continue;
        }
        let Some(function) = find(&file, &imported) else {
            if let Some(near) = closest(&file, &imported) {
                return fail(
                    g,
                    n.imported.loc,
                    format!("`{imported}` is not a pub fn of `{source}`; did you mean `{near}`?"),
                );
            }
            return fail(
                g,
                n.imported.loc,
                format!("`{imported}` is not a pub fn of `{source}`"),
            );
        };
        let signature = signature(g, &alias, &file, &function, n.imported.loc)?;
        app_state::with_module(|state| {
            state.rust_functions.push(Function {
                local: local.clone(),
                ..signature
            })
        });
    }
    Ok(())
}

/// `name` is one of this file's imported Rust types.
pub fn named(_g: &mut Generator<'_>, name: &str) -> Result<Option<Type>> {
    Ok(app_state::with_module(|state| {
        state
            .rust_types
            .iter()
            .find(|entry| entry.local == name)
            .map(|entry| entry.ty)
    }))
}

/// Resolve `name` in the Rust file `source` (a type another project file
/// imported through this file).
pub fn named_in(g: &mut Generator<'_>, source: &str, name: &str) -> Result<Option<Type>> {
    let Some(project) = app_state::project() else {
        return Ok(None);
    };
    let Some(module) = project.find_rust(source) else {
        return Ok(None);
    };
    let module_name = super::imports::module_name(&module.name);
    let file = module.file.clone();
    resolve_type(g, &format!("crate::{module_name}"), &file, name)
}
