//! Native module specs: `native/<name>.ts` compiled to a Rust trait.
//!
//! A spec declares the data types and functions `native/<name>.rs` gives the
//! app, like a React Native TurboModule spec:
//!
//! ```ts
//! import type { i64 } from "zant";
//! import type { Kind } from "./kinds";
//! export interface ListEntry { row: i64; kind: Kind; preview: string }
//! export type CopyResult = "ok" | "not_found";
//! export declare function entries(query: string): ListEntry[];
//! export declare function copy(row: i64): CopyResult;
//! export declare function clear(): void;
//! ```
//!
//! The compiler writes one generated file, `native/spec.rs`, holding a module
//! per spec:
//!
//! ```rust,ignore
//! pub mod clips {
//!     use zant::tsx;
//!
//!     #[derive(Clone, PartialEq, Default)]
//!     pub struct ListEntry { pub row: i64, pub kind: super::kinds::Kind, /* .. */ }
//!
//!     pub trait ClipsSpec {
//!         fn entries(query: &str) -> tsx::List<ListEntry>;
//!         // ..
//!     }
//!
//!     pub const NAME: &str = "clips";
//! }
//! ```
//!
//! The app writes `native/clips.rs` as `impl crate::native::spec::clips::ClipSpec
//! for Native`, and a missing or mistyped function is a rustc error at the
//! `impl`: the trait is the compile-time conformance check. The generated
//! entry performs the optional registry registration
//! (`zant::tsx::native::register_named(NAME, Native)`); generated call sites
//! (`<Native as ClipsSpec>::entries(..)`) bypass the registry. See
//! `docs/tsx-native.md`.
//!
//! The application code generator calls: [`module_name`] to recognise
//! `import … from "native/<name>"`, [`spec_for`] / [`find`] to resolve it,
//! [`import`] to classify an imported name (and to get the `native/<name>.ts`
//! diagnostics), and [`spec_root`] to emit the generated `native/spec.rs`. Type
//! resolution of a spec type inside app code stays in the import scope
//! machinery (`codegen::imports`); this module supplies the declarations.
//!
//! A spec's types and functions are emitted together as one trait
//! (`ClipsSpec`) so rustc checks the implementation, and `native/spec.rs` is a
//! single generated file with a Rust module per spec so the app declares one
//! module.

use crate::ast;
use crate::error::Diag;

use crate::codegen::{
    self, Generator, Result, TypeAlias, fail, functions, model, rust_text, types,
};

mod emit;
mod validate;

pub use self::emit::*;
pub use self::validate::*;

/// A parsed spec `native/<name>.ts` of the project.
#[derive(Clone, Copy, Debug)]
pub struct Spec<'a> {
    pub name: &'a str,
    pub module: &'a ast::Module,
}

/// `native/clips` -> `Some("clips")`; `None` for any other import source.
pub fn module_name(source: &str) -> Option<&str> {
    let name = source.strip_prefix("native/")?;
    if name.is_empty() || name.chars().any(|ch| matches!(ch, '/' | '\\' | '.')) {
        return None;
    }
    Some(name)
}

/// `./components/kinds` / `./kinds.ts` -> `kinds`.
fn file_stem(source: &str) -> &str {
    let base = source.rsplit(['/', '\\']).next().unwrap_or(source);
    base.strip_suffix(".ts").unwrap_or(base)
}

/// The spec `name` of the project.
pub fn find<'a>(specs: &'a [Spec<'a>], name: &str) -> Option<&'a Spec<'a>> {
    specs.iter().find(|spec| spec.name == name)
}

/// The spec an import source names, or the "add the spec" diagnostic:
/// `import … from "native/files"` / `import type … from "./kinds"`.
pub fn spec_for<'a>(
    specs: &'a [Spec<'a>],
    source: &str,
    loc: ast::Loc,
) -> std::result::Result<&'a Spec<'a>, Diag> {
    let name = module_name(source).unwrap_or_else(|| file_stem(source));
    find(specs, name).ok_or_else(|| Diag {
        loc,
        msg: format!(
            "cannot find `{source}`: add native/{name}.ts (the spec) and native/{name}.rs"
        ),
    })
}

/// The type `name` the spec exports, if any.
pub fn exported_type<'a>(module: &'a ast::Module, name: &[u8]) -> Option<&'a ast::TypeDecl> {
    module
        .types
        .iter()
        .find(|decl| decl.exported && decl.name.name == name)
}

/// The spec's `export declare function` `name`.
pub fn declared<'a>(module: &'a ast::Module, name: &[u8]) -> Option<&'a ast::Function> {
    module
        .declared
        .iter()
        .find(|top| top.exported && top.func.name == name)
        .map(|top| &*top.func)
}

/// What `import { … } from "native/<name>"` brings into a `.tsx` file.
#[derive(Clone, Copy, Debug)]
pub enum Imported<'a> {
    /// A type the spec exports (it has the same shape in generated Rust).
    Type(&'a ast::TypeDecl),
    /// A declared function: generated code calls it as a trait method.
    Function(&'a ast::Function),
}

/// Classifies one imported name; `Err` is the diagnostic for a name the
/// spec does not export or for a function imported as a type (or vice versa).
pub fn import<'a>(
    spec: &Spec<'a>,
    name: &ast::ImportName,
    type_only: bool,
) -> std::result::Result<Imported<'a>, Diag> {
    if let Some(decl) = exported_type(spec.module, &name.imported.name) {
        return Ok(Imported::Type(decl));
    }
    if let Some(function) = declared(spec.module, &name.imported.name) {
        if type_only {
            return Err(Diag {
                loc: name.imported.loc,
                msg: format!(
                    "`{}` is a function",
                    String::from_utf8_lossy(&name.imported.name)
                ),
            });
        }
        return Ok(Imported::Function(function));
    }
    Err(Diag {
        loc: name.imported.loc,
        msg: format!(
            "native/{}.ts declares no exported `{}`",
            spec.name,
            String::from_utf8_lossy(&name.imported.name)
        ),
    })
}

/// The Rust module of a spec inside the generated `native/spec.rs`:
/// `clips` -> `clips` (escaped like every generated identifier).
pub fn module_ident(name: &str) -> String {
    rust_text::id(name.as_bytes()).unwrap_or_else(|| "_".to_owned())
}

/// The generated trait of a spec: `clips` -> `ClipsSpec`.
pub fn trait_name(name: &str) -> String {
    let mut name = rust_text::enum_variant(name.as_bytes()).unwrap_or_else(|| "_".to_owned());
    name.push_str("Spec");
    name
}

/// The generated module path of a spec: `clips` -> `crate::native::spec::clips`.
pub fn app_spec_path(spec_name: &str) -> String {
    format!("crate::native::spec::{}", module_ident(spec_name))
}

/// The generated trait path of a spec: `crate::native::spec::clips::ClipsSpec`.
pub fn app_trait_path(spec_name: &str) -> String {
    format!("{}::{}", app_spec_path(spec_name), trait_name(spec_name))
}

/// The app's implementation path of a spec: `crate::native::clips::Native`
/// (the `native/<name>.rs` module; call sites name it).
pub fn app_impl_path(spec_name: &str) -> String {
    format!("crate::native::{}::Native", module_ident(spec_name))
}

/// The app's spelling of a spec type in generated code:
/// `crate::native::spec::clips::ListEntry`.
pub fn app_type_path(spec_name: &str, type_name: &str) -> String {
    format!(
        "{}::{}",
        app_spec_path(spec_name),
        rust_text::id(type_name.as_bytes()).unwrap_or_else(|| "_".to_owned())
    )
}
