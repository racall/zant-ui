//! Child components (`docs/tsx.md`, "子组件").
//!
//! Each non-default top-level function becomes a struct with a `build` function:
//!
//! ```text
//! pub struct TodoRow { props: Props, _id: u64 }
//! impl TodoRow {
//!     pub fn build(props: Props, id: u64) -> ui::Node { … }
//!     fn render(&mut self) -> ui::Node { … }
//! }
//! ```
//!
//! Stateful children grow a `Rc<RefCell<…>>` local state slot (see
//! [`super::generate`]); callback props take `ui::Action`-family types when
//! their signature matches a ui event and `std::rc::Rc<dyn Fn(..)>` otherwise.

use crate::ast;
use crate::tags;

use super::captures;
use super::expr;
use super::functions;
use super::model;
use super::nesting;
use super::rust_text;
use super::scope;
use super::spread;
use super::stateful;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

#[derive(Clone)]
pub struct Component {
    /// TSX and Rust struct name (`TodoRow`).
    pub name: String,
    /// Builder path (`TodoRow::build`).
    pub builder: String,
    pub func: Box<ast::Function>,
    /// The props struct type (an empty struct when there is no parameter).
    pub props: Option<Type>,
    /// The parameter's name (`props`) when not destructured.
    pub param: String,
    /// Destructured prop names (`{ todo, on_toggle }`) and their defaults.
    pub destructure: Vec<ast::Name>,
    pub destructure_defaults: Vec<Option<Box<ast::Expr>>>,
    /// Imported from another file: `builder` is already Rust code.
    pub external: bool,
    /// Keeps local state (see `stateful`): the builder takes an instance id.
    pub stateful: bool,
}

/// An inline arrow passed to a child component's callback prop.
#[derive(Clone, Debug)]
pub struct CallbackEvent {
    pub expr: ast::Expr,
    pub method: usize,
}

// ── types ─────────────────────────────────────────────────────────────

/// The action type of a callback signature, if it matches a ui event.
pub fn action_kind(params: &[Type]) -> Option<tags::Kind> {
    if params.is_empty() {
        return Some(tags::Kind::Action);
    }
    if params.len() != 1 {
        return None;
    }
    match params[0] {
        Type::Boolean => Some(tags::Kind::BoolAction),
        Type::String => Some(tags::Kind::TextAction),
        Type::Usize => Some(tags::Kind::IndexAction),
        _ => None,
    }
}

/// The Rust type of an action kind (`ui::TextAction`).
pub fn action_type(kind: tags::Kind) -> &'static str {
    match kind {
        tags::Kind::Action => "ui::Action",
        tags::Kind::BoolAction => "ui::BoolAction",
        tags::Kind::TextAction => "ui::TextAction",
        tags::Kind::IndexAction => "ui::IndexAction",
        tags::Kind::ShortcutAction => "ui::HotkeyAction",
        _ => "ui::Action",
    }
}

/// The `ui::*` builder of an action kind (`ui::text_action`).
pub fn action_builder(kind: tags::Kind) -> &'static str {
    match kind {
        tags::Kind::Action => "ui::action",
        tags::Kind::BoolAction => "ui::bool_action",
        tags::Kind::TextAction => "ui::text_action",
        tags::Kind::IndexAction => "ui::index_action",
        tags::Kind::ShortcutAction => "ui::hotkey_action",
        _ => "ui::action",
    }
}

/// `()` / `bool` / `&str` / `usize`; action callbacks return nothing.
pub fn action_value_type(kind: tags::Kind) -> Type {
    match kind {
        tags::Kind::Action => Type::Void,
        tags::Kind::TextAction => Type::String,
        tags::Kind::BoolAction => Type::Boolean,
        tags::Kind::IndexAction => Type::Usize,
        _ => Type::Void,
    }
}

