//! Rust code generation core: the shared state of the code generator (the
//! [`Generator`]), its diagnostic helper and the extension points through
//! which the JSX, component/state and application modules plug in.
//!
//! The entry point is [`generate`] (module validation, component collection
//! and emission), which builds the generator with [`Generator::new`]. The
//! emitted Rust follows these conventions:
//!
//! - `string` is `zant::Str`, `T[]` is `tsx::List<T>`, `T | null` is
//!   `Option<T>`, `ui::Node` is `ui::Node`.
//! - Every read of a non-`Copy` value appends `.clone()`; the generated file
//!   header allows `clippy::redundant_clone`.
//! - `ui::`/`tsx::`/`zant::Str` paths rely on the file preamble
//!   `use zant::ui; use zant::tsx;`.
//! - Locals are declared with explicit types; unused ones get `let _ = x;`.
//!
//! # Lowering choices
//!
//! - Identifiers that collide with Rust keywords become `keyword_` instead of
//!   raw identifiers, which keeps the usage scanner and golden output simple.
//! - `switch` is a labeled `if`/`else` chain: case tests are arbitrary
//!   expressions with side effects, so a Rust `match` cannot represent their
//!   evaluation order. Fallthrough is a diagnostic.
//! - A C-style `for` becomes `loop { update; if !(cond) { break; } body }`
//!   (update at the top after the first iteration) so `continue` still runs
//!   the update; `for…of` uses `for x in items.iter().cloned()`.
//! - State fields are plain Rust fields (no wrapper type), and
//!   `scope::rename_modules` is a no-op because Rust module paths live in
//!   another namespace.
//! - `interface` structs derive `Clone`/`PartialEq`/`Default`; structs with
//!   `Rc<dyn Fn>` callback fields derive `Clone` only and get a hand-written
//!   `Default` (no-op closure), since `Rc<dyn Fn>` is neither `Default` nor
//!   `PartialEq`.
//! - String literals remain `"...".into()`; where a generic `PartialEq`
//!   context would make that ambiguous (string comparison, `switch` case
//!   tests) the literal is spelled `zant::Str::Static("...")`.
//! - Enum names, numeric casts and `??` become `.as_str()`, typed literals
//!   and `Option` combinators (`unwrap`, `unwrap_or_else`, `map`).
//!
//! # Extension points
//!
//! The modules of the code generator depend on each other cyclically; the
//! borrow rules are simpler when the cross-module calls are function pointers
//! on [`Generator::extensions`]. [`generate`] installs them once before
//! generation:
//!
//! | Module | Hook field | Shape |
//! |---|---|---|
//! | `jsx` | `hooks.emit_jsx` | `fn(&mut Generator, &mut Context, &ast::Jsx) -> Result<Value>` |
//! | `captures` | `hooks.captures_read` | `fn(&mut Generator, &mut Context, &str) -> Result<Option<Value>>` |
//! | `components` (`read_prop`) | `hooks.components_read_prop` | `fn(&mut Generator, &str) -> Result<Option<Value>>` |
//! | `components` (`callback_type`) | `hooks.callback_type` | `fn(&mut Generator, Loc, &ast::FuncType) -> Result<Type>` |
//! | `hooks` | `hooks.ref_read` / `ref_write` / `bare_ref` / `hook_statement` / `hook_declaration` / `cleanup_return` | see [`Hooks`] |
//! | `events` | `hooks.method_param` | `fn(&mut Generator, &Method, usize, &ast::Param) -> Result<Type>` |
//! | `events` (`scan_body` / `emit_setter` / `forward_arrow` / `parameter_type`) | `hooks.scan_body` / `emit_setter` / `forward_arrow` / `parameter_type` | see [`Hooks`] |
//! | `captures` (`nested`) | `hooks.nested` | `fn(..) -> Result<NestedCall>` |
//! | `array_methods` | `extensions.array_methods` | `fn(&mut Generator, &mut Context, Loc, &ast::Call, Option<Type>) -> Result<Value>` |
//! | `components` / `stores` / `windows` (`try_call`) | `extensions.call_before_functions: Vec<TryCall>` | tried in registration order |
//! | `zant_api` (`try_call`) | `extensions.call_before_builtins: Vec<TryCall>` | tried in registration order |
//! | `imports` / `rust_imports` | `hooks.imported_type` / `hooks.rust_type` / `hooks.external_name` | see [`Hooks`] |
//! | `app_hooks` / `jsx` (`image_type`) | `hooks.key_event_type` / `hooks.image_type` | `fn(&mut Generator) -> Result<Type>` |
//! | hook predicates | `extensions.extra_hook` / `is_app_hook` / `is_desktop_hook` / `component_shadow` | `fn(..) -> bool` |
//!
//! `jsx::emit` handles `ui::*` tags, `<List>` and stateful widgets only.
//! Component tags are dispatched by `components`, so the installed `emit_jsx`
//! is a wrapper that checks components first and then calls `jsx::emit`; the
//! scan pass likewise dispatches component tags before calling
//! `events::scan_jsx`.

