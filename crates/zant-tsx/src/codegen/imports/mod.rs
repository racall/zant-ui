//! Imports between files of one TSX project (`import { TodoRow, type Todo }
//! from "./TodoRow"`).
//!
//! A project compiles one Rust module per file into one output directory, and
//! the generated entry declares every module at the crate root, so an import
//! becomes a Rust path:
//!
//! ```ignore
//! import { Row, type Todo } from "./components/Row";
//! // => crate::components_row::Row::build(..) / crate::components_row::Todo
//! ```
//!
//! The project's files are kept by [`super::app_state`] (see its docs for why
//! the project is a thread-local instead of a `Generator` field).
//!
//! Imported names resolve against the declaring file's declarations; the
//! `named` / `external_name` hooks installed by `generate::install` let the
//! type machinery resolve `Todo` to `crate::row::Todo` without redeclaring it
//! here.

use crate::ast;

use super::app_state::{self, ModuleState};

use super::components;
use super::functions;
use super::model;
use super::rust_imports;
use super::rust_text;
use super::stateful;
use super::stores;
use super::types::{self, Type};
use super::windows;
use super::{Generator, Result, fail};

/// A parsed file of the project, named by its file stem (`TodoRow`); the
/// parsed modules live in [`app_state::ProjectData`].
pub use super::app_state::ModuleFile as NamedModule;

mod items;
mod resolve;
#[cfg(test)]
mod tests;

use self::items::*;
pub use self::resolve::*;

/// The declarations of another file and the Rust path it is imported under;
/// its imports, for the types its declarations use from elsewhere.
#[derive(Clone, Debug, Default)]
pub struct Scope {
    /// The file's `interface` / `type` declarations.
    pub decls: Vec<ast::TypeDecl>,
    pub alias: String,
    /// The file's imports (types its declarations reference).
    pub imports: Vec<ast::Import>,
    /// A native module spec: its relative imports name other specs.
    pub native: bool,
}

/// A type imported by name into this file.
#[derive(Clone, Debug)]
pub struct ImportedType {
    pub local: String,
    pub imported: String,
    /// Index into [`ModuleState::scopes`].
    pub scope: usize,
}

pub fn is_relative(source: &str) -> bool {
    source.starts_with("./") || source.starts_with("../")
}

/// `./components/TodoRow.tsx` → `TodoRow` (also for `.rs` files).
pub fn stem(source: &str) -> String {
    let base = source.rsplit(['/', '\\']).next().unwrap_or(source);
    let base = base
        .strip_suffix(".tsx")
        .or_else(|| base.strip_suffix(".rs"))
        .unwrap_or(base);
    base.to_owned()
}

/// The Rust module name of a project file: `TodoRow` → `todo_row`. The
/// generated entry declares `#[path = "TodoRow.rs"] pub mod todo_row;`.
pub fn module_name(stem: &str) -> String {
    let mut out = String::new();
    for (index, ch) in stem.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

/// The project file a relative import names (`./components/TodoRow` →
/// `TodoRow`).
pub fn find_module<'a>(
    project: &'a app_state::ProjectData,
    source: &str,
) -> Option<&'a app_state::ModuleFile> {
    project.find_module(source)
}

