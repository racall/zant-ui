//! Resolving imported type names in the scope of the file that declares them.

use super::*;

/// A type name used in this file (`imported`) or inside another file's
/// declarations (`Generator::type_scope`). Null when neither knows it.
pub fn named(g: &mut Generator<'_>, name: &str) -> Result<Option<Type>> {
    if let Some(scope) = app_state::with_module(|state| state.type_scope) {
        return resolve(g, scope, name);
    }
    let entry = app_state::with_module(|state| {
        state
            .imported_types
            .iter()
            .find(|entry| entry.local == name)
            .cloned()
    });
    let Some(entry) = entry else {
        return Ok(None);
    };
    match resolve(g, entry.scope, &entry.imported)? {
        Some(ty) => Ok(Some(ty)),
        None => fail(
            g,
            0,
            format!(
                "`{}` is not declared in `{}`",
                entry.imported,
                scope_alias(entry.scope)
            ),
        ),
    }
}

pub(super) fn resolve(g: &mut Generator<'_>, scope: usize, name: &str) -> Result<Option<Type>> {
    let Scope {
        decls,
        alias,
        imports,
        native,
    } = scope_data(scope);
    let escaped = rust_text::id_at(g, name.as_bytes(), 0)?;
    let key = format!("{alias}::{escaped}");
    if let Some(alias) = g.type_aliases.iter().find(|alias| alias.name == key) {
        return Ok(Some(alias.ty));
    }
    if let Some(decl) = decls
        .iter()
        .find(|decl| String::from_utf8_lossy(&decl.name.name) == name)
    {
        let declaration = decl.clone();
        let ty = in_scope(g, scope, |g| {
            model::resolve(g, &declaration.ty, Some(&declaration.name.name))
        })?;
        if let Some(alias) = g.type_aliases.iter().find(|alias| alias.name == key) {
            return Ok(Some(alias.ty));
        }
        g.type_aliases
            .push(crate::codegen::TypeAlias { name: key, ty });
        return Ok(Some(ty));
    }
    // a type that file imports (another .tsx's, or a .rs file's)
    for import in &imports {
        for n in &import.names {
            if String::from_utf8_lossy(&n.local.name) != name {
                continue;
            }
            let source = String::from_utf8_lossy(&import.source).into_owned();
            if source.ends_with(".rs") {
                let imported = String::from_utf8_lossy(&n.imported.name).into_owned();
                return rust_imports::named_in(g, &source, &imported);
            }
            // A native spec type (`import type { Kind } from "native/clips"`)
            // used inside this file's own type declarations (an interface
            // field). `process_native` registers the import for plain
            // annotations; a declaration resolved here runs in `in_scope`,
            // where `model::named` goes through this function and the import
            // has to be resolved from the spec itself.
            if let Some(spec_name) = source.strip_prefix("native/") {
                let Some(project) = app_state::project() else {
                    return Ok(None);
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
                let spec = match crate::native_specs::find(&specs, spec_name) {
                    Some(spec) => *spec,
                    None => return Ok(None),
                };
                let imported = String::from_utf8_lossy(&n.imported.name).into_owned();
                let spec_scope = push_scope(
                    g,
                    Scope {
                        decls: spec.module.types.clone(),
                        alias: crate::native_specs::app_spec_path(spec_name),
                        imports: spec.module.imports.clone(),
                        native: true,
                    },
                );
                return resolve(g, spec_scope, &imported);
            }
            // Inside a native spec, a relative import names another spec's
            // type (`import type { Kind } from "./clips"`), which the generated
            // `native/spec.rs` spells `crate::codegen::clips::Kind`.
            if native && is_relative(&source) {
                let Some(project) = app_state::project() else {
                    return Ok(None);
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
                let name = stem(&source);
                let Some(spec) = crate::native_specs::find(&specs, &name).copied() else {
                    return Ok(None);
                };
                let nested = push_scope(
                    g,
                    Scope {
                        decls: spec.module.types.clone(),
                        // app code names the generated spec modules absolutely
                        // (`crate::native::spec::<name>`), unlike `native/spec.rs`
                        // itself where the sibling is `crate::codegen::<name>`
                        alias: crate::native_specs::app_spec_path(&name),
                        imports: spec.module.imports.clone(),
                        native: true,
                    },
                );
                let imported = String::from_utf8_lossy(&n.imported.name).into_owned();
                return resolve(g, nested, &imported);
            }
            if !is_relative(&source) || crate::assets::is_asset(&source) {
                return Ok(None);
            }
            let Some(project) = app_state::project() else {
                return Ok(None);
            };
            let Some(target) = project.find_module(&source) else {
                return Ok(None);
            };
            let nested = push_scope(
                g,
                Scope {
                    decls: target.module.types.clone(),
                    alias: format!("crate::{}", module_name(&stem(&source))),
                    imports: target.module.imports.clone(),
                    native,
                },
            );
            let imported = String::from_utf8_lossy(&n.imported.name).into_owned();
            return resolve(g, nested, &imported);
        }
    }
    Ok(None)
}

/// The Rust path and alias key of a named compound declared in the current
/// type scope (`crate::row::Todo`), or null for this file's own types.
pub fn external_name(g: &mut Generator<'_>, name: &str) -> Result<Option<String>> {
    let Some(scope) = app_state::with_module(|state| state.type_scope) else {
        // `validate_imports` asks this hook for names it does not know; the
        // `"zant"` platform functions (`clipboard`, `app`, …) are imported
        // names, not types, so report them here and never for a declared type.
        if crate::codegen::zant_api::exports(name)
            && !g
                .type_decls
                .iter()
                .any(|decl| decl.name.name.as_slice() == name.as_bytes())
        {
            return Ok(Some(name.to_owned()));
        }
        return Ok(None);
    };
    let _ = g;
    Ok(Some(format!(
        "{}::{}",
        scope_alias(scope),
        rust_text::id(name.as_bytes()).unwrap_or_else(|| "_".to_owned())
    )))
}

/// Runs `f` with the generator's type scope set to the project scope `index`.
pub fn in_scope<R>(
    g: &mut Generator<'_>,
    index: usize,
    f: impl FnOnce(&mut Generator<'_>) -> Result<R>,
) -> Result<R> {
    let previous = app_state::with_module(|state| {
        let previous = state.type_scope;
        state.type_scope = Some(index);
        previous
    });
    let previous_flag = std::mem::replace(&mut g.type_scope, true);
    let result = f(g);
    g.type_scope = previous_flag;
    app_state::with_module(|state| state.type_scope = previous);
    result
}

/// The alias (`crate::row`) of a scope.
pub fn scope_alias(index: usize) -> String {
    app_state::with_module(|state| state.scopes[index].alias.clone())
}

pub(super) fn scope_data(index: usize) -> Scope {
    app_state::with_module(|state| state.scopes[index].clone())
}

/// A private helper for tests: registers a project module scope directly.
pub fn push_scope_for_test(g: &mut Generator<'_>, scope: Scope) -> usize {
    push_scope(g, scope)
}
