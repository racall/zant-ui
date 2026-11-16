//! The code generation entry point, module validation, state collection and
//! component emission.
//!
//! [`generate`] turns one parsed module into a complete Rust source file in
//! this shape:
//!
//! - the root component is a plain struct with its state as fields, a
//!   hand-written `impl Default` (the `useState` initial values), inherent
//!   handler methods and `impl ui::App for App` holding `render`;
//! - a child component is `pub struct Child { props, _id, _state, … }` with
//!   `Child::build(props, id)` (the local state comes from `ui::local_state`;
//!   actions capture the slot through `self._state`, a `Weak` upgrade);
//! - interface / enum declarations, file constants, stores and plain functions
//!   are emitted before the components.
//!
//! The output is validated with [`crate::codegen::parse_and_format`] (`syn` +
//! `prettyplease`); a parse failure panics with the generated source.
//!
//! # Extension points
//!
//! `imports` (module paths), `windows`, `app_hooks` and the JSX / event
//! modules (`jsx`, `events`, `list`, `jsx_values`) are reached through hooks
//! on [`Extensions`], which [`install`] fills in before generation. Native
//! module specs (`native/<name>.ts`) are compiled by [`crate::native_specs`].

use crate::ast;
use crate::tags;

use super::components;
use super::expr;
use super::functions;
use super::globals;
use super::hooks;
use super::model;
use super::rust_text;
use super::scope;
use super::stateful;
use super::stmt;
use super::stores;
use super::types::{self, Type};
use super::{Diag, Generator, Result, fail, parse_and_format};

/// The fixed file header plus the module imports every generated file uses.
///
/// `non_snake_case` is inherent to the generated code: TSX props keep their
/// camelCase names (`onIncrement`) and the handler methods derive from them.
/// Parentheses keep translated expressions unambiguous, callbacks return
/// through labeled blocks, and a custom hook file's imports (assets) are
/// used where the hook is inlined: none of these is the app's code to fix,
/// so an app's `clippy -D warnings` stays about its own code.
pub const HEADER_ALLOWS: &str = "#![allow(clippy::redundant_clone)]\n\
     #![allow(clippy::too_many_arguments)]\n\
     #![allow(non_snake_case)]\n\
     #![allow(unused_parens, break_with_label_and_loop, dead_code)]\n";
pub const PREAMBLE: &str = "#[allow(unused_imports)]\nuse zant::nodes;\n#[allow(unused_imports)]\nuse zant::tsx;\n#[allow(unused_imports)]\nuse zant::ui;\n\n";

/// The input of one module compilation.
#[derive(Clone, Copy, Debug)]
pub struct Options<'a> {
    /// The file path for the `@generated` marker and diagnostics.
    pub source_name: &'a str,
    /// The project's parsed files, for relative imports.
    pub project: &'a [stateful::ModuleSource<'a>],
    /// A file without a root component (only child components) is an error
    /// unless false (library files of a project).
    pub require_root: bool,
    /// The file's path in a project build (asset imports).
    pub source_path: Option<&'a str>,
    /// A native module spec (`native/<name>.ts`), compiled by `native_specs`.
    pub native: bool,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Options {
            source_name: "app.tsx",
            project: &[],
            require_root: true,
            source_path: None,
            native: false,
        }
    }
}

thread_local! {
    /// The diagnostic of the last failed [`generate`] (the codegen error type
    /// carries no message; the caller renders this like the parser's `Diag`).
    static LAST_DIAG: std::cell::RefCell<Option<Diag>> = const { std::cell::RefCell::new(None) };
}

/// The first diagnostic of the last [`generate`] that failed.
pub fn last_diag() -> Option<Diag> {
    LAST_DIAG.with(|diag| diag.borrow().clone())
}

/// One generated Rust module.
#[derive(Clone, Debug)]
pub struct GeneratedFile {
    /// The validated, pretty-printed source.
    pub source: String,
}

/// `expr` sends every JSX element through this hook: child components first
/// (they emit `Child::build(..)`), then the `ui::*` tags (`jsx::emit`).
pub(crate) fn emit_jsx(
    g: &mut Generator<'_>,
    c: &mut super::scope::Context,
    jsx: &ast::Jsx,
) -> Result<super::types::Value> {
    let name = String::from_utf8_lossy(&jsx.name).into_owned();
    if let Some(component) = super::components::find(g, &name) {
        let component = component.clone();
        return super::components::emit_call(g, c, jsx, &component);
    }
    super::jsx::emit(g, c, jsx)
}

pub fn install(g: &mut Generator<'_>) {
    // application-layer state of this module (imports, windows, desktop hooks)
    super::app_state::reset();
    g.extensions.extra_hook = hooks::extra_hook;
    g.extensions.hooks.ref_read = Some(hooks::ref_read);
    g.extensions.hooks.ref_write = Some(hooks::ref_write);
    g.extensions.hooks.bare_ref = Some(hooks::bare_ref);
    // this dispatcher chains app hooks, desktop hooks and `useEffect`
    g.extensions.hooks.hook_statement = Some(super::app_hooks::statement);
    g.extensions.hooks.hook_declaration = Some(super::app_hooks::declaration);
    g.extensions.hooks.cleanup_return = Some(hooks::cleanup_return);
    g.extensions.hooks.components_read_prop = Some(components::read_prop);
    g.extensions.hooks.callback_type = Some(components::callback_type);
    g.extensions.hooks.captures_read = Some(super::captures::read);
    g.extensions.hooks.nested = Some(super::captures::nested);
    g.extensions.array_methods = Some(super::array_methods::emit);
    // dispatch order of `expr::call_value`: components, .rs functions,
    // stores, windows
    g.extensions.call_before_functions = vec![
        components::try_call,
        super::rust_imports::try_call,
        stores::try_call,
        super::windows::try_call,
    ];
    // `zant_api` goes before the JS builtins
    g.extensions.call_before_builtins = vec![
        super::zant_api::try_call,
        // `setTimeout` / `clearTimeout`
        super::events::timer_call,
    ];
    g.extensions.is_app_hook = super::app_hooks::is_app_hook;
    g.extensions.is_desktop_hook = super::desktop_hooks::is_hook;
    g.extensions.hooks.zant_api_read = Some(super::zant_api::read);
    g.extensions.hooks.key_event_type = Some(super::app_hooks::key_event_type);
    g.extensions.hooks.image_type = Some(super::jsx::image_type);
    g.extensions.hooks.imported_type = Some(super::imports::named);
    g.extensions.hooks.rust_type = Some(super::rust_imports::named);
    g.extensions.hooks.external_name = Some(super::imports::external_name);
    // imports, windows and the app lifecycle
    g.extensions.process_imports = Some(super::imports::process);
    g.extensions.collect_windows = Some(super::windows::collect);
    g.extensions.app_lifecycle = Some(super::app_hooks::lifecycle);
    g.extensions.is_window_const = Some(super::windows::is_create);
    // JSX and events
    g.extensions.hooks.emit_jsx = Some(emit_jsx);
    g.extensions.hooks.scan_body = Some(super::events::scan_body);
    g.extensions.hooks.scan_expr = Some(super::events::scan_expr);
    g.extensions.hooks.emit_setter = Some(super::events::emit_setter);
    g.extensions.hooks.method_param = Some(super::events::method_param_type);
    g.extensions.hooks.parameter_type = Some(super::events::parameter_type);
}

