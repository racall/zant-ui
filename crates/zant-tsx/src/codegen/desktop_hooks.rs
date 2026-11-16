//! `useTray` and `useHotkey` (docs/tsx.md, "托盘与全局快捷键"):
//!
//! ```tsx
//! useTray({ icon: trayIcon, tooltip: "Clips", items: [{ title: "Open", onSelect: () => showMain() }] });
//! const ok = useHotkey(hotkey, () => panel.toggle());   // "ctrl+cmd+v"; "" for none
//! ```
//!
//! Both run while rendering and apply only changes (`zant::tsx::desktop`);
//! their callbacks are handlers (captures, snapshots) registered before the
//! component is emitted.
//!
//! `useTray` becomes `tsx::desktop::update(tsx::desktop::TrayOptions { … })`,
//! `useHotkey` a hidden `tsx::desktop::Hotkey` field updated every render.

use crate::ast;
use crate::tags;

use super::app_state::{self, DesktopCallback};
use super::expr;
use super::generate;
use super::hooks;
use super::model;
use super::rust_imports;
use super::rust_text;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

pub fn is_hook(name: &str) -> bool {
    name == "useTray" || name == "useHotkey"
}

/// Registers the callbacks of `useTray(…)` / `useHotkey(…)` (before handlers
/// are scanned, like event arrows).
pub fn collect(g: &mut Generator<'_>, e: &ast::Expr) -> Result<()> {
    let ast::ExprData::Call(call) = &e.data else {
        return Ok(());
    };
    let Some(name) = hooks::call_name(e) else {
        return Ok(());
    };
    if name == "useHotkey" {
        if call.args.len() != 2 {
            return fail(g, e.loc, "useHotkey(shortcut, () => …)");
        }
        return callback(g, &call.args[1]);
    }
    if call.args.len() != 1 {
        return fail(
            g,
            e.loc,
            "useTray takes its options: useTray({ icon, tooltip, items })",
        );
    }
    let ast::ExprData::Object(properties) = &call.args[0].data else {
        return fail(
            g,
            e.loc,
            "useTray takes its options: useTray({ icon, tooltip, items })",
        );
    };
    for property in properties {
        let key = String::from_utf8_lossy(&property.key.name).into_owned();
        if key == "onClick" {
            callback(g, &property.value)?;
        }
        if key != "items" {
            continue;
        }
        let ast::ExprData::Array(items) = &property.value.data else {
            return fail(g, property.value.loc, "tray items are an array literal");
        };
        for item in items {
            let ast::ExprData::Object(fields) = &item.data else {
                return fail(g, item.loc, "a tray item is an object literal");
            };
            for field in fields {
                if String::from_utf8_lossy(&field.key.name) == "onSelect" {
                    callback(g, &field.value)?;
                }
            }
        }
    }
    Ok(())
}

fn callback(g: &mut Generator<'_>, e: &ast::Expr) -> Result<()> {
    let arrow = match &e.data {
        ast::ExprData::Arrow(_) => e.clone(),
        ast::ExprData::Ident(name) => {
            if let Some(function) = generate::inline_function(g, &String::from_utf8_lossy(name)) {
                ast::Expr {
                    loc: e.loc,
                    data: ast::ExprData::Arrow(Box::new(function)),
                }
            } else if crate::codegen::functions::find_index(g, name).is_some() {
                // A plain function passed by name is called, like
                // `events::function_arrow` does for JSX handlers.
                let call = ast::Expr {
                    loc: e.loc,
                    data: ast::ExprData::Call(ast::Call {
                        callee: Box::new(e.clone()),
                        type_args: Vec::new(),
                        args: Vec::new(),
                        optional: false,
                    }),
                };
                ast::Expr {
                    loc: e.loc,
                    data: ast::ExprData::Arrow(Box::new(ast::Function {
                        type_params: Vec::new(),
                        loc: e.loc,
                        name: Vec::new(),
                        params: Vec::new(),
                        ret: None,
                        body: ast::Body::Expr(Box::new(call)),
                    })),
                }
            } else {
                return fail(
                    g,
                    e.loc,
                    "the callback must be an arrow or the name of a function",
                );
            }
        }
        _ => return fail(g, e.loc, "a callback here is an arrow or a function name"),
    };
    let ast::ExprData::Arrow(function) = &arrow.data else {
        unreachable!("arrow")
    };
    if !function.params.is_empty() {
        return fail(g, arrow.loc, "this callback takes no parameters");
    }
    g.counter += 1;
    let method = g.methods.len();
    app_state::with_module(|state| {
        state.desktop_callbacks.push(DesktopCallback {
            expr: e.clone(),
            method,
        })
    });
    let name = scope::unique_method(g, &format!("_desktop{}", g.counter))?;
    let capture_name = scope::unique_method(g, &format!("_DesktopCapture{}", g.counter))?;
    g.methods.push(crate::codegen::Method {
        name,
        func: Some(function.clone()),
        action: Some(tags::Kind::Action),
        capture_name: Some(capture_name),
        ..crate::codegen::Method::default()
    });
    Ok(())
}

/// The `ui::Action` of a registered callback.
fn action(g: &mut Generator<'_>, c: &mut scope::Context, e: &ast::Expr) -> Result<String> {
    let Some(info) = app_state::with_module(|state| {
        state
            .desktop_callbacks
            .iter()
            .find(|callback| callback.expr == *e)
            .cloned()
    }) else {
        return fail(g, e.loc, "the desktop hook callback was not collected");
    };
    generate::action_value(g, c, info.method, Some(tags::Kind::Action))
}

