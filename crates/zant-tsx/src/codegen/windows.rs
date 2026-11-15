//! Other windows of the app (docs/tsx.md, "多窗口"):
//!
//! ```tsx
//! export const panel = createWindow(Panel, { title: "Clips", style: "panel", width: 380, height: 480 });
//! panel.toggle();  panel.place("trayCorner", 12);  panel.focus("search");
//! ```
//!
//! A window is a file-level `createWindow(Component, { … })` constant; Rust
//! has no mutable static, so it becomes a `thread_local!` slot plus a
//! `pub fn panel_with(..)` accessor that opens the window on first use:
//!
//! ```ignore
//! pub fn panel_with<R>(apply: impl FnOnce(&mut tsx::window::Window<Panel>) -> R) -> R
//! ```
//!
//! A window's component keeps its state while the window exists and its app
//! hooks (`useKeyDown`, `useCloseRequest`, `useWindowFocus`) are the window's;
//! [`collect`] also emits the `impl tsx::window::Component for Panel` block.
//! Other files import the window like a store (`crate::app::panel_with`).

use crate::ast;

use super::app_hooks;
use super::app_state::{self, Window};
use super::components;
use super::expr;
use super::model;
use super::rust_text;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

/// Whether `e` is a `createWindow(…)` call.
pub fn is_create(e: &ast::Expr) -> bool {
    matches!(
        &e.data,
        ast::ExprData::Call(call)
            if matches!(&call.callee.data, ast::ExprData::Ident(name) if name == b"createWindow")
    )
}

