//! Emitting one component: its struct, state fields, `Default`, `render` and the child builder function.

use super::*;

/// Per-component generator state starts empty; types, constants and import
/// aliases are shared by the whole module.
pub(super) fn reset_component(g: &mut Generator<'_>) {
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
pub(super) fn emit_component(
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
        // reactions and local functions are emitted where they are used;
        // one never reached (`.then` on something else) is not a method
        if (g.methods[index].promise_callback || g.methods[index].local_function)
            && g.methods[index].emitted.is_none()
        {
            continue;
        }
        let method = g.methods[index].clone();
        methods.push(emit_method(g, &method)?);
    }
    let render_body = scope::finish(g, &c, statements)?;
    let name = rust_text::id_at(g, &f.name, f.loc)?;
    let fields = state_fields(g)?;
    let captures = crate::codegen::captures::declarations(g)?;
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
        let named_by_window = crate::codegen::app_state::project().is_some_and(|project| {
            project
                .window_components
                .iter()
                .any(|window| *window == String::from_utf8_lossy(&f.name))
        });
        if named_by_window {
            out.push('\n');
            out.push_str(&crate::codegen::windows::component_impl(
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

/// A stand-in for the root "component data" (no props, no id).
pub(super) fn root_component_stub() -> components::Component {
    components::Component {
        name: String::new(),
        builder: String::new(),
        func: Box::new(ast::Function {
            type_params: Vec::new(),
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
pub(super) fn default_impl(
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
pub(super) fn render_state_parts(g: &mut Generator<'_>) -> Result<(String, String, String)> {
    let mut declarations = String::new();
    let mut defaults = String::new();
    let mut assignments = String::new();
    for state in g.states.clone() {
        if state.kind != crate::codegen::StateKind::State {
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
pub(super) fn state_fields(g: &mut Generator<'_>) -> Result<String> {
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
pub(super) fn builder_function(
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
