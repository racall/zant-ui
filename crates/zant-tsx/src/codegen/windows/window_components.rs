//! Window components: the generated component impl and the window hooks it uses.

use super::*;

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
         // the component's handlers reach it through `_state`\n\
         local.borrow_mut()._state = std::rc::Rc::downgrade(&local);\n\
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
pub(super) struct WindowHooks {
    key: bool,
    focused: bool,
    close: bool,
}

pub(super) fn window_hooks(function: &ast::Function) -> WindowHooks {
    let mut hooks = WindowHooks::default();
    let ast::Body::Block(statements) = &function.body else {
        return hooks;
    };
    for statement in statements {
        let ast::StmtData::Expr(e) = &statement.data else {
            continue;
        };
        let Some(name) = crate::codegen::hooks::call_name(e) else {
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
