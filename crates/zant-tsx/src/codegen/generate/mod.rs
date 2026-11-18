//! The code generation entry point, module validation, state collection and
//! component emission.
//!
//! [`generate`] turns one parsed module into a complete Rust source file in
//! this shape:
//!
//! - the root component is a plain struct with its state as fields, a
//!   hand-written `impl Default` (the `useState` initial values), inherent
//!   handler methods and `impl ui::App for App` holding `render`;
//! - a child component is `pub struct Child { props, _id, _state, … }` with
//!   `Child::build(props, id)` (the local state comes from `ui::local_state`;
//!   actions capture the slot through `self._state`, a `Weak` upgrade);
//! - interface / enum declarations, file constants, stores and plain functions
//!   are emitted before the components.
//!
//! The output is validated and formatted by
//! [`crate::codegen::check_and_format`] (`syn` + `prettyplease`); a parse
//! failure is reported as an internal compiler error.
//!
//! The work is split by stage: [`module`] (one file), [`collect`] (the scan
//! before a component), [`component`] (struct, state, builder), [`methods`],
//! [`values`] (actions / callbacks) and [`hot`] (hot-reload state).
//!
//! # Extension points
//!
//! `imports` (module paths), `windows`, `app_hooks` and the JSX / event
//! modules (`jsx`, `events`, `list`, `jsx_values`) are reached through hooks
//! on [`Extensions`], which [`install`] fills in before generation. Native
//! module specs (`native/<name>.ts`) are compiled by [`crate::native_specs`].

use crate::ast;
use crate::tags;

use super::components;
use super::expr;
use super::functions;
use super::globals;
use super::hooks;
use super::model;
use super::rust_text;
use super::scope;
use super::stateful;
use super::stmt;
use super::stores;
use super::types::{self, Type};
use super::{Diag, Generator, Result, fail, parse_and_format};

mod collect;
mod component;
mod hot;
mod methods;
mod module;
#[cfg(test)]
mod tests;
mod values;

pub use self::collect::*;
pub(crate) use self::component::*;
pub(in crate::codegen) use self::hot::*;
pub use self::methods::*;
use self::module::*;
pub use self::values::*;

/// The fixed file header plus the module imports every generated file uses.
///
/// `non_snake_case` is inherent to the generated code: TSX props keep their
/// camelCase names (`onIncrement`) and the handler methods derive from them.
/// Parentheses keep translated expressions unambiguous, callbacks return
/// through labeled blocks, and a custom hook file's imports (assets) are
/// used where the hook is inlined: none of these is the app's code to fix,
/// so an app's `clippy -D warnings` stays about its own code.
pub const HEADER_ALLOWS: &str = "#![allow(clippy::redundant_clone)]\n\
     #![allow(clippy::too_many_arguments)]\n\
     #![allow(non_snake_case, non_upper_case_globals)]\n\
     #![allow(unused_parens, break_with_label_and_loop, dead_code, unreachable_code)]\n";

pub const PREAMBLE: &str = "#[allow(unused_imports)]\nuse zant::nodes;\n#[allow(unused_imports)]\nuse zant::tsx;\n#[allow(unused_imports)]\nuse zant::ui;\n\n";

/// The input of one module compilation.
#[derive(Clone, Copy, Debug)]
pub struct Options<'a> {
    /// The file path for the `@generated` marker and diagnostics.
    pub source_name: &'a str,
    /// The project's parsed files, for relative imports.
    pub project: &'a [stateful::ModuleSource<'a>],
    /// A file without a root component (only child components) is an error
    /// unless false (library files of a project).
    pub require_root: bool,
    /// The file's path in a project build (asset imports).
    pub source_path: Option<&'a str>,
    /// A native module spec (`native/<name>.ts`), compiled by `native_specs`.
    pub native: bool,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Options {
            source_name: "app.tsx",
            project: &[],
            require_root: true,
            source_path: None,
            native: false,
        }
    }
}

thread_local! {
    /// The diagnostic of the last failed [`generate`] (the codegen error type
    /// carries no message; the caller renders this like the parser's `Diag`).
    static LAST_DIAG: std::cell::RefCell<Option<Diag>> = const { std::cell::RefCell::new(None) };
}

/// The first diagnostic of the last [`generate`] that failed.
pub fn last_diag() -> Option<Diag> {
    LAST_DIAG.with(|diag| diag.borrow().clone())
}