struct WindowOption {
    ts: &'static str,
    rust: &'static str,
    kind: OptionKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OptionKind {
    Number,
    Boolean,
    Color,
    Style,
}

/// `createWindow` options → `zant::desktop::WindowOptions` fields (`title` is
/// apart).
const OPTIONS: [WindowOption; 11] = [
    WindowOption {
        ts: "style",
        rust: "style",
        kind: OptionKind::Style,
    },
    WindowOption {
        ts: "width",
        rust: "width",
        kind: OptionKind::Number,
    },
    WindowOption {
        ts: "height",
        rust: "height",
        kind: OptionKind::Number,
    },
    WindowOption {
        ts: "minWidth",
        rust: "min_width",
        kind: OptionKind::Number,
    },
    WindowOption {
        ts: "minHeight",
        rust: "min_height",
        kind: OptionKind::Number,
    },
    WindowOption {
        ts: "background",
        rust: "background",
        kind: OptionKind::Color,
    },
    WindowOption {
        ts: "resizable",
        rust: "resizable",
        kind: OptionKind::Boolean,
    },
    WindowOption {
        ts: "alwaysOnTop",
        rust: "always_on_top",
        kind: OptionKind::Boolean,
    },
    WindowOption {
        ts: "movableByBackground",
        rust: "movable_by_background",
        kind: OptionKind::Boolean,
    },
    WindowOption {
        ts: "center",
        rust: "center",
        kind: OptionKind::Boolean,
    },
    WindowOption {
        ts: "titlebarHeight",
        rust: "titlebar_height",
        kind: OptionKind::Number,
    },
];

const STYLES: [(&str, &str); 5] = [
    ("native", "Native"),
    ("hidden-titlebar", "HiddenTitlebar"),
    ("hiddenTitlebar", "HiddenTitlebar"),
    ("borderless", "Borderless"),
    ("panel", "Panel"),
];

const PLACES: [(&str, &str); 6] = [
    ("center", "Center"),
    ("topLeft", "TopLeft"),
    ("topRight", "TopRight"),
    ("bottomLeft", "BottomLeft"),
    ("bottomRight", "BottomRight"),
    ("trayCorner", "TrayCorner"),
];

/// `createWindow(Component, { … })` constants → `thread_local!` slot and
/// accessor function, plus `impl tsx::window::Component` when the component
/// declares window hooks.
pub fn collect(g: &mut Generator<'_>, module: &ast::Module) -> Result<String> {
    let mut code = String::new();
    for constant in &module.consts {
        if !is_create(&constant.value) {
            continue;
        }
        let ast::ExprData::Call(call) = &constant.value.data else {
            unreachable!("createWindow call")
        };
        if call.args.is_empty() || call.args.len() > 2 {
            return fail(
                g,
                constant.value.loc,
                "createWindow takes a component and options: createWindow(Panel, { title: \"…\", width: 380 })",
            );
        }
        let ast::ExprData::Ident(component_name) = &call.args[0].data else {
            return fail(
                g,
                constant.value.loc,
                "createWindow takes a component and options: createWindow(Panel, { title: \"…\", width: 380 })",
            );
        };
        let name = String::from_utf8_lossy(&constant.name.name).into_owned();
        if scope::global_name(g, constant.name.name.as_slice()) || global_window(g, &name) {
            return fail(
                g,
                constant.name.loc,
                format!("duplicate or reserved name `{name}`"),
            );
        }
        let component_text = String::from_utf8_lossy(component_name).into_owned();
        let Some(component) = components::find(g, &component_text).cloned() else {
            return fail(
                g,
                call.args[0].loc,
                format!("`{component_text}` is not a component of this file or imported"),
            );
        };
        if let Some(props) = component.props
            && let Some(info) = model::info(g, props)
        {
            for field in &info.fields {
                if !field.optional {
                    return fail(
                        g,
                        call.args[0].loc,
                        format!(
                            "a window's component takes no props (`{}` needs `{}`)",
                            component.name,
                            String::from_utf8_lossy(&field.name)
                        ),
                    );
                }
            }
        }
        let struct_name = component
            .builder
            .strip_suffix("::build")
            .unwrap_or(&component.builder)
            .to_owned();
        let window_name = rust_text::quote_at(g, &constant.name.name, constant.name.loc)?;
        let options = if call.args.len() == 2 {
            options_value(g, &call.args[1], &window_name)?
        } else {
            default_options(&window_name)
        };
        let rust_name = rust_text::id_at(g, &constant.name.name, constant.name.loc)?;
        let accessor = format!("{rust_name}_with");
        let slot = rust_name.to_ascii_uppercase();
        // every window component renders through a sibling wrapper that holds
        // the window's `Rc<RefCell<Panel>>` (see `component_impl`). In a
        // project the wrapper lives in the component's own module (it may be
        // imported here), so `generate` emits it from the project-wide
        // `window_components` list; a single-file `compile` has no project and
        // emits it here.
        let project_lists = app_state::project().is_some_and(|project| {
            project
                .window_components
                .iter()
                .any(|window| *window == String::from_utf8_lossy(component.name.as_bytes()))
                || project
                    .window_components
                    .iter()
                    .any(|window| struct_name.ends_with(&format!("::{window}")))
        });
        if !component.external && !project_lists {
            let wrapper = format!("{struct_name}Window");
            code.push_str(&component_impl(
                g,
                &struct_name,
                &wrapper,
                component.func.as_ref(),
            )?);
        }
        let window_type = format!("{struct_name}Window");
        code.push_str(&format!(
            "thread_local! {{\n    static {slot}: std::cell::RefCell<Option<tsx::window::Window<{window_type}>>> = const {{ std::cell::RefCell::new(None) }};\n}}\n\n\
             pub fn {accessor}<R>(apply: impl FnOnce(&mut tsx::window::Window<{window_type}>) -> R) -> R {{\n\
             {slot}.with(|slot| {{\n\
             let mut slot = slot.borrow_mut();\n\
             let window = slot.get_or_insert_with(|| tsx::window::Window::new({options}));\n\
             apply(window)\n\
             }})\n\
             }}\n\n"
        ));
        app_state::with_module(|state| {
            state.windows.push(Window {
                name,
                rust: accessor,
            });
        });
        let global = rust_text::id_at(g, constant.name.name.as_slice(), constant.name.loc)?;
        g.extra_globals.push(global);
    }
    Ok(code)
}

fn global_window(g: &Generator<'_>, name: &str) -> bool {
    app_state::with_module(|state| state.windows.iter().any(|window| window.name == name))
        || g.extra_globals.iter().any(|global| global == name)
}

fn default_options(name: &str) -> String {
    format!(
        "zant::desktop::OpenOptions {{ name: Some({name}.into()), title: \"\".into(), ..Default::default() }}"
    )
}

fn options_value(g: &mut Generator<'_>, e: &ast::Expr, window_name: &str) -> Result<String> {
    let ast::ExprData::Object(properties) = &e.data else {
        return fail(g, e.loc, "window options are an object literal");
    };
    let mut c = scope::Context {
        render: true,
        ret: Type::Void,
        constant: true,
        ..scope::Context::default()
    };
    let mut title = "\"\"".to_owned();
    let mut fields = String::new();
    for property in properties {
        if property.spread {
            return fail(g, property.value.loc, "list the window options one by one");
        }
        let name = String::from_utf8_lossy(&property.key.name).into_owned();
        if name == "title" {
            let ast::ExprData::String(value) = &property.value.data else {
                return fail(g, property.value.loc, "the title is a string literal");
            };
            title = rust_text::quote_at(g, value, property.value.loc)?;
            continue;
        }
        let Some(option) = OPTIONS.iter().find(|option| option.ts == name) else {
            return fail(
                g,
                property.key.loc,
                format!(
                    "unknown window option `{name}`; options: title, {}",
                    option_names()
                ),
            );
        };
        let code = match option.kind {
            OptionKind::Number => {
                expr::emit(g, &mut c, &property.value, Some(Type::F32), false)?.code
            }
            OptionKind::Boolean => {
                expr::emit(g, &mut c, &property.value, Some(Type::Boolean), false)?.code
            }
            OptionKind::Color => rust_text::color(g, &mut c, &property.value)?,
            OptionKind::Style => style_literal(g, &property.value)?,
        };
        fields.push_str(&format!("{}: {code}, ", option.rust));
    }
    Ok(format!(
        "zant::desktop::OpenOptions {{\n    name: Some({window_name}.into()),\n    title: {title}.into(),\n    window: zant::desktop::WindowOptions {{\n        {fields}..Default::default()\n    }},\n    ..Default::default()\n}}"
    ))
}

fn option_names() -> String {
    OPTIONS
        .iter()
        .map(|option| option.ts)
        .collect::<Vec<_>>()
        .join(", ")
}

fn style_literal(g: &mut Generator<'_>, e: &ast::Expr) -> Result<String> {
    if let ast::ExprData::String(value) = &e.data {
        let text = String::from_utf8_lossy(value).into_owned();
        if let Some((_, variant)) = STYLES.iter().find(|(ts, _)| *ts == text) {
            return Ok(format!("zant::desktop::WindowStyle::{variant}"));
        }
    }
    let names = STYLES
        .iter()
        .map(|(ts, _)| *ts)
        .collect::<Vec<_>>()
        .join(", ");
    fail(g, e.loc, format!("style is one of: {names}"))
}

fn place_literal(g: &mut Generator<'_>, e: &ast::Expr) -> Result<String> {
    if let ast::ExprData::String(value) = &e.data {
        let text = String::from_utf8_lossy(value).into_owned();
        if let Some((_, variant)) = PLACES.iter().find(|(ts, _)| *ts == text) {
            return Ok(format!("zant::desktop::Place::{variant}"));
        }
    }
    let names = PLACES
        .iter()
        .map(|(ts, _)| *ts)
        .collect::<Vec<_>>()
        .join(", ");
    fail(g, e.loc, format!("the place is one of: {names}"))
}

/// The `impl tsx::window::Component for PanelWindow` of a window's component:
/// the component's app hooks are the window's hooks.
///
/// A child component keeps its state in flat fields on the struct (not in a
/// separate `Local` type), so the window renders through a small wrapper
/// holding the `Rc<RefCell<Panel>>` the window keeps alive
/// (`tsx::window::Component::Local = Panel`).
pub(crate) fn component_impl(
    g: &mut Generator<'_>,
    struct_name: &str,
    wrapper: &str,
    function: &ast::Function,
) -> Result<String> {
    let hooks = window_hooks(function);
    let mut out = format!(
        "pub struct {wrapper} {{\n_s: std::rc::Rc<std::cell::RefCell<{struct_name}>>,\n}}\n\n\
         impl tsx::window::Component for {wrapper} {{\n\
         type Local = {struct_name};\n\n\
         fn with_local(local: std::rc::Rc<std::cell::RefCell<{struct_name}>>) -> {wrapper} {{\n\
         {wrapper} {{ _s: local }}\n\
         }}\n\n\
         fn render(&mut self) -> ui::Node {{\n\
         self._s.borrow_mut().render()\n\
         }}\n"
    );
    if hooks.key {
        out.push_str(
            "\nfn on_key(&mut self, event: &tsx::KeyEvent) -> bool {\nself._s.borrow_mut()._window_on_key(event.clone())\n}\n",
        );
    }
    if hooks.focused {
        out.push_str(
            "\nfn on_focus(&mut self, focused: bool) {\nself._s.borrow_mut()._window_on_focus(focused);\n}\n",
        );
    }
    if hooks.close {
        out.push_str(
            "\nfn on_close_request(&mut self) -> bool {\nself._s.borrow_mut()._window_on_close_request()\n}\n",
        );
    }
    out.push_str("}\n\n");
    let _ = g;
    Ok(out)
}

#[derive(Default)]
struct WindowHooks {
    key: bool,
    focused: bool,
    close: bool,
}

fn window_hooks(function: &ast::Function) -> WindowHooks {
    let mut hooks = WindowHooks::default();
    let ast::Body::Block(statements) = &function.body else {
        return hooks;
    };
    for statement in statements {
        let ast::StmtData::Expr(e) = &statement.data else {
            continue;
        };
        let Some(name) = super::hooks::call_name(e) else {
            continue;
        };
        if !app_hooks::is_app_hook(&name) {
            continue;
        }
        match name.as_str() {
            "useKeyDown" => hooks.key = true,
            "useWindowFocus" => hooks.focused = true,
            "useCloseRequest" => hooks.close = true,
            _ => {}
        }
    }
    hooks
}

/// The window a file exports as `name`, if any.
pub fn exported(module: &ast::Module, name: &str) -> Option<ast::TopConst> {
    module
        .consts
        .iter()
        .find(|constant| {
            constant.exported
                && is_create(&constant.value)
                && String::from_utf8_lossy(&constant.name.name) == name
        })
        .cloned()
}

/// `import { panel } from "./App"`: the accessor of the declaring module.
pub fn import(
    g: &mut Generator<'_>,
    local: &str,
    imported: &str,
    alias: &str,
    loc: ast::Loc,
) -> Result<()> {
    let accessor = format!(
        "{alias}::{}_with",
        rust_text::id_at(g, imported.as_bytes(), loc)?
    );
    app_state::with_module(|state| {
        state.windows.push(Window {
            name: local.to_owned(),
            rust: accessor,
        })
    });
    Ok(())
}

fn find(_g: &Generator<'_>, c: &scope::Context, e: &ast::Expr) -> Option<Window> {
    let ast::ExprData::Ident(name) = &e.data else {
        return None;
    };
    if scope::local(c, name).is_some() {
        return None;
    }
    app_state::with_module(|state| {
        state
            .windows
            .iter()
            .find(|window| window.name.as_bytes() == name.as_slice())
            .cloned()
    })
}

/// `panel.show()`, `hide`, `activate`, `toggle`, `visible`, `active`,
/// `minimized`, `place`, `focus`.
pub fn try_call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<Value>> {
    let ast::ExprData::Member(member) = &call.callee.data else {
        return Ok(None);
    };
    let Some(window) = find(g, c, &member.object) else {
        return Ok(None);
    };
    let method = String::from_utf8_lossy(&member.name.name).into_owned();
    let begin = format!("{}(", window.rust);
    for name in ["show", "hide", "activate", "toggle"] {
        if method != name {
            continue;
        }
        if !call.args.is_empty() {
            return fail(g, loc, format!("{name}() takes no arguments"));
        }
        return Ok(Some(Value::new(
            Type::Void,
            format!("{begin}|window| window.{method}())"),
        )));
    }
    for name in ["visible", "active", "minimized"] {
        if method != name {
            continue;
        }
        if !call.args.is_empty() {
            return fail(g, loc, format!("{name}() takes no arguments"));
        }
        return Ok(Some(Value::new(
            Type::Boolean,
            format!("{begin}|window| window.{method}())"),
        )));
    }
    if method == "place" {
        if call.args.is_empty() || call.args.len() > 2 {
            return fail(g, loc, "place(where, margin?)");
        }
        let place = place_literal(g, &call.args[0])?;
        let margin = match call.args.get(1) {
            Some(margin) => expr::emit(g, c, margin, Some(Type::F32), false)?.code,
            None => "0.0".to_owned(),
        };
        return Ok(Some(Value::new(
            Type::Void,
            format!("{begin}|window| window.place({place}, {margin}))"),
        )));
    }
    if method == "focus" {
        if call.args.len() != 1 {
            return fail(g, loc, "focus(key) takes the key of a node");
        }
        let key = expr::emit(g, c, &call.args[0], Some(Type::String), false)?;
        return Ok(Some(Value::new(
            Type::Void,
            format!("{begin}|window| window.focus({}.as_str()))", key.code),
        )));
    }
    fail(
        g,
        member.name.loc,
        "a window has show, hide, activate, toggle, visible, active, minimized, place and focus",
    )
}

/// Keeps `types::` imported for the spellings above.
#[allow(dead_code)]
fn _types(_: types::Value) {}
