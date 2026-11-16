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
                    import_function(g, &local, &imported, function, scope)?;
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

/// A plain exported constant (not a store or a window).
fn exported_constant(module: &ast::Module, name: &str) -> Option<ast::TopConst> {
    module
        .consts
        .iter()
        .find(|constant| {
            constant.exported
                && String::from_utf8_lossy(&constant.name.name) == name
                && !windows::is_create(&constant.value)
                && stores::exported(module, name).is_none()
        })
        .cloned()
}

/// `import { promptMs } from "./AppStore"`: `crate::app_store::promptMs`,
/// typed by its annotation (resolved in the declaring file).
fn import_constant(
    g: &mut Generator<'_>,
    local: &str,
    imported: &str,
    constant: &ast::TopConst,
    scope: usize,
    loc: ast::Loc,
) -> Result<()> {
    let ty = match constant.ty.as_ref() {
        Some(reference) => in_scope(g, scope, |g| types::type_ref(g, reference, false))?,
        // an unannotated literal constant (`export const rowPrefix = "#"`)
        // takes the literal's type
        None => match &constant.value.data {
            ast::ExprData::String(_) => Type::String,
            ast::ExprData::Number(_) => Type::F64,
            ast::ExprData::Boolean(_) => Type::Boolean,
            _ => {
                return fail(
                    g,
                    loc,
                    format!("give `{imported}` a type annotation to import it"),
                );
            }
        },
    };
    // module constants are functions over a thread_local value (globals.rs)
    let code = format!(
        "{}::{}()",
        scope_alias(scope),
        rust_text::id_at(g, imported.as_bytes(), loc)?
    );
    g.constants.push(crate::codegen::Constant {
        name: local.to_owned(),
        value: types::Value {
            ty,
            code,
            ..types::Value::default()
        },
    });
    Ok(())
}

fn exported_type(module: &ast::Module, name: &str) -> bool {
    module
        .types
        .iter()
        .any(|decl| decl.exported && String::from_utf8_lossy(&decl.name.name) == name)
}

fn exported_function<'a>(module: &'a ast::Module, name: &str) -> Option<&'a ast::Function> {
    module
        .functions
        .iter()
        .find(|top| top.exported && String::from_utf8_lossy(&top.func.name) == name)
        .map(|top| &*top.func)
}

/// An imported plain function: its signature resolves in the declaring file.
fn import_function(
    g: &mut Generator<'_>,
    local: &str,
    imported: &str,
    function: &ast::Function,
    scope: usize,
) -> Result<()> {
    let rust = format!(
        "{}::{}",
        scope_alias(scope),
        rust_text::id_at(g, imported.as_bytes(), function.loc)?
    );
    let mut signature = in_scope(g, scope, |g| functions::signature(g, function, rust, true))?;
    signature.name = local.to_owned();
    g.functions.push(signature);
    Ok(())
}

/// An imported component: its props type resolves in the declaring file.
fn import_component(
    g: &mut Generator<'_>,
    local: &str,
    imported: &str,
    function: &ast::Function,
    scope: usize,
    keeps_state: bool,
) -> Result<()> {
    let builder = format!(
        "{}::{}::build",
        scope_alias(scope),
        rust_text::id_at(g, imported.as_bytes(), function.loc)?
    );
    let mut component = components::Component {
        name: local.to_owned(),
        builder,
        func: Box::new(function.clone()),
        props: None,
        param: String::new(),
        destructure: Vec::new(),
        destructure_defaults: Vec::new(),
        external: true,
        stateful: keeps_state,
    };
    if let Some(parameter) = function.params.first() {
        let Some(reference) = parameter.ty.as_ref() else {
            return fail(
                g,
                parameter.name.loc,
                format!("`{imported}` has untyped props"),
            );
        };
        component.props = Some(in_scope(g, scope, |g| {
            types::type_ref(g, reference, false)
        })?);
    }
    if let Some(parameter) = function.params.first() {
        component.param = String::from_utf8_lossy(&parameter.name.name).into_owned();
        component.destructure = parameter.destructure.clone();
        component.destructure_defaults = parameter.destructure_defaults.clone();
    }
    g.components.push(component);
    Ok(())
}

/// `import { prefs } from "./store"`: the accessors live in the declaring
/// module (`crate::store::prefs_read()`).
fn import_store(
    g: &mut Generator<'_>,
    local: &str,
    imported: &str,
    constant: &ast::TopConst,
    scope: usize,
    loc: ast::Loc,
) -> Result<()> {
    let ast::ExprData::Call(call) = &constant.value.data else {
        unreachable!("createStore call")
    };
    if call.type_args.len() != 1 {
        return fail(g, loc, format!("`{imported}` needs a type argument"));
    }
    let ty = in_scope(g, scope, |g| types::type_ref(g, &call.type_args[0], false))?;
    stores::imported(g, local, imported, &scope_alias(scope), ty)
}

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