/// The boolean value of the hook call (registered / tray shown).
pub fn value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    top: bool,
) -> Result<Value> {
    if !c.render || !top {
        return fail(
            g,
            e.loc,
            "hooks must be called at the top level of the component",
        );
    }
    let ast::ExprData::Call(call) = &e.data else {
        return fail(g, e.loc, "hook call expected");
    };
    let Some(name) = hooks::call_name(e) else {
        return fail(g, e.loc, "hook call expected");
    };
    if name == "useHotkey" {
        let shortcut = expr::emit(g, c, &call.args[0], Some(Type::String), false)?;
        g.counter += 1;
        let field = scope::unique_method(g, &format!("_hotkey{}", g.counter))?;
        g.hook_fields.push(hooks::Field {
            name: field.clone(),
            rust_type: "tsx::desktop::Hotkey".to_owned(),
        });
        let slot = scope::state_field(g, field.as_bytes())?;
        let action = action(g, c, &call.args[1])?;
        return Ok(Value::new(
            Type::Boolean,
            format!("self.{slot}.update({}.as_str(), {action})", shortcut.code),
        ));
    }
    let options = tray_options(g, c, &call.args[0])?;
    Ok(Value::new(
        Type::Boolean,
        format!("tsx::desktop::update({options})"),
    ))
}

fn tray_options(g: &mut Generator<'_>, c: &mut scope::Context, e: &ast::Expr) -> Result<String> {
    let ast::ExprData::Object(properties) = &e.data else {
        return fail(
            g,
            e.loc,
            "useTray takes its options: useTray({ icon, tooltip, items })",
        );
    };
    let mut out = String::new();
    let mut icon = false;
    for property in properties {
        let key = String::from_utf8_lossy(&property.key.name).into_owned();
        if property.spread {
            return fail(g, property.value.loc, "list the tray options one by one");
        }
        let code = match key.as_str() {
            "icon" | "iconLight" => {
                icon |= key == "icon";
                let bytes = rust_imports::bytes(g)?;
                expr::emit(g, c, &property.value, Some(bytes), false)?.code
            }
            "template" => expr::emit(g, c, &property.value, Some(Type::Boolean), false)?.code,
            "tooltip" => expr::emit(g, c, &property.value, Some(Type::String), false)?.code,
            "onClick" => action(g, c, &property.value)?,
            "items" => tray_items(g, c, &property.value)?,
            _ => {
                return fail(
                    g,
                    property.key.loc,
                    format!(
                        "unknown tray option `{key}`; options: icon, iconLight, template, tooltip, items, onClick"
                    ),
                );
            }
        };
        let line = match key.as_str() {
            "iconLight" => format!("icon_light: Some({code}),\n"),
            "onClick" => format!("on_click: Some({code}),\n"),
            other => format!("{other}: {code},\n"),
        };
        out.push_str(&line);
    }
    if !icon {
        return fail(
            g,
            e.loc,
            "the tray needs an `icon` (PNG bytes, e.g. an imported .png)",
        );
    }
    Ok(format!(
        "tsx::desktop::TrayOptions {{\n{out}..Default::default()\n}}"
    ))
}

fn tray_items(g: &mut Generator<'_>, c: &mut scope::Context, e: &ast::Expr) -> Result<String> {
    let ast::ExprData::Array(items) = &e.data else {
        return fail(g, e.loc, "tray items are an array literal");
    };
    let mut out = String::from("vec![");
    for item in items {
        let ast::ExprData::Object(properties) = &item.data else {
            return fail(g, item.loc, "a tray item is an object literal");
        };
        let mut fields = String::new();
        for property in properties {
            let key = String::from_utf8_lossy(&property.key.name).into_owned();
            let code = match key.as_str() {
                "title" => expr::emit(g, c, &property.value, Some(Type::String), false)?.code,
                "checked" | "disabled" | "separator" => {
                    expr::emit(g, c, &property.value, Some(Type::Boolean), false)?.code
                }
                "onSelect" => action(g, c, &property.value)?,
                _ => {
                    return fail(
                        g,
                        property.key.loc,
                        format!(
                            "unknown tray item field `{key}`; fields: title, checked, disabled, separator, onSelect"
                        ),
                    );
                }
            };
            let field = match key.as_str() {
                "onSelect" => format!("on_select: Some({code}), "),
                other => format!("{other}: {code}, "),
            };
            fields.push_str(&field);
        }
        out.push_str(&format!(
            "tsx::desktop::TrayItem {{ {fields}..Default::default() }},"
        ));
    }
    out.push(']');
    Ok(out)
}

/// `const x = useHotkey(…)` / `const shown = useTray(…)`: a render local whose
/// initializer is the hook's expression.
pub fn declaration(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    d: &ast::Decl,
    top: bool,
) -> Result<String> {
    let ast::Pattern::Ident(pattern) = &d.pattern else {
        return fail(g, loc, "const ok = useHotkey(…)");
    };
    let Some(init) = d.init.as_deref() else {
        return fail(g, loc, "hook declarations need an initializer");
    };
    let value = value(g, c, init, top)?;
    let name = pattern.name.clone();
    let binding = scope::bind(g, c, &name, value.ty, !d.is_const)?;
    c.bindings[binding].init = Some(value.code);
    Ok(format!(
        "{}{}{}",
        scope::PLACEHOLDER_START,
        binding,
        scope::PLACEHOLDER_END
    ))
}

/// Keeps `types` / `rust_text` imported for the spellings above.
#[allow(dead_code)]
fn _keep(_: types::Value, _: &mut Generator<'_>) {
    let _ = rust_text::id(b"x");
    let _ = model::InfoKind::Object;
}