pub mod action_cx;
pub mod aggregate;
pub mod app_hooks;
pub mod app_state;
pub mod array_methods;
pub mod array_mutations;
pub mod builtins;
pub mod callbacks;
pub mod captures;
pub mod closures;
pub mod components;
pub mod desktop_hooks;
pub mod dicts;
pub mod enums;
pub mod events;
pub mod exceptions;
pub mod expr;
pub mod free_names;
pub mod functions;
pub mod generate;
pub mod generic_functions;
pub mod generic_types;
pub(crate) mod generics;
pub mod globals;
pub mod hooks;
pub mod i18n_calls;
pub mod imports;
pub mod json;
pub mod jsx;
pub mod jsx_children;
pub mod jsx_values;
pub mod list;
pub mod local_functions;
pub mod loops;
pub mod model;
pub mod narrowing;
pub mod nesting;
pub mod place;
pub mod promises;
pub mod regexps;
pub mod rich_text;
pub mod rust_imports;
pub mod rust_text;
pub mod scope;
pub mod spread;
pub mod stateful;
pub mod stmt;
pub mod stores;
pub mod string_methods;
pub mod structural;
pub mod throws;
pub mod truthy;
pub mod type_ops;
pub mod types;
pub mod unions;
pub mod windows;
pub mod zant_api;

use crate::ast;

mod extensions;
mod generator;
mod records;
#[cfg(test)]
mod tests;

pub use self::extensions::*;
pub use self::generator::*;
pub use self::records::*;

pub type Diag = crate::error::Diag;

/// A code generation failure. The message lives in [`Diag`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("tsx code generation error")]
    Tsx,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Record the first diagnostic of this compilation and fail.
pub fn fail<T>(g: &mut Generator<'_>, loc: ast::Loc, msg: impl Into<String>) -> Result<T> {
    if g.diag.is_none() {
        *g.diag = Some(Diag {
            loc,
            msg: msg.into(),
        });
    }
    Err(Error::Tsx)
}

/// Add schema context to the first diagnostic without moving its caret.
pub fn contextualize(g: &mut Generator<'_>, prefix: &str) -> Result<()> {
    let Some(diagnostic) = g.diag.as_ref() else {
        return Err(Error::Tsx);
    };
    *g.diag = Some(Diag {
        loc: diagnostic.loc,
        msg: format!("{prefix}: {}", diagnostic.msg),
    });
    Ok(())
}

/// `useState`, a custom hook, a store hook or an app / desktop hook.
pub fn hook_name(g: &Generator<'_>, n: &str) -> bool {
    if n == "useState" || (g.extensions.extra_hook)(n) || n == "useStore" {
        return true;
    }
    if (g.extensions.is_app_hook)(n) || (g.extensions.is_desktop_hook)(n) {
        return true;
    }
    g.hook_names.iter().any(|name| name == n)
}

/// Whether this expression is a hook call.
pub fn hook(g: &Generator<'_>, e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::Call(call) => match &call.callee.data {
            ast::ExprData::Ident(n) => hook_name(g, &String::from_utf8_lossy(n)),
            _ => false,
        },
        _ => false,
    }
}

/// Formats a generated snippet with `prettyplease`; a snippet that does not
/// parse is returned as written: [`check_and_format`] reports it once for the
/// whole module.
pub fn parse_and_format(source: &str) -> String {
    match syn::parse_file(source) {
        Ok(file) => prettyplease::unparse(&file),
        Err(_) => source.to_owned(),
    }
}

/// Validates a whole generated module with `syn::parse_file` and formats it.
/// A parse failure is a compiler bug, never a user error: the message says so
/// and where the generated code was saved (`ZANT_TSX_DUMP=<dir>`), instead
/// of a panic in the app's build script.
pub fn check_and_format(source: &str, source_name: &str) -> std::result::Result<String, String> {
    let error = match syn::parse_file(source) {
        Ok(file) => return Ok(prettyplease::unparse(&file)),
        Err(error) => error,
    };
    let saved = std::env::var_os("ZANT_TSX_DUMP").and_then(|dir| {
        let base = source_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(source_name);
        let path = std::path::Path::new(&dir).join(format!("{base}.generated.rs"));
        std::fs::create_dir_all(&dir).ok()?;
        std::fs::write(&path, source).ok()?;
        Some(path)
    });
    let dump = match saved {
        Some(path) => format!("the generated code is in {}", path.display()),
        None => "set ZANT_TSX_DUMP=<directory> to save the generated code".to_owned(),
    };
    Err(format!(
        "internal compiler error: the Rust generated for this file does not parse ({error}). \
         This is a zant-tsx bug, not an error in your code; please report it with this file \
         ({dump})"
    ))
}