/// `(id: number) => void` as a callback type (interned by signature): a ui
/// action when the signature matches an event, `Rc<dyn Fn(..)>` otherwise.
pub fn callback_type(g: &mut Generator<'_>, loc: ast::Loc, f: &ast::FuncType) -> Result<Type> {
    let ret = types::type_ref(g, &f.ret, true)?;
    if ret != Type::Void {
        return fail(g, loc, "callback props must return void");
    }
    let mut params: Vec<Type> = Vec::new();
    for param in &f.params {
        let Some(reference) = param.ty.as_ref() else {
            return fail(
                g,
                param.name.loc,
                format!(
                    "callback parameter `{}` needs a type",
                    String::from_utf8_lossy(&param.name.name)
                ),
            );
        };
        params.push(types::type_ref(g, reference, false)?);
    }
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.kind == model::InfoKind::Callback && entry.params == params {
            return Ok(Type::Compound(index as u32));
        }
    }
    let rust = match action_kind(&params) {
        Some(kind) if !matches!(kind, tags::Kind::ShortcutAction) => action_type(kind).to_owned(),
        // Not a ui event: the callback runs from a handler with the runtime
        // context, which is how it reaches the root state (a closure cannot
        // capture `&mut App`, so it takes the `ActionCx` the handler runs in).
        _ => format!(
            "std::rc::Rc<dyn Fn({})>",
            std::iter::once("&mut ui::ActionCx<'_>".to_owned())
                .chain(params.iter().map(|ty| ty.rust(g).to_owned()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    model::add(
        g,
        model::Info {
            kind: model::InfoKind::Callback,
            rust,
            params,
            ..model::Info::default()
        },
    )
}

/// The callback info of `ty` (or of its optional child).
pub fn callback_info(g: &Generator<'_>, ty: Type) -> Option<model::Info> {
    let base = model::optional_child(g, ty).unwrap_or(ty);
    let entry = model::info(g, base)?;
    if entry.kind == model::InfoKind::Callback {
        Some(entry)
    } else {
        None
    }
}

// ── collection ────────────────────────────────────────────────────────

/// Registers every non-default top-level function as a child component.
pub fn collect(
    g: &mut Generator<'_>,
    module: &ast::Module,
    root: Option<&ast::Function>,
    project: &[stateful::ModuleSource<'_>],
) -> Result<()> {
    for top in &module.functions {
        if let Some(root) = root
            && top.func.name == root.name
            && top.func.loc == root.loc
        {
            continue;
        }
        if functions::is_function(&top.func) {
            continue;
        }
        let f = &top.func;
        if f.params.len() > 1 {
            return fail(g, f.params[1].name.loc, "components take one props object");
        }
        let name = String::from_utf8_lossy(&f.name).into_owned();
        let builder = builder_name(g, &f.name)?;
        let mut component = Component {
            name: name.clone(),
            builder,
            func: f.clone(),
            props: None,
            param: String::new(),
            destructure: Vec::new(),
            destructure_defaults: Vec::new(),
            external: false,
            stateful: stateful::stateful(project, module, f),
        };
        if let Some(p) = f.params.first() {
            let Some(reference) = p.ty.as_ref() else {
                return fail(
                    g,
                    p.name.loc,
                    format!("props need a type: function {name}(props: {name}Props)"),
                );
            };
            let ty = types::type_ref(g, reference, false)?;
            let entry = model::info(g, ty);
            if entry
                .as_ref()
                .is_none_or(|entry| entry.kind != model::InfoKind::Object)
            {
                return fail(g, reference.loc, "props must be an object type");
            }
            component.props = Some(ty);
            component.param = String::from_utf8_lossy(&p.name.name).into_owned();
            component.destructure = p.destructure.clone();
            component.destructure_defaults = p.destructure_defaults.clone();
            for (index, n) in p.destructure.iter().enumerate() {
                let Some(field) = field_type(g, ty, &n.name) else {
                    return fail(
                        g,
                        n.loc,
                        format!(
                            "`{}` is not a prop of `{}`",
                            String::from_utf8_lossy(&n.name),
                            String::from_utf8_lossy(&reference.name)
                        ),
                    );
                };
                let default = p.destructure_defaults.get(index).cloned().flatten();
                if let Some(default) = default {
                    functions::prop(g, n, &default, field, "")?;
                }
            }
        }
        if g.components.iter().any(|other| other.name == name) {
            return fail(g, f.loc, format!("duplicate component `{name}`"));
        }
        g.extra_globals.push(name);
        g.components.push(component);
    }
    Ok(())
}

/// `TodoRow` → `TodoRow::build` (the builder inside the component's struct).
pub fn builder_name(g: &mut Generator<'_>, name: &[u8]) -> Result<String> {
    if name.is_empty() || !name[0].is_ascii_uppercase() {
        return fail(
            g,
            0,
            format!(
                "component `{}` must start with a capital letter",
                String::from_utf8_lossy(name)
            ),
        );
    }
    Ok(format!("{}::build", rust_text::id_at(g, name, 0)?))
}

pub fn find<'a>(g: &'a Generator<'_>, name: &str) -> Option<&'a Component> {
    g.components.iter().find(|component| component.name == name)
}

fn field_type(g: &Generator<'_>, props: Type, name: &[u8]) -> Option<Type> {
    let entry = model::info(g, props)?;
    entry
        .fields
        .iter()
        .find(|field| field.name == name)
        .map(|field| field.ty)
}

fn fields(g: &Generator<'_>, component: &Component) -> Vec<model::Field> {
    match component.props {
        Some(props) => model::info(g, props)
            .map(|info| info.fields)
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

// ── inside a child component ──────────────────────────────────────────

/// Reads a prop of the component being generated: `self.props.todo`, or the
/// whole object for a non-destructured `props`.
pub fn read_prop(g: &mut Generator<'_>, n: &str) -> Result<Option<Value>> {
    let Some(index) = g.current_component else {
        return Ok(None);
    };
    let component = g.components[index].clone();
    let Some(props) = component.props else {
        return Ok(None);
    };
    for (i, d) in component.destructure.iter().enumerate() {
        if String::from_utf8_lossy(&d.name) != n {
            continue;
        }
        g.component_props_read = true;
        let ty = field_type(g, props, &d.name).expect("destructured prop");
        let code = format!("self.props.{}", rust_text::id_at(g, &d.name, 0)?);
        if let Some(default) = component
            .destructure_defaults
            .get(i)
            .and_then(|default| default.as_ref())
        {
            return Ok(Some(functions::prop(g, d, default, ty, &code)?));
        }
        return Ok(Some(super::scope::owned_read(g, ty, code)));
    }
    if component.destructure.is_empty() && component.param == n {
        g.component_props_read = true;
        return Ok(Some(super::scope::owned_read(
            g,
            props,
            "self.props".to_owned(),
        )));
    }
    Ok(None)
}

/// `on_toggle(todo.id)` / `props.on_toggle(todo.id)` inside a child: calls the
/// callback prop. Null when the callee is not a callback prop.
fn props_callback(g: &mut Generator<'_>, m: &ast::Member) -> Result<bool> {
    let ast::ExprData::Ident(object) = &m.object.data else {
        return Ok(false);
    };
    let name = String::from_utf8_lossy(object).into_owned();
    let Some(object) = read_prop(g, &name)? else {
        return Ok(false);
    };
    let Some(entry) = model::info(g, object.ty) else {
        return Ok(false);
    };
    if entry.kind != model::InfoKind::Object {
        return Ok(false);
    }
    let Some(ty) = field_type(g, object.ty, &m.name.name) else {
        return Ok(false);
    };
    Ok(callback_info(g, ty).is_some())
}

pub fn try_call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<Value>> {
    if g.current_component.is_none() {
        return Ok(None);
    }
    let callee_is_prop = match &call.callee.data {
        ast::ExprData::Ident(name) => {
            scope::local(c, name).is_none()
                && read_prop(g, &String::from_utf8_lossy(name))?.is_some()
        }
        // `props.onToggle(...)`, not an array method on a prop (`items.filter(...)`)
        ast::ExprData::Member(m) => match &m.object.data {
            ast::ExprData::Ident(object) => {
                scope::local(c, object).is_none() && props_callback(g, m)?
            }
            _ => false,
        },
        _ => false,
    };
    if !callee_is_prop {
        return Ok(None);
    }
    let callee = expr::emit(g, c, &call.callee, None, false)?;
    let Some(entry) = callback_info(g, callee.ty) else {
        return Ok(None);
    };
    if model::optional_child(g, callee.ty).is_some() {
        return fail(
            g,
            loc,
            "optional callback props may be null; check first: `if (onX) onX(...)`",
        );
    }
    if c.render {
        return fail(
            g,
            loc,
            "callback props can only be called in event handlers",
        );
    }
    if call.args.len() != entry.params.len() {
        return fail(
            g,
            loc,
            format!("the callback takes {} arguments", entry.params.len()),
        );
    }
    let mut args: Vec<String> = Vec::new();
    for (arg, ty) in call.args.iter().zip(&entry.params) {
        args.push(expr::emit(g, c, arg, Some(*ty), false)?.code);
    }
    // The callback runs synchronously: the runtime `ActionCx` of this
    // handler is passed straight through (the handler takes it because it
    // calls a callback prop — see `action_cx`).
    let Some(cx) = c.cx_var.clone() else {
        return fail(
            g,
            loc,
            "a callback prop can only be called in an event handler",
        );
    };
    let kind = action_kind(&entry.params);
    let mut joined = cx.clone();
    for (value, ty) in args.iter().zip(&entry.params) {
        // `ui::text_action` hands the handler `&str`
        let value = if kind == Some(tags::Kind::TextAction) && *ty == Type::String {
            format!("({value}).as_str()")
        } else {
            value.clone()
        };
        joined.push_str(", ");
        joined.push_str(&value);
    }
    Ok(Some(Value {
        ty: Type::Void,
        code: match kind {
            Some(_) => format!("({}).call({joined})", callee.code),
            None => format!("({})({joined})", callee.code),
        },
        ..Value::default()
    }))
}

/// Whether `e` is this component's callback prop (`on_save` or
/// `props.on_save`) with the signature of the ui event `kind`.
pub fn forwarded_prop(
    g: &mut Generator<'_>,
    e: &ast::Expr,
    kind: tags::Kind,
    loc: ast::Loc,
) -> Result<bool> {
    if g.current_component.is_none() {
        return Ok(false);
    }
    let ty = match &e.data {
        ast::ExprData::Ident(name) => match read_prop(g, &String::from_utf8_lossy(name))? {
            Some(value) => value.ty,
            None => return Ok(false),
        },
        ast::ExprData::Member(m) => {
            let ast::ExprData::Ident(object) = &m.object.data else {
                return Ok(false);
            };
            let Some(object) = read_prop(g, &String::from_utf8_lossy(object))? else {
                return Ok(false);
            };
            match field_type(g, object.ty, &m.name.name) {
                Some(ty) => ty,
                None => return Ok(false),
            }
        }
        _ => return Ok(false),
    };
    let Some(entry) = callback_info(g, ty) else {
        return Ok(false);
    };
    if model::optional_child(g, ty).is_some() {
        return fail(
            g,
            loc,
            "an optional callback prop cannot be passed to a ui event; make it required",
        );
    }
    if action_kind(&entry.params) != Some(kind) {
        return fail(
            g,
            loc,
            "this callback prop does not have the event's signature",
        );
    }
    Ok(true)
}

/// A forwarded callback prop wrapped as the ui action of an event
/// (`on_click: Some(ui::Action::new(move |cx| callback.call(cx)))`).
pub fn forwarded_action_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    kind: tags::Kind,
) -> Result<String> {
    let callee = expr::emit(g, c, e, None, true)?;
    Ok(match kind {
        tags::Kind::Action => format!(
            "ui::Action::new({{\n    let callback = {};\n    move |cx| callback.call(cx)\n}})",
            callee.code
        ),
        tags::Kind::TextAction => format!(
            "ui::TextAction::new({{\n    let callback = {};\n    move |cx, value| callback.call(cx, value)\n}})",
            callee.code
        ),
        tags::Kind::BoolAction => format!(
            "ui::BoolAction::new({{\n    let callback = {};\n    move |cx, checked| callback.call(cx, checked)\n}})",
            callee.code
        ),
        tags::Kind::IndexAction => format!(
            "ui::IndexAction::new({{\n    let callback = {};\n    move |cx, index| callback.call(cx, index)\n}})",
            callee.code
        ),
        _ => callee.code,
    })
}

// ── call sites ────────────────────────────────────────────────────────

/// Registers inline arrows passed to callback props (before emission, like ui
/// event handlers) and scans everything else.
pub fn scan(g: &mut Generator<'_>, j: &ast::Jsx, component: &Component) -> Result<()> {
    for attr in spread::literals(g, &j.attrs)? {
        if attr.spread {
            if let Some(value) = attr.value.as_deref() {
                scan_expr_via(g, value)?;
            }
            continue;
        }
        let Some(value) = attr.value.as_deref() else {
            continue;
        };
        let Some(props) = component.props else {
            continue;
        };
        let Some(ty) = field_type(g, props, &attr.name.name) else {
            continue;
        };
        let Some(entry) = callback_info(g, ty) else {
            scan_expr_via(g, value)?;
            continue;
        };
        let function: Option<Box<ast::Function>> = match &value.data {
            ast::ExprData::Arrow(function) => Some(function.clone()),
            ast::ExprData::Ident(name) => {
                let name = String::from_utf8_lossy(name).into_owned();
                if let Some(function) = inline_function(g, &name) {
                    Some(Box::new(function))
                } else {
                    // a plain function of this file or an imported one, or a
                    // state setter: `(v) => fn(v)`
                    let plain = scope::source_method(g, name.as_bytes()).is_none()
                        && read_prop(g, &name)?.is_none()
                        && (functions::find_index(g, name.as_bytes()).is_some()
                            || scope::setter_index(g, name.as_bytes()).is_some());
                    if !plain {
                        continue;
                    }
                    Some(Box::new(forward_arrow(g, value, entry.params.len())?))
                }
            }
            _ => continue,
        };
        let Some(function) = function else {
            continue;
        };
        g.counter += 1;
        let name = scope::unique_method(
            g,
            &format!("_{}{}", String::from_utf8_lossy(&attr.name.name), g.counter),
        )?;
        let capture_name = scope::unique_method(g, &format!("_Capture{}", g.counter))?;
        if function.params.len() > entry.params.len() {
            return fail(
                g,
                value.loc,
                format!("the callback passes {} arguments", entry.params.len()),
            );
        }
        let index = g.methods.len();
        g.methods.push(crate::codegen::Method {
            name,
            func: Some(function.clone()),
            action: action_kind(&entry.params),
            callback_params: Some(entry.params.clone()),
            capture_name: Some(capture_name),
            ..crate::codegen::Method::default()
        });
        g.callback_events.push(CallbackEvent {
            expr: value.clone(),
            method: index,
        });
        scan_body_via(g, &function.body)?;
    }
    Ok(())
}

fn scan_expr_via(g: &mut Generator<'_>, e: &ast::Expr) -> Result<()> {
    match g.extensions.hooks.scan_expr {
        Some(hook) => hook(g, e),
        // `events::scan_expr` scans nested handlers; without the hook
        // installed there is nothing to scan.
        None => Ok(()),
    }
}

fn scan_body_via(g: &mut Generator<'_>, body: &ast::Body) -> Result<()> {
    if let Some(hook) = g.extensions.hooks.scan_body {
        return hook(g, body);
    }
    Ok(())
}

/// A component function that reads render locals (`codegen.inlineFunction`).
fn inline_function(g: &Generator<'_>, name: &str) -> Option<ast::Function> {
    let index = g
        .methods
        .iter()
        .position(|method| method.user && method.name == name && method.inline_only)?;
    g.methods[index].func.as_deref().cloned()
}

fn forward_arrow(g: &mut Generator<'_>, e: &ast::Expr, count: usize) -> Result<ast::Function> {
    let mut params: Vec<ast::Param> = Vec::new();
    let mut args: Vec<Box<ast::Expr>> = Vec::new();
    for index in 0..count {
        let name = format!("_a{index}");
        params.push(ast::Param {
            name: ast::Name {
                name: name.as_bytes().to_vec(),
                loc: e.loc,
            },
            ty: None,
            optional: false,
            destructure: Vec::new(),
            destructure_defaults: Vec::new(),
            default: None,
        });
        args.push(Box::new(ast::Expr {
            loc: e.loc,
            data: ast::ExprData::Ident(name.into_bytes()),
        }));
    }
    let _ = g;
    Ok(ast::Function {
        loc: e.loc,
        name: Vec::new(),
        params,
        ret: None,
        body: ast::Body::Expr(Box::new(ast::Expr {
            loc: e.loc,
            data: ast::ExprData::Call(ast::Call {
                callee: Box::new(e.clone()),
                type_args: Vec::new(),
                args,
            }),
        })),
    })
}

/// `<TodoRow todo={t} onToggle={toggle} />` → `TodoRow::build(Props { .. }, id)`.
pub fn emit_call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    j: &ast::Jsx,
    component: &Component,
) -> Result<Value> {
    let mut parts: Vec<String> = Vec::new();
    let mut key: Option<String> = None;
    let mut given: Vec<Vec<u8>> = Vec::new();
    let mut raw_key_code: Option<String> = None;
    let mut raw_key_is_string = false;
    for attr in spread::flatten(g, c, &j.attrs)? {
        if attr.name.name == b"key" {
            let Some(value) = attr.value.as_deref() else {
                return fail(g, attr.name.loc, "key needs a value");
            };
            let emitted = expr::emit(g, c, value, None, false)?;
            let concrete = if emitted.ty.numeric() {
                types::concrete(g, emitted.clone())?
            } else {
                emitted.clone()
            };
            let code = if emitted.ty == Type::String {
                concrete.code.clone()
            } else if emitted.ty.numeric() {
                format!("zant::Str::copy(&format!(\"{{}}\", {}))", concrete.code)
            } else {
                return fail(g, value.loc, "key must be a string or a number");
            };
            key = Some(code);
            raw_key_is_string = emitted.ty == Type::String;
            raw_key_code = Some(concrete.code);
            continue;
        }
        let ty = match component.props {
            Some(props) => field_type(g, props, &attr.name.name),
            None => None,
        };
        let Some(ty) = ty else {
            return fail(
                g,
                attr.name.loc,
                format!(
                    "unknown prop `{}` on <{}>; props: {}",
                    String::from_utf8_lossy(&attr.name.name),
                    component.name,
                    field_list(g, component)
                ),
            );
        };
        let code = match attr.value.as_deref() {
            Some(value) => prop_value(g, c, ty, value)?,
            None if ty == Type::Boolean => "true".to_owned(),
            None if model::optional_child(g, ty) == Some(Type::Boolean) => "Some(true)".to_owned(),
            None => {
                return fail(
                    g,
                    attr.name.loc,
                    format!(
                        "bare attribute `{}` is only valid for boolean props",
                        String::from_utf8_lossy(&attr.name.name)
                    ),
                );
            }
        };
        parts.push(format!(
            "{}: {},\n",
            rust_text::id_at(g, &attr.name.name, attr.name.loc)?,
            code
        ));
        given.push(attr.name.name.clone());
    }
    if let Some(code) = children_value(g, c, j, component)? {
        parts.push(format!("children: {code},\n"));
        given.push(b"children".to_vec());
    }
    for field in fields(g, component) {
        if field.optional {
            continue;
        }
        if !given.iter().any(|name| name == &field.name) {
            return fail(
                g,
                j.loc,
                format!(
                    "<{}> needs the prop `{}`",
                    component.name,
                    String::from_utf8_lossy(&field.name)
                ),
            );
        }
    }
    if component.stateful && raw_key_code.is_none() && g.loop_depth > 0 {
        return fail(
            g,
            j.loc,
            format!(
                "<{}> keeps its own state; give each one in a list a key (key={{item.id}}) so the state follows the item",
                component.name
            ),
        );
    }
    let id = if component.stateful {
        Some(instance_id(
            g,
            component,
            raw_key_code.as_deref(),
            raw_key_is_string,
        )?)
    } else {
        None
    };
    let builder = component.builder.clone();
    let props_name = match component.props {
        Some(props) => props.rust(g).to_owned(),
        None => "Default::default()".to_owned(),
    };
    let value = if component.props.is_some() {
        // omitted props keep their struct defaults (`Default::default()`)
        let all_given = fields(g, component)
            .iter()
            .all(|field| given.iter().any(|name| name == &field.name));
        if all_given {
            format!("{props_name} {{\n{}}}", parts.join(""))
        } else {
            format!(
                "{props_name} {{\n{}    ..Default::default()\n}}",
                parts.join("")
            )
        }
    } else {
        props_name
    };
    let mut node = match id {
        Some(id) => format!("{builder}({value}, {id})"),
        None => format!("{builder}({value})"),
    };
    if let Some(key) = key {
        node = format!("ui::with_key({node}, {key})");
    }
    Ok(Value {
        ty: Type::Node,
        code: node,
        ..Value::default()
    })
}

/// `, tsx::child_id(parent, "TodoRow", key)`: the parent's instance id (the
/// root's is 0) mixed with the component name and its key; without a key,
/// `tsx::site_id(parent, "TodoRow", n)` numbers the call site.
fn instance_id(
    g: &mut Generator<'_>,
    component: &Component,
    key: Option<&str>,
    key_is_string: bool,
) -> Result<String> {
    let parent = if g.current_component.is_some() {
        // inside a `<List>` row closure the id is hoisted (`list.rs`)
        if g.handler_state.is_some() {
            "_parent_id"
        } else {
            "self._id"
        }
    } else {
        "0"
    };
    if let Some(key) = key {
        let key = if key_is_string {
            format!("({key}).as_str()")
        } else {
            key.to_owned()
        };
        return Ok(format!(
            "tsx::child_id({parent}, \"{}\", {key})",
            component.name
        ));
    }
    g.child_sites += 1;
    Ok(format!(
        "tsx::site_id({parent}, \"{}\", {})",
        component.name, g.child_sites
    ))
}

fn field_list(g: &Generator<'_>, component: &Component) -> String {
    let names: Vec<String> = fields(g, component)
        .iter()
        .map(|field| String::from_utf8_lossy(&field.name).into_owned())
        .collect();
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(", ")
    }
}

/// JSX children go to a `children` prop: one element for `ReactNode`, a list
/// for `ReactNode[]` (elements, `{cond && <X/>}` and arrays such as
/// `{todos.map(…)}` mixed).
fn children_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    j: &ast::Jsx,
    component: &Component,
) -> Result<Option<String>> {
    let declared = match component.props {
        Some(props) => field_type(g, props, b"children"),
        None => None,
    };
    if let Some(ty) = declared
        && model::optional_child(g, ty).unwrap_or(ty) == Type::String
    {
        return text_children(g, c, j, component);
    }
    struct Part {
        code: String,
        ty: Type,
    }
    let mut parts: Vec<Part> = Vec::new();
    for child in &j.children {
        match child {
            ast::Child::Text(t) => {
                return fail(g, t.loc, "text must be inside <Text>");
            }
            ast::Child::Element(el) => {
                let e = ast::Expr {
                    loc: el.loc,
                    data: ast::ExprData::Jsx(el.clone()),
                };
                let value = expr::emit(g, c, &e, Some(Type::Node), false)?;
                parts.push(Part {
                    code: value.code,
                    ty: Type::Node,
                });
            }
            ast::Child::Expr(Some(e)) => {
                let value = expr::emit(g, c, e, None, true)?;
                if value.ty != Type::Node
                    && value.ty != Type::OptionalNode
                    && model::element(g, value.ty) != Some(Type::Node)
                {
                    return fail(g, e.loc, "children must be elements");
                }
                parts.push(Part {
                    code: value.code,
                    ty: value.ty,
                });
            }
            ast::Child::Expr(None) => {}
        }
    }
    if parts.is_empty() {
        return Ok(None);
    }
    let Some(ty) = declared else {
        return fail(
            g,
            j.loc,
            format!("<{}> has no `children` prop", component.name),
        );
    };
    let base = model::optional_child(g, ty).unwrap_or(ty);
    if base == Type::Node {
        if parts.len() != 1 || parts[0].ty != Type::Node {
            return fail(
                g,
                j.loc,
                format!("<{}> takes one child element", component.name),
            );
        }
        return Ok(Some(parts[0].code.clone()));
    }
    if model::element(g, base) != Some(Type::Node) {
        return fail(
            g,
            j.loc,
            "`children` must be typed ReactNode or ReactNode[]",
        );
    }
    let codes: Vec<String> = parts
        .into_iter()
        .map(|part| {
            if part.ty == ty {
                // a `children: ReactNode[]` value is a `tsx::List<Node>`
                format!("({}).as_slice().to_vec()", part.code)
            } else {
                part.code
            }
        })
        .collect();
    Ok(Some(format!(
        "tsx::List::from_vec(nodes![{}])",
        codes.join(", ")
    )))
}

/// `children: string`: `<Badge>new {count}</Badge>` passes its text.
fn text_children(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    j: &ast::Jsx,
    component: &Component,
) -> Result<Option<String>> {
    let mut format = rust_text::Format::default();
    let mut any = false;
    for child in &j.children {
        match child {
            ast::Child::Text(t) => {
                format.text(&t.value);
                any = true;
            }
            ast::Child::Expr(Some(e)) => {
                let expression = nesting::text_expression(g, e)?;
                let value = expr::emit(g, c, &expression, None, false)?;
                format.value(g, value, e.loc)?;
                any = true;
            }
            ast::Child::Element(el) => {
                return fail(
                    g,
                    el.loc,
                    format!(
                        "<{}> can't be inside <{}>; its children are text (`children: string`)",
                        if el.name.is_empty() {
                            "a fragment".to_owned()
                        } else {
                            String::from_utf8_lossy(&el.name).into_owned()
                        },
                        component.name
                    ),
                );
            }
            ast::Child::Expr(None) => {}
        }
    }
    if !any {
        return Ok(None);
    }
    Ok(Some(format.finish(g, false)?))
}

fn prop_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    ty: Type,
    value: &ast::Expr,
) -> Result<String> {
    if let Some(entry) = callback_info(g, ty) {
        if matches!(value.data, ast::ExprData::NullLit) && model::optional_child(g, ty).is_some() {
            return Ok("None".to_owned());
        }
        return callback_value(g, c, &entry, value);
    }
    Ok(expr::emit(g, c, value, Some(ty), false)?.code)
}

/// A function, an inline arrow or a callback prop of this component, bound as
/// the callback type of the target prop.
fn callback_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    entry: &model::Info,
    value: &ast::Expr,
) -> Result<String> {
    let method_index: usize;
    match &value.data {
        ast::ExprData::Arrow(_) => {
            let Some(index) = g
                .callback_events
                .iter()
                .find(|event| event.expr == *value)
                .map(|event| event.method)
            else {
                unreachable!("registered by `scan`")
            };
            method_index = index;
            captures::prepare(g, c, method_index)?;
        }
        ast::ExprData::Ident(name) => {
            if let Some(index) = g
                .callback_events
                .iter()
                .find(|event| event.expr == *value)
                .map(|event| event.method)
            {
                method_index = index;
                captures::prepare(g, c, method_index)?;
            } else {
                let prop = read_prop(g, &String::from_utf8_lossy(name))?;
                if let Some(prop) = prop {
                    // forwarding this component's own callback prop
                    let same = callback_info(g, prop.ty);
                    if same.as_ref().is_none_or(|info| info.params != entry.params) {
                        return fail(
                            g,
                            value.loc,
                            format!(
                                "`{}` has a different signature",
                                String::from_utf8_lossy(name)
                            ),
                        );
                    }
                    return Ok(prop.code);
                }
                let Some(index) = scope::source_method(g, name) else {
                    return fail(
                        g,
                        value.loc,
                        "callback props take a component function, an inline arrow or a callback prop",
                    );
                };
                method_index = index;
                let Some(function) = g.methods[index].func.clone() else {
                    return fail(
                        g,
                        value.loc,
                        format!(
                            "pass an arrow for setters: (v) => {}(v)",
                            String::from_utf8_lossy(name)
                        ),
                    );
                };
                if function.params.len() != entry.params.len() {
                    return fail(
                        g,
                        value.loc,
                        format!(
                            "`{}` must take {} arguments",
                            String::from_utf8_lossy(name),
                            entry.params.len()
                        ),
                    );
                }
            }
        }
        _ => {
            let v = expr::emit(g, c, value, None, false)?;
            let Some(same) = callback_info(g, v.ty) else {
                return fail(
                    g,
                    value.loc,
                    "callback props take a component function, an inline arrow or a callback prop",
                );
            };
            if same.params != entry.params {
                return fail(g, value.loc, "this callback has a different signature");
            }
            return Ok(v.code);
        }
    }
    match action_kind(&entry.params) {
        Some(kind) => super::generate::action_value(g, c, method_index, Some(kind)),
        None => super::generate::callback_value(g, c, method_index, &entry.params),
    }
}