/// Validate the module, register state and handlers, then emit a complete
/// Rust module.
pub fn generate(
    source_name: &str,
    module: &ast::Module,
    options: &Options<'_>,
) -> Result<GeneratedFile> {
    generate_with(source_name, module, options, None)
}

/// [`generate`] with one extra hook-installation step, run after [`install`].
/// Tests replace individual hooks through it; the production entry point
/// passes `None`.
pub fn generate_with(
    source_name: &str,
    module: &ast::Module,
    options: &Options<'_>,
    install_extra: Option<fn(&mut Generator<'_>)>,
) -> Result<GeneratedFile> {
    let mut diag: Option<Diag> = None;
    let result = generate_inner(source_name, module, options, install_extra, &mut diag);
    LAST_DIAG.with(|slot| *slot.borrow_mut() = diag);
    result
}

fn generate_inner(
    source_name: &str,
    module: &ast::Module,
    options: &Options<'_>,
    install_extra: Option<fn(&mut Generator<'_>)>,
    diag: &mut Option<Diag>,
) -> Result<GeneratedFile> {
    if options.native {
        // `native/<name>.ts`: the spec compiles to `pub mod <name> { … }` of
        // `native/spec.rs`; the project's specs resolve its imports.
        let name = options
            .source_path
            .map(|path| path.rsplit(['/', '\\']).next().unwrap_or(path))
            .and_then(|base| base.strip_suffix(".ts"))
            .unwrap_or("native");
        // keep the project alive: the specs borrow its parsed modules
        let project = super::app_state::project();
        let specs: Vec<crate::native_specs::Spec<'_>> = project
            .as_deref()
            .map(|project| {
                project
                    .modules
                    .iter()
                    .filter(|module| module.native)
                    .map(|module| crate::native_specs::Spec {
                        name: module.name.as_str(),
                        module: &module.module,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let source = crate::native_specs::generate(module, name, &specs, diag)?;
        return Ok(GeneratedFile { source });
    }
    let mut g = Generator::new(diag);
    g.name.clear();
    g.type_decls = module.types.clone();
    g.react_imports = module.imports.clone();
    g.resolving = vec![false; module.types.len()];
    install(&mut g);
    if let Some(extra) = install_extra {
        extra(&mut g);
    }
    validate_imports(&mut g, module)?;
    if let Some(hook) = g.extensions.process_imports {
        hook(&mut g, module)?;
    }
    for declaration in &module.types {
        let _ = model::named(&mut g, &declaration.name.name)?;
    }
    if let Some(first) = module.declared.first() {
        return fail(
            &mut g,
            first.func.loc,
            "`declare function` belongs in a native module spec (native/<module>.ts)",
        );
    }
    // the root is the default export without props (the app's state); a file of
    // a project may hold child components only
    let mut root: Option<ast::Function> = None;
    for top in &module.functions {
        if !top.default {
            continue;
        }
        if root.is_some() {
            return fail(
                &mut g,
                top.func.loc,
                "only one component can be the default export",
            );
        }
        if !top.func.params.is_empty() && !options.require_root {
            continue;
        }
        root = Some((*top.func).clone());
    }
    if root.is_none() && options.require_root {
        return fail(
            &mut g,
            0,
            "missing default exported component; use export default function App()",
        );
    }
    if let Some(function) = &root {
        if !function.params.is_empty() {
            return fail(
                &mut g,
                function.params[0].name.loc,
                "the root component takes no props; it is the app's state",
            );
        }
        let name = String::from_utf8_lossy(&function.name);
        if reserved_component_name(&name) {
            return fail(
                &mut g,
                function.loc,
                format!("component name `{name}` is reserved; rename it"),
            );
        }
    }
    let is_window = g.extensions.is_window_const.unwrap_or(|_| false);
    let store_code = stores::collect(&mut g, &module.consts)?;
    let plain = stores::plain_constants(&mut g, &module.consts, is_window)?;
    let asset_code = crate::assets::process(&mut g, module, options.source_path)?;
    let globals_code = format!(
        "{asset_code}{}{store_code}",
        globals::collect(&mut g, &plain)?
    );
    functions::collect(&mut g, module)?;
    components::collect(&mut g, module, root.as_ref(), options.project)?;
    let window_code = match g.extensions.collect_windows {
        Some(hook) => hook(&mut g, module)?,
        None => String::new(),
    };
    let function_code = functions::emit_all(&mut g)?;
    // children first, then the root; the header needs every compound type
    let mut bodies = String::new();
    for index in 0..g.components.len() {
        if g.components[index].external {
            continue;
        }
        let function = (*g.components[index].func).clone();
        bodies.push_str(&emit_component(&mut g, &function, Some(index))?);
        bodies.push('\n');
    }
    if let Some(function) = &root {
        bodies.push_str(&emit_component(&mut g, function, None)?);
    }
    let source_name = source_name.replace('\n', " ");
    let declarations = globals::declarations(&g);
    let mut out = String::new();
    out.push_str(HEADER_ALLOWS);
    out.push_str(PREAMBLE);
    out.push_str(&declarations);
    out.push_str(&globals_code);
    out.push_str(&window_code);
    out.push_str(&function_code);
    out.push_str(&bodies);
    let raw = scope::rename_modules(&mut g, out)?;
    // `prettyplease` reformats from the `syn` AST and drops comments: keep the
    // `@generated` marker out of the formatted body and prepend it verbatim.
    let formatted = parse_and_format(&raw);
    Ok(GeneratedFile {
        source: format!("// @generated by zant-tsx from {source_name} — do not edit.\n{formatted}"),
    })
}

/// Imports come from `"zant"`, `"react"`, a relative file or a
/// `native/<module>` spec; imported names must be known.
fn validate_imports(g: &mut Generator<'_>, module: &ast::Module) -> Result<()> {
    for import in &module.imports {
        let source = String::from_utf8_lossy(&import.source);
        if source.starts_with('.') || source.starts_with("native/") {
            continue;
        }
        if source != "zant" && source != "react" {
            return fail(
                g,
                import.source_loc,
                "imports come from \"zant\", \"react\", \"native/<module>\" or a relative .tsx / .rs file",
            );
        }
        for name in &import.names {
            let imported = String::from_utf8_lossy(&name.imported.name);
            let local = String::from_utf8_lossy(&name.local.name);
            let tag = tags::find(&imported).is_some();
            let known = tag
                || hooks::extra_hook(&imported)
                || crate::codegen::hook_name(g, &imported)
                || types::numeric_name(name.imported.name.as_slice()).is_some()
                || types::node_name(name.imported.name.as_slice())
                || matches!(
                    imported.as_ref(),
                    "useState"
                        // lowered to its function (`react_compat`)
                        | "useCallback"
                        | "useTheme"
                        | "List"
                        | "createStore"
                        | "useStore"
                        | "KeyEvent"
                        | "createWindow"
                        | "Image"
                );
            if !known {
                if let Some(external) = g.extensions.hooks.external_name
                    && external(g, &imported)?.is_some()
                {
                    continue;
                }
                return fail(
                    g,
                    name.imported.loc,
                    format!("unknown import `{imported}`; this name is not supported by Zant"),
                );
            }
            // `import type {u16 as Small} from "zant"`
            // aliases the numeric type; tags cannot be renamed.
            if imported != local {
                if tag {
                    return fail(
                        g,
                        name.local.loc,
                        format!("tag imports cannot be renamed; use `{imported}`"),
                    );
                }
                if imported == "useState" {
                    g.hook_names.push(local.into_owned());
                } else if let Some(ty) = types::numeric_name(name.imported.name.as_slice()) {
                    g.type_aliases.push(super::TypeAlias {
                        name: local.into_owned(),
                        ty,
                    });
                } else {
                    return fail(g, name.local.loc, format!("`{imported}` cannot be renamed"));
                }
            }
        }
    }
    Ok(())
}

/// Per-component generator state starts empty; types, constants and import
/// aliases are shared by the whole module.
fn reset_component(g: &mut Generator<'_>) {
    g.states.clear();
    g.methods.clear();
    g.events.clear();
    g.render_names.clear();
    g.callback_events.clear();
    g.lists.clear();
    g.child_sites = 0;
    g.hook_fields.clear();
    g.effects.clear();
    g.cleanups.clear();
    g.capture_scope = None;
    g.handler_state = None;
    g.loop_depth = 0;
    g.scroll_depth = 0;
}

/// One component as a struct: the root with its state fields, or a child with
/// `props` and a builder function.
fn emit_component(
    g: &mut Generator<'_>,
    f: &ast::Function,
    component: Option<usize>,
) -> Result<String> {
    reset_component(g);
    if let Some(reference) = f.ret.as_ref() {
        let name = String::from_utf8_lossy(&reference.name);
        if name != "JSX.Element"
            && name != "React.ReactNode"
            && name != "ReactNode"
            && !crate::react_compat::node_type(&g.react_imports, &reference.name)
        {
            return fail(
                g,
                reference.loc,
                "component return type must be JSX.Element, React.ReactNode, or ReactNode",
            );
        }
    }
    g.name = String::from_utf8_lossy(&f.name).into_owned();
    g.current_component = component;
    g.component_props_read = false;
    g.child_component = component.is_some();
    collect(g, f)?;
    scope::module_names(g)?;
    if let Some(hook) = g.extensions.hooks.scan_body {
        hook(g, &f.body)?;
    }
    let body = match &f.body {
        ast::Body::Block(statements) => statements,
        ast::Body::Expr(_) => {
            return fail(
                g,
                f.loc,
                "render must return a node on every path; the final statement must be a return",
            );
        }
    };
    if body.is_empty()
        || !matches!(body[body.len() - 1].data, ast::StmtData::Ret(_))
        || !stmt::returns_all(body)
    {
        return fail(
            g,
            f.loc,
            "render must return a node on every path; the final statement must be a return",
        );
    }
    let mut c = scope::Context {
        render: true,
        ret: Type::Node,
        ..scope::Context::default()
    };
    let statements = stmt::statements(g, &mut c, body, true)?;
    let mut methods: Vec<String> = Vec::new();
    for index in 0..g.methods.len() {
        if g.methods[index].inline_only {
            continue;
        }
        let method = g.methods[index].clone();
        methods.push(emit_method(g, &method)?);
    }
    let render_body = scope::finish(g, &c, statements)?;
    let name = rust_text::id_at(g, &f.name, f.loc)?;
    let fields = state_fields(g)?;
    let captures = super::captures::declarations(g)?;
    let mut out = String::new();
    if let Some(child_index) = component {
        let component_data = g.components[child_index].clone();
        // a module-level `Props` would collide between components (and an
        // inherent associated type is unstable), so each component's props
        // type is named after it: `TodoRowProps = RowProps` / `PanelProps`
        let props_name = format!("{name}Props_");
        let props_type = match component_data.props {
            Some(ty) => format!("pub type {props_name} = {};", ty.rust(g)),
            None => format!("#[derive(Default)]\npub struct {props_name};"),
        };
        out.push_str(&format!("pub struct {name} {{\n"));
        if g.component_props_read {
            out.push_str(&format!("    props: {props_name},\n"));
        }
        if component_data.stateful {
            out.push_str("    _id: u64,\n");
        }
        // actions of a child reach their own local state slot through a `Weak`
        // upgrade (`callbacks capture state.clone()` in the design doc; a weak
        // handle lets handlers keep the `&mut self` receiver shape)
        out.push_str(&format!(
            "    _state: std::rc::Weak<std::cell::RefCell<{name}>>,\n"
        ));
        out.push_str(&fields);
        out.push_str(&render_state_parts(g)?.0);
        out.push_str("}\n\n");
        out.push_str(&format!("{props_type}\n\n"));
        out.push_str(&default_impl(g, &name, &component_data)?);
        out.push_str(&hot_impl(&name, &fields, false));
        out.push_str(&captures);
        out.push_str(&format!("impl {name} {{\n"));
        out.push_str(&builder_function(g, &name, &component_data, &props_name)?);
        let render_states = render_state_parts(g)?.2;
        out.push_str(&format!(
            "pub fn render(&mut self) -> ui::Node {{\n{render_states}{render_body}}}\n"
        ));
        for method in methods {
            out.push('\n');
            out.push_str(&method);
        }
        out.push_str("}\n");
        // a component named by `createWindow(..)` anywhere in the project
        // renders another window: its own module emits the `Window` wrapper
        // (a window handle in another file refers to `crate::<module>::XWindow`)
        let named_by_window = super::app_state::project().is_some_and(|project| {
            project
                .window_components
                .iter()
                .any(|window| *window == String::from_utf8_lossy(&f.name))
        });
        if named_by_window {
            out.push('\n');
            out.push_str(&super::windows::component_impl(
                g,
                &name,
                &format!("{name}Window"),
                f,
            )?);
        }
    } else {
        // no `derive(Default)`: the hand-written impl below carries the
        // `useState` initial values
        let render_fields = render_state_parts(g)?.0;
        out.push_str(&format!(
            "pub struct {name} {{\n{fields}{render_fields}}}\n\n"
        ));
        out.push_str(&default_impl(g, &name, &root_component_stub())?);
        out.push_str(&hot_impl(&name, &fields, false));
        out.push_str(&captures);
        out.push_str(&format!("impl {name} {{\n"));
        for method in methods {
            out.push_str(&method);
            out.push('\n');
        }
        out.push_str("}\n\n");
        let render_states = render_state_parts(g)?.2;
        out.push_str(&format!(
            "impl ui::App for {name} {{\nfn render(&mut self) -> ui::Node {{\n{render_states}{render_body}}}\n"
        ));
        let (save, load) = hot_fields(&fields);
        let mutable = if fields.trim().is_empty() { "" } else { "mut " };
        let restore_name = if fields.trim().is_empty() {
            "_restore"
        } else {
            "restore"
        };
        out.push_str(&format!("fn hot_save(&self) -> Option<String> {{ let {mutable}snapshot = tsx::hot::Snapshot::new(); {save} Some(snapshot.finish()) }}\nfn hot_load(&mut self, state: &str) {{ let {restore_name} = tsx::hot::Restore::parse(state); {load} }}\n"));
        if let Some(hook) = g.extensions.app_lifecycle {
            out.push_str(&hook(g)?);
        }
        out.push_str("}\n");
    }
    Ok(out)
}

/// Emit field dispatch with concrete types so autoref specialization can skip
/// bookkeeping and unsupported application values.
fn hot_fields(fields: &str) -> (String, String) {
    let mut save = String::new();
    let mut load = String::new();
    for line in fields.lines() {
        let Some((field, _)) = line.trim().split_once(':') else {
            continue;
        };
        save.push_str(&format!(
            "zant::hot_put!(snapshot, {field:?}, self.{field});\n"
        ));
        load.push_str(&format!(
            "zant::hot_get!(restore, {field:?}, self.{field});\n"
        ));
    }
    (save, load)
}

pub(super) fn hot_impl(name: &str, fields: &str, strict: bool) -> String {
    if fields.trim().is_empty() {
        return parse_and_format(&format!("impl tsx::hot::Hot for {name} {{
            fn save(&self) -> tsx::hot::Value {{ tsx::hot::Snapshot::new().value() }}
            fn load(value: &tsx::hot::Value) -> Option<Self> {{ value.as_object()?; Some(Self::default()) }}
        }}\n"));
    }
    let (save, load) = hot_fields(fields);
    let load = load.replace("self.", "instance.");
    let load = if strict {
        load.lines()
            .map(|line| format!("if !{} {{ return None; }}\n", line.trim_end_matches(';')))
            .collect::<String>()
    } else {
        load
    };
    parse_and_format(&format!("impl tsx::hot::Hot for {name} {{
        fn save(&self) -> tsx::hot::Value {{ let mut snapshot = tsx::hot::Snapshot::new(); {save} snapshot.value() }}
        fn load(value: &tsx::hot::Value) -> Option<Self> {{
            value.as_object()?;
            let restore = tsx::hot::Restore::parse(&value.to_string());
            let mut instance = Self::default(); {load} Some(instance)
        }}
    }}\n"))
}

/// A stand-in for the root "component data" (no props, no id).
fn root_component_stub() -> components::Component {
    components::Component {
        name: String::new(),
        builder: String::new(),
        func: Box::new(ast::Function {
            loc: 0,
            name: Vec::new(),
            params: Vec::new(),
            ret: None,
            body: ast::Body::Block(Vec::new()),
        }),
        props: None,
        param: String::new(),
        destructure: Vec::new(),
        destructure_defaults: Vec::new(),
        external: false,
        stateful: false,
    }
}

/// The hand-written `impl Default` of a component: state fields keep their
/// `useState` / `useRef` initial values, everything else is `Default::default()`.
fn default_impl(
    g: &mut Generator<'_>,
    name: &str,
    component: &components::Component,
) -> Result<String> {
    let mut values = String::new();
    if component.props.is_none() && component.name.is_empty() {
        // root
    } else {
        if g.component_props_read {
            values.push_str("            props: Default::default(),\n");
        }
        if component.stateful {
            values.push_str("            _id: 0,\n");
        }
        values.push_str("            _state: Default::default(),\n");
    }
    let states = g.states.clone();
    for state in &states {
        let field = rust_text::id_at(g, state.name.as_bytes(), 0)?;
        values.push_str(&format!("            {field}: {},\n", state.initial));
    }
    values.push_str(&render_state_parts(g)?.1);
    let hook_fields = g.hook_fields.clone();
    for field in &hook_fields {
        let id = rust_text::id_at(g, field.name.as_bytes(), 0)?;
        values.push_str(&format!("            {id}: Default::default(),\n"));
    }
    for info in &g.lists {
        values.push_str(&format!(
            "            {}: Default::default(),\n",
            info.field
        ));
    }
    Ok(format!(
        "impl Default for {name} {{\n    fn default() -> Self {{\n        Self {{\n{values}        }}\n    }}\n}}\n\n"
    ))
}

/// React gives every handler the state of the render it came from: setting
/// state during an event does not change what the rest of that event reads
/// (`bump(); bump()` with `const bump = () => setN(n + 1)` sets `n + 1`
/// once). Handlers snapshot `_render_<state>`, assigned when the component
/// renders; functional updates (`setN(p => p + 1)`) read the latest value.
pub(crate) fn render_state_field(name: &str) -> String {
    format!("_render_{name}")
}

/// The `_render_<state>` fields: declarations, `Default` values and the
/// assignments at the start of `render` (kept out of `fields`, so hot reload
/// does not save them).
fn render_state_parts(g: &mut Generator<'_>) -> Result<(String, String, String)> {
    let mut declarations = String::new();
    let mut defaults = String::new();
    let mut assignments = String::new();
    for state in g.states.clone() {
        if state.kind != super::StateKind::State {
            continue;
        }
        let field = scope::state_field(g, state.name.as_bytes())?;
        let shadow = render_state_field(&state.name);
        declarations.push_str(&format!("    {shadow}: {},\n", state.ty.rust(g)));
        defaults.push_str(&format!("            {shadow}: {},\n", state.initial));
        let value = if state.ty.copy(g) {
            format!("self.{field}")
        } else {
            format!("self.{field}.clone()")
        };
        assignments.push_str(&format!("self.{shadow} = {value};\n"));
    }
    Ok((declarations, defaults, assignments))
}

/// State fields with their types (values come from [`default_impl`]).
fn state_fields(g: &mut Generator<'_>) -> Result<String> {
    let mut out = String::new();
    for index in 0..g.states.len() {
        let state = g.states[index].clone();
        let field = rust_text::id_at(g, state.name.as_bytes(), 0)?;
        out.push_str(&format!("    {field}: {},\n", state.ty.rust(g)));
    }
    let hook_fields = g.hook_fields.clone();
    for field in &hook_fields {
        let id = rust_text::id_at(g, field.name.as_bytes(), 0)?;
        out.push_str(&format!("    {id}: {},\n", field.rust_type));
    }
    // `<List>` scroll state (`list` registers `_listN` fields).
    let lists = g.lists.clone();
    for info in &lists {
        out.push_str(&format!(
            "    {}: std::rc::Rc<std::cell::RefCell<ui::ListState>>,\n",
            info.field
        ));
    }
    Ok(out)
}

/// `pub fn build(props: Props[, id: u64]) -> ui::Node` inside the component's
/// impl: builds the frame-local instance over the `ui::local_state` slot.
fn builder_function(
    g: &mut Generator<'_>,
    name: &str,
    component: &components::Component,
    props_name: &str,
) -> Result<String> {
    let props_param = if g.component_props_read {
        "props"
    } else {
        "_props"
    };
    let props_argument = format!("{props_param}: {props_name}");
    if !component.stateful {
        let props_field = if g.component_props_read {
            "props: props, "
        } else {
            ""
        };
        return Ok(format!(
            "\npub fn build({props_argument}) -> ui::Node {{\n\
             let mut instance = {name} {{ {props_field}_state: Default::default() }};\n\
             instance.render()\n}}\n"
        ));
    }
    let props_assignment = if g.component_props_read {
        "instance.props = props;\n"
    } else {
        ""
    };
    Ok(format!(
        "\npub fn build({props_argument}, id: u64) -> ui::Node {{\n\
         let state: std::rc::Rc<std::cell::RefCell<{name}>> = ui::local_state::local_state_hot::<{name}>(id, {name}::default);\n\
         {{\n\
         let mut instance = state.borrow_mut();\n\
         {props_assignment}\
         instance._id = id;\n\
         instance._state = std::rc::Rc::downgrade(&state);\n\
         instance.render()\n\
         }}\n}}\n"
    ))
}

/// A handler bound through `self._state` needs a component built over a state
/// slot: a component classified without one (`stateful.rs`) would get an empty
/// `Weak` and drop the event silently, so that is a compile error.
fn require_instance(g: &mut Generator<'_>, method: &super::Method) -> Result<()> {
    let Some(index) = g.current_component else {
        return Ok(());
    };
    if g.components[index].stateful {
        return Ok(());
    }
    let loc = method.func.as_deref().map_or(0, |function| function.loc);
    let component = g.components[index].name.clone();
    fail(
        g,
        loc,
        format!(
            "internal: a handler of `{component}` needs the component's state slot, but \
             `{component}` was classified without one (zant-tsx `stateful.rs`); please \
             report this. Workaround: write the handler as an inline arrow function"
        ),
    )
}

/// Emit a method with inferred event parameters.
pub fn emit_method(g: &mut Generator<'_>, m: &super::Method) -> Result<String> {
    if let Some(emitted) = &m.emitted {
        return Ok(emitted.clone());
    }
    if m.setter.is_some() {
        return match g.extensions.hooks.emit_setter {
            Some(hook) => hook(g, m),
            None => fail(
                g,
                0,
                "setter adapter methods need the event emitter installed",
            ),
        };
    }
    method(g, m, None, None)
}

/// `fn name(&mut self[, cap: &_CaptureN][, params…]) -> Ret { body }`.
pub fn method(
    g: &mut Generator<'_>,
    m: &super::Method,
    source: Option<&scope::Context>,
    index: Option<usize>,
) -> Result<String> {
    let Some(f) = m.func.clone() else {
        return fail(g, 0, format!("method `{}` has no function body", m.name));
    };
    // a handler that calls a callback prop needs the runtime context (see
    // `action_cx`): it is the first parameter, before the event parameters
    let needs_cx = super::action_cx::needs_cx(g, &f);
    let mut c = scope::Context {
        render: false,
        ret: m.ret,
        capture_source: source.map(|source| source.capture_source()),
        capture_method: index,
        effect_field: m.effect_field.clone(),
        cleanup: m.cleanup,
        cx_var: needs_cx.then(|| super::action_cx::CX.to_owned()),
        ..scope::Context::default()
    };
    let mut params: Vec<String> = Vec::new();
    if needs_cx {
        params.push(format!(", {}: &mut ui::ActionCx<'_>", super::action_cx::CX));
    }
    let mut text_parameter: Option<String> = None;
    for (i, p) in f.params.iter().enumerate() {
        let ty = match g.extensions.hooks.method_param {
            Some(hook) => hook(g, m, i, p)?,
            None => match p.ty.as_ref() {
                Some(reference) => types::type_ref(g, reference, false)?,
                None => {
                    return fail(
                        g,
                        p.name.loc,
                        format!(
                            "parameter `{}` needs a type (event parameter inference needs the event emitter installed)",
                            String::from_utf8_lossy(&p.name.name)
                        ),
                    );
                }
            },
        };
        let binding = scope::bind(g, &mut c, &p.name.name, ty, false)?;
        c.params.push(binding);
        let id = c.bindings[binding].rust.clone();
        // `ui::TextAction` passes `&str`; the TSX parameter is a string
        if m.action == Some(tags::Kind::TextAction) && ty == Type::String {
            params.push(format!(", {id}: &str"));
            text_parameter = Some(id);
        } else {
            params.push(format!(", {id}: {}", ty.rust(g)));
        }
    }
    if let Some(expected) = &m.callback_params
        && f.params.len() < expected.len()
    {
        for ty in &expected[f.params.len()..] {
            params.push(format!(", _: {}", ty.rust(g)));
        }
    } else if let Some(kind) = m.action
        && !matches!(kind, tags::Kind::Action)
        && f.params.is_empty()
    {
        let ty = match kind {
            tags::Kind::TextAction => "&str".to_owned(),
            other => components::action_value_type(other).rust(g).to_owned(),
        };
        params.push(format!(", _: {ty}"));
    }
    let body = match &f.body {
        ast::Body::Block(statements) => method_block(g, &mut c, f.loc, statements)?,
        ast::Body::Expr(e) => method_expression(g, &mut c, e)?,
    };
    let body = match &text_parameter {
        Some(id) => format!("let {id}: zant::Str = zant::Str::copy({id});\n{body}"),
        None => body,
    };
    let finished = scope::finish(g, &c, body)?;
    let name = rust_text::id_at(g, m.name.as_bytes(), 0)?;
    // captures are discovered while emitting the body, so read them back from
    // the generator rather than from `m` (captures.prepare passes a snapshot)
    let captures = match index {
        Some(index) => g.methods[index].captures.clone(),
        None => m.captures.clone(),
    };
    let receiver = if captures.is_empty() {
        "&mut self".to_owned()
    } else {
        format!(
            "&mut self, cap: &{}",
            m.capture_name.clone().unwrap_or_default()
        )
    };
    let returns = if m.ret == Type::Void {
        String::new()
    } else {
        format!(" -> {}", m.ret.rust(g))
    };
    Ok(format!(
        "fn {name}({receiver}{}){returns} {{\n{finished}}}\n",
        params.join("")
    ))
}

/// Collect top-level hooks and component functions before resolving JSX event
/// references.
pub fn collect(g: &mut Generator<'_>, f: &ast::Function) -> Result<()> {
    let ast::Body::Block(statements) = &f.body else {
        return fail(g, f.loc, "a component body must be a block");
    };
    for s in statements {
        match &s.data {
            ast::StmtData::Decl(d) => collect_declaration(g, s.loc, d)?,
            ast::StmtData::Func(func) => collect_method(g, &func.name, func)?,
            ast::StmtData::Expr(e) => {
                if let Some(name) = hooks::call_name(e) {
                    if name == "useEffect" {
                        hooks::collect_effect(g, e)?;
                    }
                    if (g.extensions.is_app_hook)(&name) {
                        super::app_hooks::collect(g, e)?;
                    }
                }
            }
            _ => {}
        }
    }
    for s in statements {
        render_names(g, s);
    }
    mark_inline(g)?;
    // desktop hooks may take a function that reads render locals (marked above)
    for s in statements {
        let e = match &s.data {
            ast::StmtData::Decl(d) => match d.init.as_deref() {
                Some(e) => e,
                None => continue,
            },
            ast::StmtData::Expr(e) => e,
            _ => continue,
        };
        if let Some(name) = hooks::call_name(e)
            && (g.extensions.is_desktop_hook)(&name)
        {
            super::desktop_hooks::collect(g, e)?;
        }
    }
    for state in g.states.clone() {
        if scope::method_index(g, state.name.as_bytes()).is_some() || state.name == "render" {
            return fail(
                g,
                f.loc,
                format!("state `{}` conflicts with a method; rename it", state.name),
            );
        }
    }
    Ok(())
}

/// Component functions that read render locals (also through another such
/// function) are emitted where they are passed, capturing that render.
fn mark_inline(g: &mut Generator<'_>) -> Result<()> {
    let mut names: Vec<String> = Vec::new();
    // refs are fields of the component: functions read them directly
    for name in g.render_names.clone() {
        if scope::state_index(g, name.as_bytes()).is_none() {
            names.push(name);
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for index in 0..g.methods.len() {
            if !g.methods[index].user || g.methods[index].inline_only {
                continue;
            }
            let Some(function) = g.methods[index].func.clone() else {
                continue;
            };
            if !super::free_names::function(
                &function,
                &names.iter().map(|name| name.as_str()).collect::<Vec<_>>(),
            ) {
                continue;
            }
            g.methods[index].inline_only = true;
            names.push(g.methods[index].name.clone());
            changed = true;
        }
    }
    Ok(())
}

/// The function of a component function that reads render locals.
pub fn inline_function(g: &Generator<'_>, n: &str) -> Option<ast::Function> {
    let index = scope::source_method(g, n.as_bytes())?;
    let method = &g.methods[index];
    if method.inline_only {
        method.func.as_deref().cloned()
    } else {
        None
    }
}

pub fn collect_method(g: &mut Generator<'_>, n: &[u8], f: &ast::Function) -> Result<()> {
    let name = String::from_utf8_lossy(n);
    let reserved = name == "render" || name == "build" || name == "self" || name == g.name;
    if scope::method_index(g, n).is_some() || scope::state_index(g, n).is_some() || reserved {
        return fail(
            g,
            f.loc,
            format!("duplicate or reserved method name `{name}`; rename it"),
        );
    }
    let ret = match f.ret.as_ref() {
        Some(reference) => types::type_ref(g, reference, true)?,
        None => Type::Void,
    };
    g.methods.push(super::Method {
        name: name.into_owned(),
        func: Some(Box::new(f.clone())),
        ret,
        user: true,
        ..super::Method::default()
    });
    Ok(())
}

pub fn collect_state(
    g: &mut Generator<'_>,
    loc: ast::Loc,
    d: &ast::Decl,
    call: &ast::Call,
) -> Result<()> {
    if !state_pattern(&d.pattern) {
        return fail(
            g,
            loc,
            "useState needs exactly two names: const [value, setValue] = useState(...)",
        );
    }
    if !d.is_const {
        return fail(g, loc, "declare useState with const");
    }
    if call.args.len() != 1 || call.type_args.len() > 1 {
        return fail(
            g,
            loc,
            "useState takes one literal initializer and at most one type argument",
        );
    }
    let init = &call.args[0];
    let aggregate_init = matches!(
        init.data,
        ast::ExprData::Array(_)
            | ast::ExprData::Object(_)
            | ast::ExprData::NullLit
            | ast::ExprData::Ident(_)
    );
    if !types::is_literal(init) && !aggregate_init {
        return fail(
            g,
            init.loc,
            "useState initializer must be a number, string, or boolean literal",
        );
    }
    let mut c = scope::Context {
        render: true,
        ret: Type::Node,
        constant: true,
        ..scope::Context::default()
    };
    let target = match call.type_args.first() {
        Some(reference) => Some(types::type_ref(g, reference, false)?),
        None => None,
    };
    let mut value = expr::emit(g, &mut c, init, target, false)?;
    let concrete = target.unwrap_or_else(|| value.ty.concrete());
    value = types::coerce(g, value, concrete, init.loc)?;
    let ast::Pattern::Array(names) = &d.pattern else {
        unreachable!("state pattern")
    };
    let n = String::from_utf8_lossy(&names[0].as_ref().expect("value name").name).into_owned();
    let setter =
        String::from_utf8_lossy(&names[1].as_ref().expect("setter name").name).into_owned();
    let value_conflict = scope::state_index(g, n.as_bytes()).is_some()
        || scope::setter_index(g, n.as_bytes()).is_some();
    let setter_conflict = scope::state_index(g, setter.as_bytes()).is_some()
        || scope::setter_index(g, setter.as_bytes()).is_some();
    if n == setter || value_conflict || setter_conflict {
        return fail(
            g,
            loc,
            format!("duplicate state or setter name; rename `{n}`"),
        );
    }
    g.states.push(super::State {
        name: n,
        setter,
        ty: value.ty,
        initial: value.code,
        kind: super::StateKind::State,
    });
    Ok(())
}

/// Emit a `ui::*` action for method `index`, capturing render locals when
/// the method needs them.
pub fn action_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    index: usize,
    kind: Option<tags::Kind>,
) -> Result<String> {
    let Some(kind) = kind else {
        return fail(
            g,
            0,
            "callback props whose signature is not a ui event need the runtime `ActionCx`; \
             this is not supported yet",
        );
    };
    let (constructor, closure_params, call_argument) = match kind {
        tags::Kind::Action => ("ui::Action", String::new(), String::new()),
        tags::Kind::BoolAction => (
            "ui::BoolAction",
            ", value: bool".to_owned(),
            ", value".to_owned(),
        ),
        tags::Kind::TextAction => (
            "ui::TextAction",
            ", value: &str".to_owned(),
            ", value".to_owned(),
        ),
        tags::Kind::IndexAction => (
            "ui::IndexAction",
            ", index: usize".to_owned(),
            ", index".to_owned(),
        ),
        other => {
            return fail(
                g,
                0,
                format!("action kind `{}` is not supported here", other as i32),
            );
        }
    };
    let method = g.methods[index].clone();
    if method.capture_name.is_some() {
        super::captures::prepare(g, c, index)?;
    }
    let method = g.methods[index].clone();
    // handlers that call a callback prop take the runtime context
    let needs_cx = method
        .func
        .as_deref()
        .is_some_and(|function| super::action_cx::needs_cx(g, function));
    let capture = if method.captures.is_empty() {
        None
    } else {
        Some(super::captures::value(g, &method)?)
    };
    let name = rust_text::id_at(g, method.name.as_bytes(), 0)?;
    let root = g.current_component.is_none();
    let generator_name = g.name.clone();
    let root_name = rust_text::id_at(g, generator_name.as_bytes(), 0)?;
    let mut prefix = String::new();
    if let Some(capture) = &capture {
        prefix.push_str(&format!("let _capture = {capture};\n"));
    }
    let mut arguments: Vec<String> = Vec::new();
    if capture.is_some() {
        arguments.push("&_capture".to_owned());
    }
    if needs_cx {
        arguments.push(super::action_cx::CX.to_owned());
    }
    if !call_argument.is_empty() {
        arguments.push(call_argument.trim_start_matches(", ").to_owned());
    }
    let child_arguments = arguments.join(", ");
    let arguments = if child_arguments.is_empty() {
        String::new()
    } else {
        format!(", {child_arguments}")
    };
    if root {
        // root handlers take the runtime state: `ui::action(|s: &mut App| …)`.
        // The block is only needed to hold the capture statements; a bare
        // closure avoids `unused_braces` in the generated app.
        let builder = components::action_builder(kind);
        let closure =
            format!("move |s: &mut {root_name}{closure_params}| {root_name}::{name}(s{arguments})");
        if prefix.is_empty() {
            return Ok(format!("{builder}({closure})"));
        }
        return Ok(format!("{builder}({{\n{prefix}{closure}\n}})"));
    }
    // A `<List>` row closure cannot borrow `self`: it upgrades the state slot
    // hoisted into a local instead (`list.rs`).
    let state = match &g.handler_state {
        Some(local) => format!("{local}.clone()"),
        None => {
            require_instance(g, &method)?;
            "self._state.clone()".to_owned()
        }
    };
    Ok(format!(
        "{constructor}::new({{\n{prefix}let _state = {state};\n\
         move |_cx{closure_params}| {{\n\
         if let Some(state) = _state.upgrade() {{\n\
         state.borrow_mut().{name}({child_arguments});\n\
         }}\n}}\n}})"
    ))
}

/// A callback prop whose signature is not a ui event (`(id: u32) => void`):
/// `std::rc::Rc<dyn Fn(&mut ui::ActionCx<'_>, …)>` calling the generated
/// handler with the runtime context.
pub fn callback_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    index: usize,
    params: &[Type],
) -> Result<String> {
    let method = g.methods[index].clone();
    if method.capture_name.is_some() {
        super::captures::prepare(g, c, index)?;
    }
    let method = g.methods[index].clone();
    // handlers that call a callback prop take the runtime context
    let needs_cx = method
        .func
        .as_deref()
        .is_some_and(|function| super::action_cx::needs_cx(g, function));
    let capture = if method.captures.is_empty() {
        None
    } else {
        Some(super::captures::value(g, &method)?)
    };
    let name = rust_text::id_at(g, method.name.as_bytes(), 0)?;
    let root = g.current_component.is_none();
    let generator_name = g.name.clone();
    let root_name = rust_text::id_at(g, generator_name.as_bytes(), 0)?;
    let mut prefix = String::new();
    if let Some(capture) = &capture {
        prefix.push_str(&format!("let _capture = {capture};\n"));
    }
    let mut closure_params = String::new();
    let mut call_args: Vec<String> = Vec::new();
    if capture.is_some() {
        call_args.push("&_capture".to_owned());
    }
    if needs_cx {
        call_args.push(super::action_cx::CX.to_owned());
    }
    for (index, ty) in params.iter().enumerate() {
        closure_params.push_str(&format!(", _a{index}: {}", ty.rust(g)));
        call_args.push(format!("_a{index}"));
    }
    let call_args = call_args.join(", ");
    let call_args = if call_args.is_empty() {
        String::new()
    } else {
        format!(", {call_args}")
    };
    if root {
        return Ok(format!(
            "std::rc::Rc::new({{\n{prefix}move |_cx: &mut ui::ActionCx<'_>{closure_params}| {{\n\
             let _root = _cx.state::<{root_name}>();\n\
             {root_name}::{name}(_root{call_args})\n}}\n}})"
        ));
    }
    // A `<List>` row closure cannot borrow `self`: it upgrades the state slot
    // hoisted into a local instead (`list.rs`).
    let state = match &g.handler_state {
        Some(local) => format!("{local}.clone()"),
        None => {
            require_instance(g, &method)?;
            "self._state.clone()".to_owned()
        }
    };
    Ok(format!(
        "std::rc::Rc::new({{\n{prefix}let _state = {state};\n\
         move |_cx: &mut ui::ActionCx<'_>{closure_params}| {{\n\
         if let Some(state) = _state.upgrade() {{\n\
         state.borrow_mut().{name}({})\n\
         }}\n}}\n}})",
        call_args.trim_start_matches(", ")
    ))
}

/// Collect top-level state and functions without adding derived render locals.
fn collect_declaration(g: &mut Generator<'_>, loc: ast::Loc, d: &ast::Decl) -> Result<()> {
    let Some(init) = d.init.as_deref() else {
        return Ok(());
    };
    if super::hook(g, init) {
        let ast::ExprData::Call(call) = &init.data else {
            unreachable!("hook call")
        };
        let ast::ExprData::Ident(name) = &call.callee.data else {
            unreachable!("hook callee")
        };
        let name = String::from_utf8_lossy(name);
        if name == "useRef" {
            return hooks::collect_ref(g, loc, d, call);
        }
        if name == "useMemo" || name == "useStore" {
            return Ok(()); // render locals (stmt.declaration)
        }
        if name == "useEffect" {
            return fail(g, loc, "useEffect returns nothing");
        }
        if (g.extensions.is_desktop_hook)(&name) {
            return Ok(()); // after `mark_inline`
        }
        return collect_state(g, loc, d, call);
    }
    if matches!(init.data, ast::ExprData::Arrow(_))
        && let ast::Pattern::Ident(name) = &d.pattern
    {
        let ast::ExprData::Arrow(function) = &init.data else {
            unreachable!("arrow")
        };
        return collect_method(g, &name.name, function);
    }
    Ok(())
}

/// Validate destructuring with short circuit checks before accessing its names.
fn state_pattern(pattern: &ast::Pattern) -> bool {
    match pattern {
        ast::Pattern::Array(names) => names.len() == 2 && names[0].is_some() && names[1].is_some(),
        _ => false,
    }
}

/// Keep reserved import and receiver names out of the component declaration.
fn reserved_component_name(name: &str) -> bool {
    matches!(name, "self" | "std" | "zant" | "ui" | "tsx")
}

/// Check complete returns before emitting a component method's statements.
fn method_block(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    statements: &[Box<ast::Stmt>],
) -> Result<String> {
    if c.ret != Type::Void && !stmt::returns_all(statements) {
        return fail(g, loc, "function must return a value on every path");
    }
    stmt::statements(g, c, statements, false)
}

/// Treat a concise arrow as a statement or return according to its declared
/// type.
fn method_expression(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
) -> Result<String> {
    if c.ret == Type::Void {
        return stmt::expression_statement(g, c, e);
    }
    let value = expr::emit(g, c, e, Some(c.ret), false)?;
    Ok(format!("return {};\n", value.code))
}

/// Collect the render locals of a statement's declarations (`codegen.renderNames`).
fn render_names(g: &mut Generator<'_>, s: &ast::Stmt) {
    match &s.data {
        ast::StmtData::Decl(d) => {
            if let ast::Pattern::Ident(name) = &d.pattern
                && d.init
                    .as_deref()
                    .is_none_or(|init| !matches!(init.data, ast::ExprData::Arrow(_)))
            {
                g.render_names
                    .push(String::from_utf8_lossy(&name.name).into_owned());
            }
        }
        ast::StmtData::Block(statements) => {
            for child in statements {
                render_names(g, child);
            }
        }
        ast::StmtData::If(v) => {
            render_names(g, &v.then);
            if let Some(otherwise) = v.else_.as_deref() {
                render_names(g, otherwise);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    fn parse_module(src: &str) -> ast::Module {
        let mut parser = Parser::init(src);
        parser.parse_module().expect("test source parses")
    }

    #[test]
    fn a_component_without_a_root_is_rejected() {
        let module = parse_module("function Child() { return <Text>hi</Text>; }\n");
        let error = generate("app.tsx", &module, &Options::default()).unwrap_err();
        assert_eq!(crate::codegen::Error::Tsx, error);
    }

    #[test]
    fn a_module_without_components_emits_types_and_constants() {
        let source = "interface Point { x: number; y: number }\nexport const origin: Point = { x: 0, y: 0 };\n";
        let module = parse_module(source);
        let options = Options {
            source_name: "types.tsx",
            require_root: false,
            ..Options::default()
        };
        // requires JSX-free emission: types + constants only
        let file = generate("types.tsx", &module, &options).expect("types generate");
        assert!(file.source.contains("pub struct Point"), "{}", file.source);
        assert!(
            file.source.contains("pub fn origin() -> Point"),
            "{}",
            file.source
        );
        assert!(
            file.source.starts_with("// @generated by zant-tsx"),
            "{}",
            file.source
        );
    }
}