thread_local! {
    /// The diagnostics after the first of the last failed [`generate`]: one
    /// per further component that failed.
    static MORE_DIAGS: std::cell::RefCell<Vec<Diag>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Every diagnostic of the last [`generate`] that failed, in source order of
/// the components (the first is [`last_diag`]).
pub fn last_diags() -> Vec<Diag> {
    let mut all: Vec<Diag> = last_diag().into_iter().collect();
    MORE_DIAGS.with(|more| all.extend(more.borrow().iter().cloned()));
    all
}

/// One generated Rust module.
#[derive(Clone, Debug)]
pub struct GeneratedFile {
    /// The validated, pretty-printed source.
    pub source: String,
}

/// `expr` sends every JSX element through this hook: child components first
/// (they emit `Child::build(..)`), then the `ui::*` tags (`jsx::emit`).
pub(crate) fn emit_jsx(
    g: &mut Generator<'_>,
    c: &mut super::scope::Context,
    jsx: &ast::Jsx,
) -> Result<super::types::Value> {
    if let Err(diag) = super::generics::jsx_type_args(jsx) {
        return fail(g, diag.loc, diag.msg);
    }
    let name = String::from_utf8_lossy(&jsx.name).into_owned();
    if let Some(component) = super::components::find(g, &name) {
        let component = component.clone();
        return super::components::emit_call(g, c, jsx, &component);
    }
    super::jsx::emit(g, c, jsx)
}

pub fn install(g: &mut Generator<'_>) {
    // application-layer state of this module (imports, windows, desktop hooks)
    super::app_state::reset();
    g.extensions.extra_hook = hooks::extra_hook;
    g.extensions.hooks.ref_read = Some(hooks::ref_read);
    g.extensions.hooks.ref_write = Some(hooks::ref_write);
    g.extensions.hooks.bare_ref = Some(hooks::bare_ref);
    // this dispatcher chains app hooks, desktop hooks and `useEffect`
    g.extensions.hooks.hook_statement = Some(super::app_hooks::statement);
    g.extensions.hooks.hook_declaration = Some(super::app_hooks::declaration);
    g.extensions.hooks.cleanup_return = Some(hooks::cleanup_return);
    g.extensions.hooks.components_read_prop = Some(components::read_prop);
    g.extensions.hooks.callback_type = Some(components::callback_type);
    g.extensions.hooks.captures_read = Some(super::captures::read);
    g.extensions.hooks.nested = Some(super::captures::nested);
    g.extensions.array_methods = Some(super::array_methods::emit);
    // dispatch order of `expr::call_value`: components, .rs functions,
    // stores, windows
    g.extensions.call_before_functions = vec![
        components::try_call,
        super::rust_imports::try_call,
        stores::try_call,
        super::windows::try_call,
    ];
    // `zant_api` goes before the JS builtins
    g.extensions.call_before_builtins = vec![
        super::zant_api::try_call,
        // `setTimeout` / `clearTimeout`
        super::events::timer_call,
    ];
    g.extensions.is_app_hook = super::app_hooks::is_app_hook;
    g.extensions.is_desktop_hook = super::desktop_hooks::is_hook;
    g.extensions.hooks.zant_api_read = Some(super::zant_api::read);
    g.extensions.hooks.key_event_type = Some(super::app_hooks::key_event_type);
    g.extensions.hooks.image_type = Some(super::jsx::image_type);
    g.extensions.hooks.imported_type = Some(super::imports::named);
    g.extensions.hooks.rust_type = Some(super::rust_imports::named);
    g.extensions.hooks.external_name = Some(super::imports::external_name);
    // imports, windows and the app lifecycle
    g.extensions.process_imports = Some(super::imports::process);
    g.extensions.collect_windows = Some(super::windows::collect);
    g.extensions.app_lifecycle = Some(super::app_hooks::lifecycle);
    g.extensions.is_window_const = Some(super::windows::is_create);
    // JSX and events
    g.extensions.hooks.emit_jsx = Some(emit_jsx);
    g.extensions.hooks.scan_body = Some(super::events::scan_body);
    g.extensions.hooks.scan_expr = Some(super::events::scan_expr);
    g.extensions.hooks.emit_setter = Some(super::events::emit_setter);
    g.extensions.hooks.method_param = Some(super::events::method_param_type);
    g.extensions.hooks.parameter_type = Some(super::events::parameter_type);
}

/// Validate the module, register state and handlers, then emit a complete
/// Rust module.
pub fn generate(
    source_name: &str,
    module: &ast::Module,
    options: &Options<'_>,
) -> Result<GeneratedFile> {
    generate_with(source_name, module, options, None)
}

/// [`generate`] with one extra hook-installation step, run after [`install`].
/// Tests replace individual hooks through it; the production entry point
/// passes `None`.
pub fn generate_with(
    source_name: &str,
    module: &ast::Module,
    options: &Options<'_>,
    install_extra: Option<fn(&mut Generator<'_>)>,
) -> Result<GeneratedFile> {
    let mut diag: Option<Diag> = None;
    MORE_DIAGS.with(|slot| slot.borrow_mut().clear());
    let result = generate_inner(source_name, module, options, install_extra, &mut diag);
    LAST_DIAG.with(|slot| *slot.borrow_mut() = diag);
    result
}
