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
//! `on_quit_request`, `on_key`, `on_app_event`, `on_wake`, `on_exit`). A
//! no-props child component of a `createWindow` gets `_window_on_*` methods
//! instead, which [`super::windows`] forwards to through
//! `tsx::window::Component`.
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum HookParam {
    None,
    Key,
    Focused,
}

struct Hook {
    name: &'static str,
    /// The `ui::App` / `tsx::window::Component` method the hook forwards to.
    rust: &'static str,
    param: HookParam,
    returns: bool,
    /// App events: the root component only (not windows).
    root_only: bool,
}

const HOOKS: [Hook; 8] = [
    Hook {
        name: "useCloseRequest",
        rust: "on_close_request",
        param: HookParam::None,
        returns: true,
        root_only: false,
    },
    Hook {
        name: "useQuitRequest",
        rust: "on_quit_request",
        param: HookParam::None,
        returns: true,
        root_only: false,
    },
    Hook {
        name: "useKeyDown",
        rust: "on_key",
        param: HookParam::Key,
        returns: true,
        root_only: false,
    },
    Hook {
        name: "useWindowFocus",
        rust: "on_focus",
        param: HookParam::Focused,
        returns: false,
        root_only: false,
    },
    Hook {
        name: "useClipboardChange",
        rust: "on_clipboard",
        param: HookParam::None,
        returns: false,
        root_only: true,
    },
    Hook {
        name: "useReopen",
        rust: "on_reopen",
        param: HookParam::None,
        returns: false,
        root_only: true,
    },
    Hook {
        name: "useWake",
        rust: "on_wake",
        param: HookParam::None,
        returns: false,
        root_only: true,
    },
    Hook {
        name: "useAppExit",
        rust: "on_exit",
        param: HookParam::None,
        returns: false,
        root_only: true,
    },
];

fn find(name: &str) -> Option<&'static Hook> {
    HOOKS.iter().find(|hook| hook.name == name)
}

pub fn is_app_hook(name: &str) -> bool {
    find(name).is_some()
}

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

/// Registers the callback of `useX(…)` (a top-level statement of a component)
/// as a method, before handlers are scanned.
pub fn collect(g: &mut Generator<'_>, e: &ast::Expr) -> Result<()> {
    let ast::ExprData::Call(call) = &e.data else {
        unreachable!("hook call")
    };
    let Some(name) = hooks::call_name(e) else {
        return Ok(());
    };
    let hook = find(&name).expect("app hook");
    if hook.root_only && g.current_component.is_some() {
        return fail(
            g,
            e.loc,
            format!("{name} belongs to the root component (the app); call it there"),
        );
    }
    if let Some(index) = g.current_component
        && let Some(props) = g.components[index].props
        && model::info(g, props).is_some_and(|info| !info.fields.is_empty())
    {
        return fail(
            g,
            e.loc,
            format!(
                "{name} belongs to the root component or a window's component (no props); call it there"
            ),
        );
    }
    if call.args.len() != 1 {
        return fail(g, e.loc, format!("{name} takes one function"));
    }
    let ast::ExprData::Arrow(function) = &call.args[0].data else {
        return fail(g, e.loc, format!("{name} takes one function"));
    };
    if function.params.len() > usize::from(hook.param != HookParam::None) {
        let expected = match hook.param {
            HookParam::None => "no parameters",
            HookParam::Key => "(e: KeyEvent)",
            HookParam::Focused => "(focused: boolean)",
        };
        return fail(
            g,
            function.loc,
            format!("the {name} callback takes {expected}"),
        );
    }
    let method = method_name(g, hook);
    if scope::method_index(g, method.as_bytes()).is_some() {
        return fail(
            g,
            e.loc,
            format!("{name} is used twice, or `{method}` is also a function of this component"),
        );
    }
    let mut function = (**function).clone();
    // The arrow's parameter is untyped (`(e) => …`); the hook supplies it.
    // Annotating it here keeps the event-handler inference (`events`) out of
    // the app layer.
    match hook.param {
        HookParam::Key => annotate_param(g, &mut function, "KeyEvent")?,
        HookParam::Focused => annotate_param(g, &mut function, "boolean")?,
        HookParam::None => {}
    }
    g.methods.push(crate::codegen::Method {
        name: method,
        func: Some(Box::new(function)),
        ret: if hook.returns {
            Type::Boolean
        } else {
            Type::Void
        },
        ..crate::codegen::Method::default()
    });
    Ok(())
}

fn annotate_param(g: &mut Generator<'_>, function: &mut ast::Function, name: &str) -> Result<()> {
    let Some(parameter) = function.params.first_mut() else {
        return Ok(());
    };
    if parameter.ty.is_none() {
        let _ = g;
        parameter.ty = Some(ast::TypeRef {
            name: name.as_bytes().to_vec(),
            loc: parameter.name.loc,
            detail: None,
        });
    }
    Ok(())
}

/// The render-time statement: the method is emitted by the component's
/// inherent impl, so nothing is emitted here.
pub fn statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    top: bool,
) -> Result<String> {
    let Some(name) = hooks::call_name(e) else {
        return fail(g, e.loc, "hook statements are not supported here");
    };
    if desktop_hooks::is_hook(&name) {
        let value = desktop_hooks::value(g, c, e, top)?;
        return Ok(format!("let _ = {};\n", value.code));
    }
    let Some(hook) = find(&name) else {
        if name == "useEffect" {
            return hooks::effect(g, c, e, top);
        }
        return fail(g, e.loc, "keep the value of this hook: const x = …");
    };
    if !c.render || !top {
        return fail(
            g,
            e.loc,
            "hooks must be called at the top level of the component",
        );
    }
    let method = method_name(g, hook);
    if scope::method_index(g, method.as_bytes()).is_none() {
        return fail(g, e.loc, format!("{name} was not collected"));
    }
    // Nothing to emit: the hook's method is a normal method of the component's
    // inherent impl, and `lifecycle` forwards the `impl ui::App` methods to it.
    Ok(String::new())
}

/// Declaration-position hooks (`useHotkey`, `useTray`, `useMemo`, `useStore`).
pub fn declaration(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    d: &ast::Decl,
    top: bool,
) -> Result<String> {
    let Some(init) = d.init.as_deref() else {
        return fail(g, loc, "hook declarations need an initializer");
    };
    let Some(name) = hooks::call_name(init) else {
        return fail(g, loc, "hook declarations are not supported here");
    };
    if desktop_hooks::is_hook(&name) {
        return desktop_hooks::declaration(g, c, loc, d, top);
    }
    match name.as_str() {
        "useMemo" => hooks::memo(g, c, loc, d, top),
        "useStore" => stores::use_store(g, c, loc, d, top),
        _ => fail(g, loc, format!("hook `{name}` cannot be declared here")),
    }
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

/// Whether the root component declares `useClipboardChange` (the generated
/// entry enables the clipboard watcher with `Config::watch_clipboard`).
pub fn watches_clipboard(module: &ast::Module) -> bool {
    module
        .functions
        .iter()
        .any(|top| top.default && body_contains_call(&top.func.body, "useClipboardChange"))
}

fn body_contains_call(body: &ast::Body, name: &str) -> bool {
    let ast::Body::Block(statements) = body else {
        return false;
    };
    statements.iter().any(|statement| match &statement.data {
        ast::StmtData::Expr(e) => hooks::call_name(e).as_deref() == Some(name),
        ast::StmtData::Decl(d) => {
            d.init.as_deref().and_then(hooks::call_name).as_deref() == Some(name)
        }
        _ => false,
    })
}
