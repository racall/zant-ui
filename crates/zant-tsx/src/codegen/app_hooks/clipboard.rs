//! Whether the app watches the clipboard (directly or through custom hooks).

use super::*;

/// Whether the root component declares `useClipboardChange` (the generated
/// entry enables the clipboard watcher with `Config::watch_clipboard`).
pub fn watches_clipboard(module: &ast::Module) -> bool {
    module
        .functions
        .iter()
        .any(|top| top.default && body_contains_call(&top.func.body, "useClipboardChange"))
}

/// Whether the root component calls `useClipboardChange` itself or through
/// custom hooks (`useLifecycle()` → `useClipboardChange(…)`), looked up by
/// name in the project's modules.
pub fn watches_clipboard_via_hooks(root: &ast::Module, project: &[ast::Module]) -> bool {
    let mut pending: Vec<String> = root
        .functions
        .iter()
        .filter(|top| top.default)
        .flat_map(|top| body_calls(&top.func.body))
        .collect();
    let mut seen: Vec<String> = Vec::new();
    while let Some(name) = pending.pop() {
        if name == "useClipboardChange" {
            return true;
        }
        if !name.starts_with("use") || seen.contains(&name) {
            continue;
        }
        seen.push(name.clone());
        for module in project {
            for top in &module.functions {
                if top.func.name == name.as_bytes() {
                    pending.extend(body_calls(&top.func.body));
                }
            }
        }
    }
    false
}

/// The names of the calls made as top-level statements / declarations.
pub(super) fn body_calls(body: &ast::Body) -> Vec<String> {
    let ast::Body::Block(statements) = body else {
        return Vec::new();
    };
    statements
        .iter()
        .filter_map(|statement| match &statement.data {
            ast::StmtData::Expr(e) => hooks::call_name(e),
            ast::StmtData::Decl(d) => d.init.as_deref().and_then(hooks::call_name),
            _ => None,
        })
        .map(|name| name.to_string())
        .collect()
}

pub(super) fn body_contains_call(body: &ast::Body, name: &str) -> bool {
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
