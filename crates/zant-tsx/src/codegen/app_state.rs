//! Build-wide and per-module state of the application layer.
//!
//! The bookkeeping of `imports`, `rust_imports`, `windows`, `desktop_hooks`
//! and `app_hooks` is kept out of the codegen core ([`super::Generator`]),
//! which stays independent of the application layer; `generate::install` is
//! its only per-module entry point. This module keeps that state in
//! thread-local storage:
//!
//! - [`ProjectData`] is set once per project by [`crate::compile`]: the parsed
//!   `.tsx`/`.rs` files, the asset imports and the entry file's name.
//! - [`ModuleState`] is reset by `generate::install` before every module and
//!   holds what one generated file registers while it is emitted (imported
//!   types and functions, windows, desktop hook callbacks, `createWindow`).
//!
//! Code generation is single threaded (one module at a time on the calling
//! thread), so `RefCell` is enough; a nested `generate` call would clobber the
//! state, which the pipeline never does.

use std::cell::RefCell;
use std::rc::Rc;

use crate::assets;
use crate::ast;

use super::imports;
use super::rust_imports;

/// A `.tsx` file of the project, parsed.
pub struct ModuleFile {
    /// The file stem (`TodoRow`), the key a relative import names it by.
    pub name: String,
    /// The path as given to the compiler (`src/components/TodoRow.tsx`).
    pub path: String,
    /// A native module spec (`native/clips.ts`, compiled by `native_specs`).
    pub native: bool,
    /// The source text (diagnostics of asset imports, entry generation).
    pub source: String,
    pub module: ast::Module,
}

/// A hand-written Rust file of the project (`native/*.rs`, `src/format.rs`),
/// parsed with `syn` for its public signatures.
pub struct RustSource {
    pub name: String,
    pub path: String,
    /// `syn::File` is not `Clone`; the source text is kept for diagnostics.
    pub source: String,
    pub file: syn::File,
}

/// Everything the generation of one project needs: the project's modules,
/// the hand-written `.rs` sources and the asset copy list.
pub struct ProjectData {
    /// Parsed `.tsx` files, in input order (native specs have `native` set).
    pub modules: Vec<ModuleFile>,
    /// Parsed `.rs` files, in input order.
    pub rust: Vec<RustSource>,
    /// Asset imports collected while emitting the modules.
    pub assets: RefCell<Vec<assets::Asset>>,
    /// The project root (entry generation and `include_bytes!` paths).
    pub root: String,
    /// The parsed `app.config.json`, if the caller supplied one.
    pub config: Option<crate::compile::ConfigSpec>,
    /// Components named by a `createWindow(<Component>, …)` somewhere in the
    /// project: their own module emits the `Window` wrapper (the window handle
    /// may live in another file, so this cannot be decided per module).
    pub window_components: Vec<String>,
}

impl ProjectData {
    /// The `.tsx` file `source` names (`./components/TodoRow` → `TodoRow`),
    /// `None` for native specs.
    pub fn find_module(&self, source: &str) -> Option<&ModuleFile> {
        if source.ends_with(".rs") {
            return None;
        }
        let name = imports::stem(source);
        self.modules
            .iter()
            .find(|module| !module.native && module.name == name)
    }

    /// The Rust file `source` names (`./format.rs` / `./format` → `format`).
    pub fn find_rust(&self, source: &str) -> Option<&RustSource> {
        let name = imports::stem(source);
        self.rust.iter().find(|rust| rust.name == name)
    }

    /// A parsed `.tsx` file by path.
    pub fn module_by_path(&self, path: &str) -> Option<&ModuleFile> {
        self.modules.iter().find(|module| module.path == path)
    }

    /// The project's parsed files as `stateful::ModuleSource` (`components`
    /// asks whether an imported component keeps local state).
    pub fn module_sources(&self) -> Vec<super::stateful::ModuleSource<'_>> {
        self.modules
            .iter()
            .map(|module| super::stateful::ModuleSource {
                name: &module.name,
                module: &module.module,
            })
            .collect()
    }
}

