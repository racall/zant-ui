//! App hooks of the root component (docs/tsx.md, "应用"):
//!
//! ```tsx
//! useCloseRequest(() => { if (dirty) { setAsk(true); return false; } return true; });
//! useQuitRequest(() => true);
//! useKeyDown((e: KeyEvent) => { if (e.key === "Escape") { … return true; } return false; });
//! useClipboardChange(() => …);  useReopen(() => …);  useWake(() => …);  useAppExit(() => …);
//! ```
//!
//! Each hook becomes a private method of the root struct and an
//! `impl ui::App` method that forwards to it (`on_close_request`,
//! `on_quit_request`, `on_key`, `on_app_event`, `on_wake`, `on_exit`). The
//! component of a `createWindow` gets `_window_on_*` methods instead, which
//! [`super::windows`] forwards to through `tsx::window::Component`
//! (`useCloseRequest`, `useKeyDown`, `useWindowFocus` only). [`collect`]
//! rejects a hook in a component whose trait would never call it.
//!
//! This module also owns the statement / declaration dispatcher hooks:
//! `generate::install` installs [`statement`] and [`declaration`], which route
//! app hooks, desktop hooks and the state hooks (`useEffect`, `useMemo`,
//! `useStore`) to their implementations.

use crate::ast;

use super::desktop_hooks;
use super::hooks;
use super::model;
use super::scope;
use super::stores;
use super::types::Type;
use super::{Generator, Result, fail};

mod calls;
mod clipboard;
mod table;

pub use self::calls::*;
pub use self::clipboard::*;
pub use self::table::*;

/// `KeyEvent` (`tsx.KeyEvent`): what `useKeyDown` passes (the DOM-like
/// `tsx::KeyEvent`, `tsx::app::KeyEvent` in Rust).
pub fn key_event_type(g: &mut Generator<'_>) -> Result<Type> {
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.rust == "tsx::KeyEvent" {
            return Ok(Type::Compound(index as u32));
        }
    }
    let mut fields: Vec<model::Field> = Vec::new();
    fields.push(model::Field {
        name: b"key".to_vec(),
        ty: Type::String,
        optional: false,
        rust: None,
    });
    for name in ["mod", "shift", "alt", "ctrl", "meta", "repeat"] {
        fields.push(model::Field {
            name: name.as_bytes().to_vec(),
            ty: Type::Boolean,
            optional: false,
            // `mod` is a Rust keyword; the runtime field is `r#mod`.
            rust: (name == "mod").then(|| "r#mod".to_owned()),
        });
    }
    model::add(
        g,
        model::Info {
            kind: model::InfoKind::Object,
            rust: "tsx::KeyEvent".to_owned(),
            fields,
            copy: true,
            ..model::Info::default()
        },
    )
}

/// Which kind of component an app hook is called in: only the root and the
/// components of `createWindow` have a trait that forwards the hooks.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Root,
    Window,
    Child,
}

fn role(g: &Generator<'_>) -> Role {
    let Some(index) = g.current_component else {
        return Role::Root;
    };
    if is_window_component(g, &g.components[index].name) {
        Role::Window
    } else {
        Role::Child
    }
}

/// Whether `name` is the component of a `createWindow(name, …)`: anywhere in
/// the project, or in this file for a single-file compile.
pub(crate) fn is_window_component(g: &Generator<'_>, name: &str) -> bool {
    if let Some(project) = super::app_state::project() {
        return project
            .window_components
            .iter()
            .any(|window| window == name);
    }
    g.const_decls.iter().any(|constant| {
        super::windows::is_create(&constant.value)
            && matches!(
                &constant.value.data,
                ast::ExprData::Call(call) if matches!(
                    call.args.first().map(|arg| &arg.data),
                    Some(ast::ExprData::Ident(component)) if component.as_slice() == name.as_bytes()
                )
            )
    })
}

/// The method name of a hook for the component being generated: the root's
/// `impl ui::App` forwards to `_root_*`, a window component's trait to
/// `_window_*`.
fn method_name(g: &Generator<'_>, hook: &Hook) -> String {
    let prefix = if g.current_component.is_none() {
        "_root"
    } else {
        "_window"
    };
    format!("{prefix}_{}", hook.rust)
}

/// The `impl ui::App` methods of the root component (the `app_lifecycle`
/// hook of `generate`).
pub fn lifecycle(g: &mut Generator<'_>) -> Result<String> {
    let mut out = String::new();
    let mut clipboard = false;
    let mut reopen = false;
    for hook in &HOOKS {
        let method = method_name(g, hook);
        if scope::method_index(g, method.as_bytes()).is_none() {
            continue;
        }
        let index = scope::method_index(g, method.as_bytes()).expect("method");
        match hook.rust {
            "on_key" => {
                out.push_str(
                    "fn on_key(&mut self, event: &zant::core::events::KeyEvent) -> bool {\n\
                     let e = tsx::app::key_event(event);\n\
                     Self::_root_on_key(self, e)\n\
                     }\n",
                );
            }
            "on_clipboard" | "on_reopen" => {
                clipboard |= hook.rust == "on_clipboard";
                reopen |= hook.rust == "on_reopen";
                let _ = index;
            }
            "on_focus" => {}
            "on_wake" | "on_exit" | "on_close_request" | "on_quit_request" => {
                let returns = if hook.returns { " -> bool" } else { "" };
                let call = if hook.returns {
                    format!("Self::{method}(self)")
                } else {
                    format!("Self::{method}(self);")
                };
                out.push_str(&format!(
                    "fn {}(&mut self){returns} {{\n{call}\n}}\n",
                    hook.rust
                ));
            }
            _ => {}
        }
    }
    if clipboard || reopen {
        out.push_str("fn on_app_event(&mut self, event: &zant::desktop::AppEvent) -> bool {\n");
        if clipboard {
            out.push_str(
                "if *event == zant::desktop::AppEvent::Clipboard {\nSelf::_root_on_clipboard(self);\n}\n",
            );
        }
        if reopen {
            out.push_str(
                "if *event == zant::desktop::AppEvent::Reopen {\nSelf::_root_on_reopen(self);\n}\n",
            );
        }
        out.push_str("true\n}\n");
    }
    Ok(out)
}
