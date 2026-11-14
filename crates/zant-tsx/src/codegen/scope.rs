//! Function scopes, Rust name clashes, unused-value discards, and handler
//! state snapshots. Translation of `tools/tsx/codegen/scope.zig`.
//!
//! Zig keeps `*Binding` pointers into an arena; Rust keeps bindings in a
//! `Vec<Binding>` and refers to them by index. Declaration placeholders are
//! expanded by [`finish`] exactly like the Zig original: the control-byte
//! markers are replaced after the body has been generated.

use crate::ast;

use super::narrowing;
use super::rust_text;
use super::types::{Type, Value};
use super::{Generator, Result, fail};

pub const PLACEHOLDER_START: char = '\u{1}';
pub const PLACEHOLDER_END: char = '\u{2}';
pub const DISCARD_START: char = '\u{3}';
pub const DISCARD_END: char = '\u{4}';

#[derive(Clone, Debug, Default)]
pub struct Binding {
    pub name: String,
    /// The Rust identifier (`zig` in the Zig original).
    pub zig: String,
    pub ty: Type,
    pub mutable: bool,
    pub assigned: bool,
    /// The initializer, materialized by [`finish`].
    pub init: Option<String>,
    pub serial: usize,
}

#[derive(Clone, Debug)]
pub struct ReturnValue {
    pub value: Value,
    pub loc: ast::Loc,
}

#[derive(Clone, Copy, Debug)]
pub struct Snapshot {
    pub state: usize,
    pub binding: usize,
}

/// A function scope (Zig `scope.Context`).
#[derive(Clone, Debug)]
pub struct Context {
    pub render: bool,
    pub ret: Type,
    pub active: Vec<usize>,
    pub bindings: Vec<Binding>,
    pub params: Vec<usize>,
    pub constant: bool,
    pub return_label: Option<String>,
    /// Nearest statement control targets; callbacks start a fresh control scope.
    pub break_label: Option<String>,
    pub continue_label: Option<String>,
    pub return_values: Option<Vec<ReturnValue>>,
    /// A callback's source context: the visible local names of the enclosing
    /// function (Zig keeps a `*Context`).
    pub capture_source: Option<std::rc::Rc<CaptureSource>>,
    pub capture_method: Option<usize>,
    pub snapshots: Vec<Snapshot>,
    /// Inside an effect with a cleanup / inside a cleanup (`hooks.zig`).
    pub effect_field: Option<String>,
    pub cleanup: bool,
    /// A plain top-level function (`functions.zig`): no component, no `self`.
    pub no_self: bool,
    /// Optionals proven non-null here (`narrowing.zig`).
    pub narrowed: Vec<narrowing::Narrow>,
    /// The runtime context parameter available in this handler (see
    /// [`super::action_cx`]): a local must not shadow it.
    pub cx_var: Option<String>,
}

impl Default for Context {
    fn default() -> Self {
        Context {
            render: false,
            ret: Type::Void,
            active: Vec::new(),
            bindings: Vec::new(),
            params: Vec::new(),
            constant: false,
            return_label: None,
            break_label: None,
            continue_label: None,
            return_values: None,
            capture_source: None,
            capture_method: None,
            snapshots: Vec::new(),
            effect_field: None,
            cleanup: false,
            no_self: false,
            narrowed: Vec::new(),
            cx_var: None,
        }
    }
}

/// The visible names of an enclosing function, for `builtins.shadowed`.
#[derive(Clone, Debug, Default)]
pub struct CaptureSource {
    pub names: Vec<String>,
    pub parent: Option<std::rc::Rc<CaptureSource>>,
}

impl Context {
    /// A snapshot of the names visible to a callback created here.
    pub fn capture_source(&self) -> std::rc::Rc<CaptureSource> {
        std::rc::Rc::new(CaptureSource {
            names: self
                .active
                .iter()
                .map(|index| self.bindings[*index].name.clone())
                .collect(),
            parent: self.capture_source.clone(),
        })
    }
}