/// A `createWindow` declaration (see `windows`).
#[derive(Clone, Debug)]
pub struct Window {
    pub name: String,
    /// The Rust accessor: `panel_with` (this file) or
    /// `crate::app::panel_with` (imported).
    pub rust: String,
}

/// A registered `useTray` / `useHotkey` callback (see `desktop_hooks`; the
/// AST has no stable node addresses, so the expression is compared
/// structurally like `hooks::Effect` does).
#[derive(Clone, Debug)]
pub struct DesktopCallback {
    pub expr: ast::Expr,
    pub method: usize,
}

/// The app-layer state of the module being generated.
pub struct ModuleState {
    /// `imports`: types imported by name from project files.
    pub imported_types: Vec<imports::ImportedType>,
    /// `imports`: import scopes of other project files.
    pub scopes: Vec<imports::Scope>,
    /// The scope declarations are currently resolved in.
    pub type_scope: Option<usize>,
    /// `rust_imports`: types imported by name from `.rs` files.
    pub rust_types: Vec<rust_imports::ImportedType>,
    /// `rust_imports`: functions imported from `.rs` files.
    pub rust_functions: Vec<rust_imports::Function>,
    /// `windows`: the `createWindow` handles of this file.
    pub windows: Vec<Window>,
    /// `desktop_hooks`: the callbacks registered by `collect`.
    pub desktop_callbacks: Vec<DesktopCallback>,
    /// `windows`: the handle whose `Window::new` is being emitted (the
    /// source of `createWindow` components' state).
    pub app_config: Option<crate::compile::ConfigSpec>,
}

impl ModuleState {
    const fn new() -> ModuleState {
        ModuleState {
            imported_types: Vec::new(),
            scopes: Vec::new(),
            type_scope: None,
            rust_types: Vec::new(),
            rust_functions: Vec::new(),
            windows: Vec::new(),
            desktop_callbacks: Vec::new(),
            app_config: None,
        }
    }

    fn reset(&mut self) {
        self.imported_types.clear();
        self.scopes.clear();
        self.type_scope = None;
        self.rust_types.clear();
        self.rust_functions.clear();
        self.windows.clear();
        self.desktop_callbacks.clear();
        self.app_config = None;
    }
}

thread_local! {
    static PROJECT: RefCell<Option<Rc<ProjectData>>> = const { RefCell::new(None) };
    static MODULE: RefCell<ModuleState> = const { RefCell::new(ModuleState::new()) };
}

/// Wipes the per-module state (`generate::install` calls this before filling
/// the hooks).
pub fn reset() {
    MODULE.with(|module| module.borrow_mut().reset());
}

/// Runs `f` with the module state (panics only if it is already borrowed,
/// which code generation never nests).
pub fn with_module<R>(f: impl FnOnce(&mut ModuleState) -> R) -> R {
    MODULE.with(|module| f(&mut module.borrow_mut()))
}

/// Sets the project the next `generate` calls resolve imports against.
pub fn set_project(project: Option<Rc<ProjectData>>) -> ProjectGuard {
    PROJECT.with(|slot| *slot.borrow_mut() = project);
    ProjectGuard
}

/// Clears the project when dropped (`compile` uses it to keep test and
/// library calls from leaking a project into the next compilation).
#[must_use]
pub struct ProjectGuard;

impl Drop for ProjectGuard {
    fn drop(&mut self) {
        PROJECT.with(|slot| *slot.borrow_mut() = None);
    }
}

/// Runs `f` with the current project; `None` for a single-file compile.
pub fn with_project<R>(f: impl FnOnce(Option<&ProjectData>) -> R) -> R {
    PROJECT.with(|slot| {
        let project = slot.borrow();
        f(project.as_deref())
    })
}

/// A clone of the current project handle (cheap; used where a `&ProjectData`
/// cannot be held across a mutable borrow of the generator).
pub fn project() -> Option<Rc<ProjectData>> {
    PROJECT.with(|slot| slot.borrow().clone())
}
