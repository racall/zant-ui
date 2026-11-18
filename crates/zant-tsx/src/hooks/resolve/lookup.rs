//! Finding a custom hook by name in a module or across the project's units.

use super::*;

/// A custom hook: a top-level `function useX` (or `const useX = () => …`).
pub struct Hook {
    pub unit: usize,
    /// The hook's function (owned: expansion copies it out of the unit so
    /// the units can stay immutable while a component is rewritten).
    pub func: ast::Function,
    pub exported: bool,
}

pub fn hook_in(module: &ast::Module, name: &[u8]) -> Option<Hook> {
    for top in &module.functions {
        if top.func.name.as_slice() == name && is_hook_name(name) {
            return Some(Hook {
                unit: 0,
                func: (*top.func).clone(),
                exported: top.exported,
            });
        }
    }
    for c in &module.consts {
        if c.name.name.as_slice() == name
            && is_hook_name(name)
            && let ast::ExprData::Arrow(f) = &c.value.data
        {
            return Some(Hook {
                unit: 0,
                func: (**f).clone(),
                exported: c.exported,
            });
        }
    }
    None
}

/// The hook `name` means in file `unit`: its own, or one it imports from
/// another file. `name@<unit>` names the hook as the file `unit` sees it.
pub fn find_hook_in(units: &[Unit], unit: usize, name: &[u8]) -> Option<Hook> {
    if let Some(at) = name.iter().position(|&b| b == b'@') {
        let from = text(&name[at + 1..]).parse::<usize>().ok()?;
        if from >= units.len() {
            return None;
        }
        return find_hook_in(units, from, &name[..at]);
    }
    if !is_hook_name(name) {
        return None;
    }
    if let Some(hook) = hook_in(&units[unit].module, name) {
        let mut hook = hook;
        hook.unit = unit;
        return Some(hook);
    }
    for imp in &units[unit].module.imports {
        for n in &imp.names {
            if n.local.name.as_slice() != name {
                continue;
            }
            let target = unit_of(units, &imp.source)?;
            let mut hook = hook_in(&units[target].module, &n.imported.name)?;
            hook.unit = target;
            return Some(hook);
        }
    }
    None
}

/// The project file a relative import names (by stem).
pub fn unit_of(units: &[Unit], source: &[u8]) -> Option<usize> {
    if !source.starts_with(b"./") && !source.starts_with(b"../") {
        return None;
    }
    if is_asset(source) || source.ends_with(b".rs") {
        return None;
    }
    let base = basename(source);
    let stem = if base.ends_with(b".tsx") {
        &base[..base.len() - 4]
    } else {
        base
    };
    units.iter().position(|u| u.name.as_slice() == stem)
}

// ── path helpers (POSIX semantics, on byte slices) ──────────────────────
