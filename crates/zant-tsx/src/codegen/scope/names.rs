//! Name lookup: locals, bindings, states, setters, methods, globals and unique names.

use super::*;

pub(super) fn lossy(name: &[u8]) -> String {
    String::from_utf8_lossy(name).into_owned()
}

pub fn local<'c>(c: &'c Context, n: &[u8]) -> Option<&'c Binding> {
    local_index(c, n).map(|index| &c.bindings[index])
}

pub fn local_index(c: &Context, n: &[u8]) -> Option<usize> {
    for index in c.active.iter().rev() {
        if c.bindings[*index].name.as_bytes() == n {
            return Some(*index);
        }
    }
    None
}

/// Allocate a Rust name that cannot shadow imports, methods, parameters, or
/// enclosing locals.
pub fn bind(
    g: &mut Generator<'_>,
    c: &mut Context,
    n: &[u8],
    t: Type,
    mutable: bool,
) -> Result<usize> {
    let mut candidate = n.to_vec();
    let mut suffix = 2usize;
    while name_taken(g, c, &candidate) {
        candidate = format!("{}_{}", lossy(n), suffix).into_bytes();
        suffix += 1;
    }
    let rust = rust_text::id_at(g, &candidate, 0)?;
    let serial = c.bindings.len();
    c.bindings.push(Binding {
        name: lossy(n),
        rust,
        ty: t,
        mutable,
        assigned: false,
        init: None,
        serial,
        fresh: false,
        escaped: false,
        assignments: 0,
    });
    c.active.push(serial);
    Ok(serial)
}

pub fn name_taken(g: &Generator<'_>, c: &Context, n: &[u8]) -> bool {
    let receiver = n == b"self" || (c.capture_method.is_some() && n == b"cap");
    if receiver
        || n == b"_"
        || global_name(g, n)
        || c.cx_var.as_ref().is_some_and(|cx| cx.as_bytes() == n)
    {
        return true;
    }
    for binding in &c.bindings {
        if binding.rust.as_bytes() == n {
            return true;
        }
        if rust_text::id(n).is_some_and(|id| binding.rust == id) {
            return true;
        }
    }
    false
}

pub fn state_index(g: &Generator<'_>, n: &[u8]) -> Option<usize> {
    g.states.iter().position(|state| state.name.as_bytes() == n)
}

pub fn setter_index(g: &Generator<'_>, n: &[u8]) -> Option<usize> {
    g.states
        .iter()
        .position(|state| state.setter.as_bytes() == n)
}

pub fn method_index(g: &Generator<'_>, n: &[u8]) -> Option<usize> {
    g.methods
        .iter()
        .position(|method| method.name.as_bytes() == n)
}

pub fn source_method(g: &Generator<'_>, n: &[u8]) -> Option<usize> {
    g.methods
        .iter()
        .position(|method| method.user && method.name.as_bytes() == n)
}

pub fn global_name(g: &Generator<'_>, n: &[u8]) -> bool {
    for name in &g.module_names {
        if name.as_bytes() == n {
            return true;
        }
    }
    for reserved in ["self", "std", "zant", "ui", "tsx", "render"] {
        if n == reserved.as_bytes() {
            return true;
        }
    }
    for method in &g.methods {
        if let Some(name) = method.capture_name.as_ref()
            && name.as_bytes() == n
        {
            return true;
        }
    }
    for entry in &g.compounds {
        if entry.declaration.is_some() && entry.rust.as_bytes() == n {
            return true;
        }
    }
    for alias in &g.type_aliases {
        if alias.name.as_bytes() == n {
            return true;
        }
    }
    for constant in &g.constants {
        if constant.name.as_bytes() == n {
            return true;
        }
    }
    for name in &g.extra_globals {
        if name.as_bytes() == n {
            return true;
        }
    }
    for function in &g.functions {
        if function.name.as_bytes() == n {
            return true;
        }
    }
    g.name.as_bytes() == n || method_index(g, n).is_some()
}

pub fn unique_method(g: &mut Generator<'_>, base: &str) -> Result<String> {
    let mut n = base.to_owned();
    let mut index = 2usize;
    while global_name(g, n.as_bytes()) || state_index(g, n.as_bytes()).is_some() {
        n = format!("{base}_{index}");
        index += 1;
    }
    Ok(n)
}

pub fn module_names(g: &mut Generator<'_>) -> Result<()> {
    for index in 0..4 {
        let name = g.module_names[index].clone();
        if method_index(g, name.as_bytes()).is_some() {
            let base = format!("_{name}");
            g.module_names[index] = unique_method(g, &base)?;
        }
    }
    Ok(())
}

/// Rust module paths (`ui::`, `tsx::`) live in a different namespace than
/// methods and functions, so user methods cannot shadow them and nothing has
/// to be renamed. Kept for the entry point's shape.
pub fn rename_modules(_g: &mut Generator<'_>, raw: String) -> Result<String> {
    Ok(raw)
}

/// The `self.` field of a state or hidden list state.
pub fn state_field(g: &mut Generator<'_>, name: &[u8]) -> Result<String> {
    rust_text::id_at(g, name, 0)
}