fn resolve(g: &mut Generator<'_>, scope: usize, name: &str) -> Result<Option<Type>> {
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
            // `native/spec.rs` spells `super::clips::Kind`.
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
                        // itself where the sibling is `super::<name>`
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
        if super::zant_api::exports(name)
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

fn scope_data(index: usize) -> Scope {
    app_state::with_module(|state| state.scopes[index].clone())
}

/// A private helper for tests: registers a project module scope directly.
pub fn push_scope_for_test(g: &mut Generator<'_>, scope: Scope) -> usize {
    push_scope(g, scope)
}

/// Keeps the `ModuleState` import used by the `pub use` above from warning.
#[allow(dead_code)]
fn _state(_: &ModuleState) {}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::codegen::app_state::{ModuleFile, ProjectData, RustSource};
    use crate::parser::Parser;

    fn project(files: &[(&str, &str)]) -> Rc<ProjectData> {
        let modules = files
            .iter()
            .map(|(path, src)| {
                let mut parser = Parser::init(src);
                ModuleFile {
                    name: stem(path),
                    path: (*path).to_owned(),
                    native: false,
                    source: (*src).to_owned(),
                    module: parser.parse_module().expect("test source parses"),
                }
            })
            .collect();
        Rc::new(ProjectData {
            modules,
            rust: Vec::<RustSource>::new(),
            assets: std::cell::RefCell::new(Vec::new()),
            root: String::new(),
            config: None,
            window_components: Vec::new(),
            i18n: None,
        })
    }

    fn parse(src: &str) -> ast::Module {
        let mut parser = Parser::init(src);
        parser.parse_module().expect("test source parses")
    }

    #[test]
    fn module_names_are_snake_case() {
        assert_eq!("todo_row", module_name("TodoRow"));
        assert_eq!("app", module_name("App"));
        assert_eq!("my_file", module_name("my_file"));
    }

    #[test]
    fn process_registers_imported_components_types_stores_and_windows() {
        let project = project(&[
            (
                "src/Row.tsx",
                "export interface Todo { id: u32 }\n\
                 export interface RowProps { todo: Todo }\n\
                 export function Row({ todo }: RowProps) { return <View/>; }\n",
            ),
            (
                "src/store.tsx",
                "export interface Prefs { dark: boolean }\nexport const prefs = createStore<Prefs>({ dark: false });\n",
            ),
            (
                "src/window.tsx",
                "function Panel() { return <View/>; }\nexport const panel = createWindow(Panel, {});\n",
            ),
        ]);
        let module = parse(
            "import { Row, type Todo } from \"./Row\";\n\
             import { prefs } from \"./store\";\n\
             import { panel } from \"./window\";\n\
             export default function App() { return <View/>; }\n",
        );
        let _guard = app_state::set_project(Some(project));
        let mut diag = None;
        let mut g = Generator::new(&mut diag);
        crate::codegen::generate::install(&mut g);
        g.react_imports = module.imports.clone();
        if process(&mut g, &module).is_err() {
            panic!("imports resolve: {:?}", g.diag.clone());
        }
        // the imported component's builder is a crate path
        assert_eq!(1, g.components.len());
        assert_eq!("crate::row::Row::build", g.components[0].builder);
        assert_eq!(
            "crate::row::RowProps",
            g.components[0].props.expect("props type").rust(&g)
        );
        // the imported store keeps its declaring accessors
        assert_eq!(1, g.stores.len());
        assert_eq!("prefs", g.stores[0].declared);
        assert_eq!("crate::store::", g.stores[0].module);
        // the imported window is a handle to the declaring accessor
        let windows = app_state::with_module(|state| state.windows.clone());
        assert_eq!(1, windows.len());
        assert_eq!("crate::window::panel_with", windows[0].rust);
        // the imported type resolves to the other module's declaration
        let imported = app_state::with_module(|state| state.imported_types.clone());
        assert_eq!("Todo", imported[0].imported);
        assert_eq!(
            "crate::row::Todo",
            scope_alias(imported[0].scope) + "::Todo"
        );
    }

    #[test]
    fn an_unknown_import_is_a_diagnostic() {
        let project = project(&[(
            "src/Row.tsx",
            "function Hidden(p: { a: string }) { return <View/>; }\n",
        )]);
        let module = parse(
            "import { Hidden } from \"./Row\";\nexport default function App() { return <View/>; }\n",
        );
        let _guard = app_state::set_project(Some(project));
        let mut diag = None;
        let mut g = Generator::new(&mut diag);
        crate::codegen::generate::install(&mut g);
        let error = process(&mut g, &module).unwrap_err();
        assert_eq!(crate::codegen::Error::Tsx, error);
        assert_eq!(
            "`Hidden` is not exported by `./Row`",
            diag.expect("diagnostic").msg
        );
    }

    #[test]
    fn zant_api_names_pass_the_import_validation() {
        let mut diag = None;
        let mut g = Generator::new(&mut diag);
        assert_eq!(
            Some("clipboard".to_owned()),
            external_name(&mut g, "clipboard").expect("hook")
        );
        assert_eq!(None, external_name(&mut g, "Todo").expect("hook"));
    }
}