/// Handles the relative imports of `module`: registers imported types and
/// components in the module state. The Rust module paths need no header lines
/// (the entry declares every module), so nothing is appended to
/// `Generator::header`.
pub fn process(g: &mut Generator<'_>, module: &ast::Module) -> Result<()> {
    let project = app_state::project();
    for import in &module.imports {
        let source = String::from_utf8_lossy(&import.source).into_owned();
        // native module specs: the trait lives in the generated
        // `native/spec.rs`, the app implements it in `native/<name>.rs`.
        if source.starts_with("native/") {
            process_native(g, import, &source)?;
            continue;
        }
        if !is_relative(&source) {
            continue;
        }
        if crate::assets::is_asset(&source) {
            continue; // `globals/assets` handles the default import
        }
        if source.ends_with(".rs") {
            rust_imports::process(g, import, &source)?;
            continue;
        }
        let Some(project) = project.as_deref() else {
            return fail(
                g,
                import.source_loc,
                format!("cannot find `{source}` among the project's .tsx files"),
            );
        };
        let Some(target) = project.find_module(&source) else {
            return fail(
                g,
                import.source_loc,
                format!("cannot find `{source}` among the project's .tsx files"),
            );
        };
        let name = stem(&source);
        let alias = format!("crate::{}", module_name(&name));
        let scope = push_scope(
            g,
            Scope {
                decls: target.module.types.clone(),
                alias: alias.clone(),
                imports: target.module.imports.clone(),
                native: false,
            },
        );
        for n in &import.names {
            let imported = String::from_utf8_lossy(&n.imported.name).into_owned();
            let local = String::from_utf8_lossy(&n.local.name).into_owned();
            if exported_type(&target.module, &imported) {
                app_state::with_module(|state| {
                    state.imported_types.push(ImportedType {
                        local,
                        imported,
                        scope,
                    });
                });
            } else if let Some(function) = exported_function(&target.module, &imported) {
                if functions::is_function(function) {
                    import_function(g, &local, &imported, function, scope, &target.name)?;
                } else {
                    let keeps_state =
                        stateful::stateful(&project.module_sources(), &target.module, function);
                    import_component(g, &local, &imported, function, scope, keeps_state)?;
                }
            } else if let Some(store) = stores::exported(&target.module, &imported) {
                import_store(g, &local, &imported, &store, scope, n.imported.loc)?;
            } else if windows::exported(&target.module, &imported).is_some() {
                windows::import(g, &local, &imported, &alias, n.imported.loc)?;
            } else if let Some(constant) = exported_constant(&target.module, &imported) {
                import_constant(g, &local, &imported, &constant, scope, n.imported.loc)?;
            } else {
                return fail(
                    g,
                    n.imported.loc,
                    format!("`{imported}` is not exported by `{source}`"),
                );
            }
        }
    }
    Ok(())
}

/// `import { list, type Clip } from "native/clips"`: types resolve to
/// the generated `crate::native::spec::<name>` modules, functions to the trait
/// implemented by `crate::native::<name>::Native`.
fn process_native(g: &mut Generator<'_>, import: &ast::Import, source: &str) -> Result<()> {
    let Some(project) = app_state::project() else {
        return fail(
            g,
            import.source_loc,
            format!("cannot find `{source}`: no project"),
        );
    };
    let specs: Vec<crate::native_specs::Spec<'_>> = project
        .modules
        .iter()
        .filter(|module| module.native)
        .map(|module| crate::native_specs::Spec {
            name: module.name.as_str(),
            module: &module.module,
        })
        .collect();
    let spec = match crate::native_specs::spec_for(&specs, source, import.source_loc) {
        Ok(spec) => *spec,
        Err(diag) => {
            if g.diag.is_none() {
                *g.diag = Some(diag);
            }
            return Err(crate::codegen::Error::Tsx);
        }
    };
    let spec_name = spec.name.to_owned();
    let module_ident = crate::native_specs::module_ident(&spec_name);
    let scope = push_scope(
        g,
        Scope {
            decls: spec.module.types.clone(),
            alias: crate::native_specs::app_spec_path(&spec_name),
            imports: spec.module.imports.clone(),
            native: true,
        },
    );
    for n in &import.names {
        let imported = String::from_utf8_lossy(&n.imported.name).into_owned();
        let local = String::from_utf8_lossy(&n.local.name).into_owned();
        let type_only = import.type_only || n.type_only;
        match crate::native_specs::import(&spec, n, type_only) {
            Ok(crate::native_specs::Imported::Type(_)) => {
                app_state::with_module(|state| {
                    state.imported_types.push(ImportedType {
                        local,
                        imported,
                        scope,
                    });
                });
            }
            Ok(crate::native_specs::Imported::Function(function)) => {
                let rust = format!(
                    "<{} as {}>::{}",
                    crate::native_specs::app_impl_path(&spec_name),
                    crate::native_specs::app_trait_path(&spec_name),
                    rust_text::id_at(g, &function.name, function.loc)?
                );
                let function = function.clone();
                let mut signature =
                    in_scope(g, scope, |g| functions::signature(g, &function, rust, true))?;
                signature.name = local;
                g.functions.push(signature);
            }
            Err(diag) => {
                if g.diag.is_none() {
                    *g.diag = Some(diag);
                }
                return Err(crate::codegen::Error::Tsx);
            }
        }
    }
    let _ = module_ident;
    Ok(())
}

/// Adds a scope to the module state, returning its index.
pub fn push_scope(g: &mut Generator<'_>, scope: Scope) -> usize {
    let _ = g;
    app_state::with_module(|state| {
        state.scopes.push(scope);
        state.scopes.len() - 1
    })
}

/// Keeps the `ModuleState` import used by the `pub use` above from warning.
#[allow(dead_code)]
fn _state(_: &ModuleState) {}
