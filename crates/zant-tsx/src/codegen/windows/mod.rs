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

mod calls;
mod options;
mod window_components;

pub use self::calls::*;
use self::options::*;
pub(crate) use self::window_components::*;

/// Whether `e` is a `createWindow(…)` call.
pub fn is_create(e: &ast::Expr) -> bool {
    matches!(
        &e.data,
        ast::ExprData::Call(call)
            if matches!(&call.callee.data, ast::ExprData::Ident(name) if name == b"createWindow")
    )
}

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

/// Keeps `types::` imported for the spellings above.
#[allow(dead_code)]
fn _types(_: types::Value) {}