fn lossy(name: &[u8]) -> String {
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
    let zig = rust_text::id_at(g, &candidate, 0)?;
    let serial = c.bindings.len();
    c.bindings.push(Binding {
        name: lossy(n),
        zig,
        ty: t,
        mutable,
        assigned: false,
        init: None,
        serial,
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
        if binding.zig.as_bytes() == n {
            return true;
        }
        if rust_text::id(n).is_some_and(|id| binding.zig == id) {
            return true;
        }
    }
    false
}

/// Resolve declaration placeholders and insert only the required discards
/// (Zig `scope.finish`).
pub fn finish(g: &mut Generator<'_>, c: &Context, body: String) -> Result<String> {
    let mut result = body;
    for binding in c.bindings.iter().rev() {
        let Some(init) = binding.init.as_ref() else {
            continue;
        };
        let marker = format!("{PLACEHOLDER_START}{}{PLACEHOLDER_END}", binding.serial);
        let kind = if binding.assigned { "let mut" } else { "let" };
        let ty = binding.ty.rust(g);
        let declaration = format!(
            "{kind} {zig}: {ty} = {init};\n{DISCARD_START}{}{DISCARD_END}",
            binding.serial,
            zig = binding.zig
        );
        result = result.replace(&marker, &declaration);
    }
    // Count identifiers in the surviving output, rather than in visited
    // expressions: a later replacement may have consumed an earlier value.
    let mut usage_body = result.clone();
    for binding in &c.bindings {
        if binding.init.is_some() {
            usage_body = usage_body.replace(
                &format!("{DISCARD_START}{}{DISCARD_END}", binding.serial),
                "",
            );
        }
    }
    let counts = rust_text::count_identifiers(&usage_body);
    for binding in &c.bindings {
        if binding.init.is_none() {
            continue;
        }
        let count = counts.get(&binding.zig).copied().unwrap_or(0);
        let discard = if count == 1 {
            format!("let _ = {};\n", binding.zig)
        } else {
            String::new()
        };
        result = result.replace(
            &format!("{DISCARD_START}{}{DISCARD_END}", binding.serial),
            &discard,
        );
    }
    let mut prefix = String::new();
    for index in &c.params {
        let binding = &c.bindings[*index];
        if counts.get(&binding.zig).copied().unwrap_or(0) == 0 {
            prefix.push_str(&format!("let _ = {};\n", binding.zig));
        }
    }
    for snapshot in &c.snapshots {
        let binding = &c.bindings[snapshot.binding];
        if counts.get(&binding.zig).copied().unwrap_or(0) == 0 {
            continue;
        }
        let state = g.states[snapshot.state].clone();
        let field = state_field(g, state.name.as_bytes())?;
        let init = if state.ty.copy(g) {
            format!("self.{field}")
        } else {
            format!("self.{field}.clone()")
        };
        prefix.push_str(&format!(
            "let {}: {} = {init};\n",
            binding.zig,
            state.ty.rust(g)
        ));
    }
    Ok(format!("{prefix}{result}"))
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
/// methods and functions, so user methods cannot shadow them: the Zig
/// tokenizer-based rename is a no-op here. Kept for the entry point's shape.
pub fn rename_modules(_g: &mut Generator<'_>, raw: String) -> Result<String> {
    Ok(raw)
}

/// The `self.` field of a state or hidden list state (Zig `scope.stateField`).
pub fn state_field(g: &mut Generator<'_>, name: &[u8]) -> Result<String> {
    rust_text::id_at(g, name, 0)
}

/// Handlers read method-entry snapshots (Zig `scope.read`).
pub fn read(g: &mut Generator<'_>, c: &mut Context, n: &[u8], loc: ast::Loc) -> Result<Value> {
    if let Some(value) = narrowing::find(c, n) {
        return Ok(value);
    }
    if let Some(binding) = local(c, n) {
        let ty = binding.ty;
        let code = binding.zig.clone();
        return Ok(owned_read(g, ty, code));
    }
    let name = lossy(n);
    if let Some(hook) = g.extensions.hooks.captures_read
        && let Some(value) = hook(g, c, &name)?
    {
        return Ok(value);
    }
    if let Some(hook) = g.extensions.hooks.components_read_prop
        && let Some(value) = hook(g, &name)?
    {
        if c.cleanup {
            return fail(
                g,
                loc,
                "a cleanup runs after the props are gone; copy what it needs into a local",
            );
        }
        return Ok(value);
    }
    if let Some(index) = state_index(g, n) {
        let state = g.states[index].clone();
        if state.kind == super::StateKind::Ref {
            return match g.extensions.hooks.bare_ref {
                Some(hook) => hook(g, index, loc),
                None => fail(g, loc, "`useRef` reads need the hook support (WP-5.5)"),
            };
        }
        let field = state_field(g, state.name.as_bytes())?;
        if c.render {
            return Ok(Value {
                ty: state.ty,
                code: if state.ty.copy(g) {
                    format!("self.{field}")
                } else {
                    format!("self.{field}.clone()")
                },
                precedence: rust_text::Precedence::Suffix,
                ..Value::default()
            });
        }
        for snapshot in &c.snapshots {
            if snapshot.state == index {
                return Ok(owned_read(
                    g,
                    state.ty,
                    c.bindings[snapshot.binding].zig.clone(),
                ));
            }
        }
        let binding = bind(g, c, n, state.ty, false)?;
        c.snapshots.push(Snapshot {
            state: index,
            binding,
        });
        return Ok(owned_read(g, state.ty, c.bindings[binding].zig.clone()));
    }
    if let Some(constant) = g
        .constants
        .iter()
        .find(|constant| constant.name.as_bytes() == n)
    {
        // the constant's code is the accessor (`name()`, see globals.rs)
        let ty = constant.value.ty;
        let code = constant.value.code.clone();
        return Ok(owned_read(g, ty, code));
    }
    if let Some(hook) = g.extensions.hooks.zant_api_read
        && let Some(value) = hook(g, c, &name, loc)?
    {
        return Ok(value);
    }
    if !c.render
        && let Some(name) = g
            .render_names
            .iter()
            .find(|render_name| render_name.as_bytes() == n)
    {
        return fail(
            g,
            loc,
            format!(
                "component functions cannot capture render locals; use an inline event handler (`{name}`)"
            ),
        );
    }
    fail(
        g,
        loc,
        format!("unknown name `{}`", String::from_utf8_lossy(n)),
    )
}

/// A read of a value: non-`Copy` binds clone (`tsx-codegen.md` §3).
pub fn owned_read(g: &Generator<'_>, ty: Type, code: String) -> Value {
    if ty.copy(g) {
        Value::new(ty, code)
    } else {
        Value {
            ty,
            code: format!("{code}.clone()"),
            precedence: rust_text::Precedence::Suffix,
            ..Value::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{Generator, State, StateKind};

    fn with_generator(f: impl FnOnce(&mut Generator<'_>)) {
        let mut diag = None;
        let mut g = Generator::new(&mut diag);
        f(&mut g);
    }

    #[test]
    fn bind_avoids_shadowing_and_temporary_names() {
        with_generator(|g| {
            let mut c = Context::default();
            let first = bind(g, &mut c, b"x", Type::F64, false).unwrap();
            let second = bind(g, &mut c, b"x", Type::F64, false).unwrap();
            assert_eq!("x", c.bindings[first].zig);
            assert_eq!("x_2", c.bindings[second].zig);
            // reserved names and import module names are never reused
            let third = bind(g, &mut c, b"self", Type::F64, false).unwrap();
            assert_eq!("self_2", c.bindings[third].zig);
        });
    }

    #[test]
    fn finish_expands_declarations_and_discards_unused_locals() {
        with_generator(|g| {
            let mut c = Context::default();
            let used = bind(g, &mut c, b"used", Type::F64, false).unwrap();
            c.bindings[used].init = Some("1.0f64".to_owned());
            let unused = bind(g, &mut c, b"unused", Type::F64, false).unwrap();
            c.bindings[unused].init = Some("2.0f64".to_owned());
            let body = format!(
                "{start}{used}{end}\n{start}{unused}{end}\nlet total: f64 = used;\n",
                start = PLACEHOLDER_START,
                end = PLACEHOLDER_END
            );
            let finished = finish(g, &c, body).unwrap();
            assert!(finished.contains("let used: f64 = 1.0f64;"), "{finished}");
            assert!(finished.contains("let unused: f64 = 2.0f64;"), "{finished}");
            assert!(finished.contains("let _ = unused;"), "{finished}");
            assert!(!finished.contains("let _ = used;"), "{finished}");
        });
    }

    #[test]
    fn finish_discards_unused_parameters() {
        with_generator(|g| {
            let mut c = Context::default();
            let parameter = bind(g, &mut c, b"arg", Type::F64, false).unwrap();
            c.params.push(parameter);
            let finished = finish(g, &c, "return;\n".to_owned()).unwrap();
            assert_eq!("let _ = arg;\nreturn;\n", finished);
        });
    }

    #[test]
    fn state_reads_snapshot_inside_handlers_and_borrow_in_render() {
        with_generator(|g| {
            g.states.push(State {
                name: "count".to_owned(),
                setter: "setCount".to_owned(),
                ty: Type::U32,
                initial: "0u32".to_owned(),
                kind: StateKind::State,
            });
            let mut render = Context {
                render: true,
                ..Context::default()
            };
            assert_eq!(
                "self.count",
                read(g, &mut render, b"count", 0).unwrap().code
            );
            let mut handler = Context::default();
            let value = read(g, &mut handler, b"count", 0).unwrap();
            assert_eq!("count", value.code);
            let finished = finish(g, &handler, "count = count + 1u32;\n".to_owned()).unwrap();
            assert!(
                finished.starts_with("let count: u32 = self.count;\n"),
                "{finished}"
            );
        });
    }

    #[test]
    fn non_copy_reads_clone() {
        with_generator(|g| {
            let mut c = Context::default();
            bind(g, &mut c, b"name", Type::String, false).unwrap();
            assert_eq!("name.clone()", read(g, &mut c, b"name", 0).unwrap().code);
        });
    }

    #[test]
    fn type_index_lookups_match_names() {
        with_generator(|g| {
            g.states.push(State {
                name: "count".to_owned(),
                setter: "setCount".to_owned(),
                ty: Type::U32,
                initial: "0u32".to_owned(),
                kind: StateKind::State,
            });
            assert_eq!(Some(0), state_index(g, b"count"));
            assert_eq!(Some(0), setter_index(g, b"setCount"));
            assert_eq!(None, setter_index(g, b"count"));
        });
    }

    #[test]
    fn rust_module_paths_need_no_renaming() {
        with_generator(|g| {
            let raw = "ui::button(ui::ButtonOptions { ..Default::default() })\n".to_owned();
            assert_eq!(raw, rename_modules(g, raw.clone()).unwrap());
        });
    }
}
