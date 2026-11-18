//! Which child components keep local state (phase 3, `docs/tsx.md`).
//!
//! A component that calls a hook (`useState`, `useRef`, …), renders a
//! `<List>` (its scroll state), or renders another stateful component needs an
//! instance id, so its builder takes one (`TodoRow::build(props, id)`) and its
//! call sites pass `tsx::child_id(parent, "TodoRow", key)` (or
//! `tsx::site_id`). Components without state keep the plain
//! `TodoRow::build(props)` and cost nothing extra.
//!
//! Event handlers count as state: a handler compiles to a method of the
//! instance and its node reaches that method through the instance's slot
//! (`_state.upgrade()`), so any JSX attribute that binds a handler — an
//! inline arrow, a function of the file, an imported or native function,
//! `onX={…}` on any tag, or an action prop of a built-in tag — needs the slot.
//! Forwarding the component's own callback prop (`onClick={onSave}`) does not:
//! that value is passed through as is. `generate::action_value` refuses to
//! emit a slot-bound handler for a component classified without one, so a gap
//! here is a compile error instead of a silently dropped event.

use crate::ast;
use crate::tags;

use crate::codegen::hooks;

#[cfg(test)]
mod tests;
mod walker;

pub(crate) use self::walker::*;

/// Nesting limit for components rendering components across files (cycles).
const MAX_DEPTH: u32 = 16;

/// `./components/TodoRow.tsx` → `TodoRow`.
pub fn stem(source: &[u8]) -> Vec<u8> {
    let mut base = source;
    if let Some(index) = base.iter().rposition(|b| *b == b'/' || *b == b'\\') {
        base = &base[index + 1..];
    }
    if let Some(stripped) = base.strip_suffix(b".tsx") {
        base = stripped;
    }
    base.to_vec()
}

/// Whether the project file `source` is the one a relative import names.
fn module_matches(source: &ModuleSource<'_>, import: &[u8]) -> bool {
    source.name.as_bytes() == stem(import).as_slice()
}

/// A parsed file of the project (the `imports` module owns the full
/// representation).
#[derive(Clone, Copy, Debug)]
pub struct ModuleSource<'a> {
    /// The file stem (`TodoRow`), the key a relative import names it by.
    pub name: &'a str,
    pub module: &'a ast::Module,
}

/// Whether `func` of `module` (a file of `project`) keeps local state.
pub fn stateful(project: &[ModuleSource<'_>], module: &ast::Module, func: &ast::Function) -> bool {
    check(project, module, func, 0)
}

fn check(
    project: &[ModuleSource<'_>],
    module: &ast::Module,
    func: &ast::Function,
    depth: u32,
) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    let mut forwarded: Vec<Vec<u8>> = Vec::new();
    for param in &func.params {
        forwarded.push(param.name.name.clone());
        // the local bindings (`{ onSave: save }` binds `save`)
        forwarded.extend(param.destructure.iter().map(|name| name.local.name.clone()));
    }
    let mut walker = Walker {
        project,
        module,
        depth,
        found: false,
        forwarded,
        local_functions: Vec::new(),
    };
    walker.body(&func.body);
    walker.found
}

/// `onClick`, `onChange`, …: the attribute names a handler.
fn event_name(attr: &[u8]) -> bool {
    attr.len() > 2 && attr.starts_with(b"on") && attr[2].is_ascii_uppercase()
}

/// An action prop of a built-in tag (whatever its name).
fn action_prop(tag: Option<&tags::Tag>, attr: &[u8]) -> bool {
    let Some(tag) = tag else {
        return false;
    };
    let attr = String::from_utf8_lossy(attr);
    tags::find_prop(tag, &attr).is_some_and(|prop| {
        matches!(
            prop.kind,
            tags::Kind::Action
                | tags::Kind::TextAction
                | tags::Kind::BoolAction
                | tags::Kind::IndexAction
                | tags::Kind::ShortcutAction
        )
    })
}

/// `ui` builders that keep per-instance state and take a stable id first
/// (`zant-tsx-tags` marks them `instance_id`).
pub fn needs_id(name: &[u8]) -> bool {
    tag_needs_id(&String::from_utf8_lossy(name))
}

/// The string form of [`needs_id`] (the tag table is keyed by `&str`).
pub fn tag_needs_id(name: &str) -> bool {
    tags::find(name).is_some_and(|tag| tag.instance_id)
}
