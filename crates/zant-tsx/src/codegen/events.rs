//! Event signatures, inferred parameters, generated handlers, and reusable
//! setter adapters.
//!
//! Handler discovery runs before emission (so names and inferred signatures are
//! stable); the JSX emitter then refers to the registered method. The
//! generated value is a closure,
//! `Some(ui::text_action(|s: &mut Counter, v: &str| s._set_name(v)))`, and the
//! method signature lives on the component.
//!
//! Event expressions are identified by AST address: nodes of the parsed module
//! are stable while a component is generated.

use crate::ast;
use crate::tags::{self, Kind};

use super::app_state;
use super::expr;
use super::functions;
use super::model;
use super::rust_text;
use super::scope;
use super::types::{self, Type};
use super::{Generator, Method, Result, fail};

/// A handler expression registered before emission.
///
/// The AST owns its nodes and the spread / JSX shape helpers clone
/// expressions, so a node's address is not stable there; the source location
/// is: a clone keeps it, and one source position holds at most one expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub loc: ast::Loc,
    pub kind: Kind,
    pub method: usize,
}

/// Register event methods before local allocation, so names and inferred
/// signatures are stable.
pub fn scan_expr(g: &mut Generator<'_>, e: &ast::Expr) -> Result<()> {
    match &e.data {
        ast::ExprData::Jsx(element) => scan_jsx(g, element),
        ast::ExprData::Binary(binary) => {
            scan_expr(g, &binary.lhs)?;
            scan_expr(g, &binary.rhs)
        }
        ast::ExprData::Unary(unary) => scan_expr(g, &unary.operand),
        ast::ExprData::NonNull(inner) => scan_expr(g, inner),
        ast::ExprData::As(cast) => scan_expr(g, &cast.expr),
        ast::ExprData::Index(index) => {
            scan_expr(g, &index.object)?;
            scan_expr(g, &index.index)
        }
        ast::ExprData::Member(member) => scan_expr(g, &member.object),
        ast::ExprData::Spread(inner) => scan_expr(g, inner),
        ast::ExprData::Array(items) => {
            for item in items {
                scan_expr(g, item)?;
            }
            Ok(())
        }
        ast::ExprData::Cond(value) => scan_conditional(g, value),
        ast::ExprData::Call(call) => {
            scan_timer(g, call)?;
            // arguments (including inline arrows) still register handlers
            for arg in &call.args {
                scan_expr(g, arg)?;
            }
            Ok(())
        }
        ast::ExprData::Template(template) => {
            for expr in &template.exprs {
                scan_expr(g, expr)?;
            }
            Ok(())
        }
        ast::ExprData::Arrow(function) => scan_body(g, &function.body),
        ast::ExprData::Assign(assign) => scan_expr(g, &assign.value),
        ast::ExprData::Object(properties) => {
            for property in properties {
                scan_expr(g, &property.value)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The name of a plain call (`setTimeout(…)`) or `None`.
fn call_ident(call: &ast::Call, name: &str) -> bool {
    matches!(&call.callee.data, ast::ExprData::Ident(n) if String::from_utf8_lossy(n) == name)
}

/// `setTimeout(() => …, ms)` registers its callback as a method while the
/// component is scanned. The lowering lives here because the callback behaves
/// like an event arrow (captures, no parameters).
fn scan_timer(g: &mut Generator<'_>, call: &ast::Call) -> Result<()> {
    if !call_ident(call, "setTimeout") {
        return Ok(());
    }
    let loc = call.args.first().map_or(0, |arg| arg.loc);
    if call.args.len() != 2 {
        return fail(g, loc, "setTimeout(() => …, ms)");
    }
    let callback = &call.args[0];
    let arrow = match &callback.data {
        ast::ExprData::Arrow(function) => ast::Function {
            type_params: function.type_params.clone(),
            loc: callback.loc,
            name: Vec::new(),
            params: function.params.clone(),
            ret: function.ret.clone(),
            body: function.body.clone(),
        },
        ast::ExprData::Ident(_) => {
            let forwarded = forward_arrow(callback, 0);
            let ast::ExprData::Arrow(function) = forwarded.data else {
                unreachable!("forward arrow")
            };
            *function
        }
        _ => {
            return fail(
                g,
                callback.loc,
                "the callback is an arrow or a function name",
            );
        }
    };
    if !arrow.params.is_empty() {
        return fail(g, callback.loc, "a timer callback takes no parameters");
    }
    g.counter += 1;
    let method = g.methods.len();
    app_state::with_module(|state| {
        state.desktop_callbacks.push(app_state::DesktopCallback {
            expr: (**callback).clone(),
            method,
        })
    });
    let name = scope::unique_method(g, &format!("_timeout{}", g.counter))?;
    let capture_name = scope::unique_method(g, &format!("_TimeoutCapture{}", g.counter))?;
    g.methods.push(Method {
        name,
        func: Some(Box::new(arrow)),
        action: Some(Kind::Action),
        capture_name: Some(capture_name),
        ..Method::default()
    });
    Ok(())
}

/// Values a timer keeps after the frame that scheduled it is gone: numbers,
/// booleans and enums, and optionals of them.
fn timer_plain(g: &Generator<'_>, ty: Type) -> bool {
    if ty == Type::Boolean || ty.numeric() {
        return true;
    }
    if let Some(child) = model::optional_child(g, ty) {
        return timer_plain(g, child);
    }
    model::info(g, ty).is_some_and(|entry| entry.kind == model::InfoKind::Enumeration)
}

/// `setTimeout(…)` / `clearTimeout(id)` in a handler.
pub fn timer_call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<super::types::Value>> {
    let set = call_ident(call, "setTimeout");
    if !set && !call_ident(call, "clearTimeout") {
        return Ok(None);
    }
    let name = if set { "setTimeout" } else { "clearTimeout" };
    if super::builtins::shadowed(g, c, name) {
        return Ok(None);
    }
    if c.render || c.constant {
        return fail(
            g,
            loc,
            "{name} runs in event handlers and effects, not while rendering",
        );
    }
    if !set {
        if call.args.len() != 1 {
            return fail(g, loc, "clearTimeout(id)");
        }
        let id = expr::emit(g, c, &call.args[0], Some(Type::F64), false)?;
        let id = types::coerce(g, id, Type::F64, call.args[0].loc)?;
        return Ok(Some(super::types::Value::new(
            Type::Void,
            format!("tsx::timers::clear({});", id.code),
        )));
    }
    if c.no_self {
        return fail(
            g,
            loc,
            "setTimeout runs in a component's handlers (a plain function has none)",
        );
    }
    if let Some(component) = g.current_component
        && g.components[component].props.is_some()
    {
        return fail(
            g,
            loc,
            "timers belong to the root component or a window's (components without props): they outlive other components; set it there and pass the result down",
        );
    }
    if call.args.len() != 2 {
        return fail(g, loc, "setTimeout(() => …, ms)");
    }
    let callback = &call.args[0];
    let Some(info) = app_state::with_module(|state| {
        state
            .desktop_callbacks
            .iter()
            .find(|item| item.expr == **callback)
            .cloned()
    }) else {
        return fail(g, callback.loc, "the timer callback was not scanned");
    };
    super::captures::prepare(g, c, info.method)?;
    let method = g.methods[info.method].clone();
    // the capture struct is built where `setTimeout` is called: re-read each
    // value in the enclosing context (`cap.doubled` inside an effect, a local
    // of the handler, …) instead of the render binding the snapshot recorded
    for (index, capture) in method.captures.iter().enumerate() {
        if let Some(value) = super::captures::read(g, c, &capture.name)? {
            g.methods[info.method].captures[index].source = value.code;
        }
    }
    for capture in &method.captures {
        if !timer_plain(g, capture.ty) {
            return fail(
                g,
                callback.loc,
                format!(
                    "a timer callback can capture numbers, booleans and enums (`{}` is not one); keep other values in state and read them when it runs",
                    capture.name
                ),
            );
        }
    }
    let action = super::generate::action_value(g, c, info.method, Some(Kind::Action))?;
    let ms = expr::emit(g, c, &call.args[1], Some(Type::F64), false)?;
    let ms = types::coerce(g, ms, Type::F64, call.args[1].loc)?;
    Ok(Some(super::types::Value::new(
        Type::F64,
        format!("tsx::timers::set({}, {action})", ms.code),
    )))
}

/// Register attribute callbacks before recursively scanning JSX children.
pub fn scan_jsx(g: &mut Generator<'_>, element: &ast::Jsx) -> Result<()> {
    let name = String::from_utf8_lossy(&element.name).into_owned();
    if let Some(component) = super::components::find(g, &name) {
        let component = component.clone();
        super::components::scan(g, element, &component)?;
    }
    if element.name == b"List" {
        return super::list::scan(g, element);
    }
    if let Some(tag) = tags::find(&String::from_utf8_lossy(&element.name)) {
        if super::jsx::spread_any(&element.attrs) {
            let attrs = super::jsx::spread_literals(g, &element.attrs)?;
            for attr in &attrs {
                scan_attribute(g, tag, attr)?;
            }
        } else {
            // Original attribute nodes: handler expressions keep the
            // addresses the emitter will look up.
            for attr in &element.attrs {
                scan_attribute(g, tag, attr)?;
            }
        }
    }
    for child in &element.children {
        match child {
            ast::Child::Element(element) => scan_jsx(g, element)?,
            ast::Child::Expr(Some(value)) => scan_expr(g, value)?,
            ast::Child::Expr(None) | ast::Child::Text(_) => {}
        }
    }
    Ok(())
}

/// One tag attribute for handler discovery: `style` objects recurse into the
/// flattened option fields, action props register their handler.
fn scan_attribute(g: &mut Generator<'_>, tag: &tags::Tag, attr: &ast::Attr) -> Result<()> {
    let Some(value) = attr.value.as_deref() else {
        return Ok(());
    };
    if attr.name.name == b"style"
        && let ast::ExprData::Object(properties) = &value.data
    {
        for property in properties {
            if property.spread {
                continue;
            }
            if let Some(prop) = tags::find_prop(tag, &String::from_utf8_lossy(&property.key.name)) {
                scan_property(g, prop, &property.value, property.key.loc)?;
            } else {
                scan_expr(g, &property.value)?;
            }
        }
        return Ok(());
    }
    if let Some(prop) = tags::find_prop(tag, &String::from_utf8_lossy(&attr.name.name)) {
        scan_property(g, prop, value, attr.name.loc)
    } else {
        scan_expr(g, value)
    }
}

pub fn scan_body(g: &mut Generator<'_>, body: &ast::Body) -> Result<()> {
    match body {
        ast::Body::Expr(e) => scan_expr(g, e),
        ast::Body::Block(ss) => {
            for s in ss {
                scan_stmt(g, s)?;
            }
            Ok(())
        }
    }
}

pub fn scan_stmt(g: &mut Generator<'_>, s: &ast::Stmt) -> Result<()> {
    match &s.data {
        ast::StmtData::For(value) => {
            if let Some(init) = value.init.as_deref() {
                scan_stmt(g, init)?;
            }
            if let Some(cond) = value.cond.as_deref() {
                scan_expr(g, cond)?;
            }
            if let Some(update) = value.update.as_deref() {
                scan_expr(g, update)?;
            }
            scan_stmt(g, &value.body)
        }
        ast::StmtData::ForOf(value) => {
            scan_expr(g, &value.iterable)?;
            scan_stmt(g, &value.body)
        }
        ast::StmtData::While(value) => {
            scan_expr(g, &value.cond)?;
            scan_stmt(g, &value.body)
        }
        ast::StmtData::Switch(value) => {
            scan_expr(g, &value.value)?;
            for case in &value.cases {
                if let Some(test) = case.value.as_deref() {
                    scan_expr(g, test)?;
                }
                for child in &case.body {
                    scan_stmt(g, child)?;
                }
            }
            Ok(())
        }
        ast::StmtData::Break | ast::StmtData::Continue => Ok(()),
        ast::StmtData::Decl(decl) => match decl.init.as_deref() {
            Some(init) => scan_expr(g, init),
            None => Ok(()),
        },
        ast::StmtData::Func(function) => scan_body(g, &function.body),
        ast::StmtData::Ret(e) => match e.as_deref() {
            Some(value) => scan_expr(g, value),
            None => Ok(()),
        },
        ast::StmtData::Expr(e) => scan_expr(g, e),
        ast::StmtData::Throw(e) => scan_expr(g, e),
        ast::StmtData::Try(t) => {
            for s in t
                .body
                .iter()
                .chain(t.catch.iter().flat_map(|c| c.body.iter()))
                .chain(t.finally.iter().flatten())
            {
                scan_stmt(g, s)?;
            }
            Ok(())
        }
        ast::StmtData::Block(ss) => {
            for s in ss {
                scan_stmt(g, s)?;
            }
            Ok(())
        }
        ast::StmtData::If(value) => {
            scan_expr(g, &value.cond)?;
            scan_stmt(g, &value.then)?;
            match value.else_.as_deref() {
                Some(otherwise) => scan_stmt(g, otherwise),
                None => Ok(()),
            }
        }
    }
}

/// Validate the action signature of a registered method.
pub fn signature(
    g: &mut Generator<'_>,
    m: &Method,
    kind: Kind,
    loc: ast::Loc,
    inline_: bool,
) -> Result<()> {
    let Some(function) = m.func.as_ref() else {
        return fail(
            g,
            loc,
            format!("handler must have signature {}", action_signature(kind)),
        );
    };
    let count = if kind == Kind::Action { 0 } else { 1 };
    if m.ret != Type::Void
        || function.params.len() > count
        || (!inline_ && function.params.len() != count)
    {
        return fail(
            g,
            loc,
            format!("handler must have signature {}", action_signature(kind)),
        );
    }
    for param in &function.params {
        if let Some(reference) = param.ty.as_ref()
            && types::type_ref(g, reference, false)? != action_type(kind)
        {
            return fail(
                g,
                param.name.loc,
                format!("handler must have signature {}", action_signature(kind)),
            );
        }
    }
    Ok(())
}

/// Validate the action signature and reuse one adapter per setter and action kind.
pub fn event(g: &mut Generator<'_>, e: &ast::Expr, prop: &tags::Prop, loc: ast::Loc) -> Result<()> {
    // `ready ? save : undefined`: each present branch is a handler of its own
    // (the branch is picked at render time, `optional_event_value`)
    if let ast::ExprData::Cond(cond) = &e.data
        && optional_event(prop.kind)
    {
        for branch in [&cond.then, &cond.else_] {
            if !absent(branch) {
                event(g, branch, prop, loc)?;
            }
        }
        return Ok(());
    }
    // a child component's callback prop forwarded to a ui event is emitted
    // directly (`onClick={onSave}`), nothing to register
    if (optional_event(prop.kind)
        && super::components::optional_forwarded_prop(g, e, prop.kind, loc)?)
        || super::components::forwarded_prop(g, e, prop.kind, loc)?
    {
        return Ok(());
    }
    for previous in &g.events {
        if previous.loc == e.loc && previous.kind == prop.kind {
            return Ok(());
        }
    }
    let index: usize;
    match &e.data {
        ast::ExprData::Ident(name) => {
            if let Some(setter) = scope::setter_index(g, name) {
                let ty = g.states[setter].ty;
                let valid = match prop.kind {
                    Kind::TextAction => ty == Type::String,
                    Kind::BoolAction => ty == Type::Boolean,
                    Kind::IndexAction => ty.integer() || ty.float(),
                    _ => false,
                };
                if !valid {
                    return fail(
                        g,
                        e.loc,
                        format!(
                            "setter `{}` does not match {}",
                            String::from_utf8_lossy(name),
                            action_signature(prop.kind)
                        ),
                    );
                }
                index = match g
                    .methods
                    .iter()
                    .position(|m| m.setter == Some(setter) && m.action == Some(prop.kind))
                {
                    Some(index) => index,
                    None => {
                        let name = scope::unique_method(
                            g,
                            &format!(
                                "_{}",
                                super::jsx::snake_case(&String::from_utf8_lossy(name))
                            ),
                        )?;
                        g.methods.push(Method {
                            name,
                            setter: Some(setter),
                            action: Some(prop.kind),
                            ..Method::default()
                        });
                        g.methods.len() - 1
                    }
                };
            } else if let Some(function) = inline_function(g, name) {
                // reads render locals: emitted here like an inline arrow
                let arrow = ast::Expr {
                    loc: e.loc,
                    data: ast::ExprData::Arrow(function),
                };
                index = register_arrow(g, prop, &arrow)?;
            } else if let (None, Some(function)) = (
                scope::source_method(g, name),
                functions::find_index(g, name),
            ) {
                // a plain function: handled as `(v) => fn(v)`, or `(v) => fn()`
                // when it ignores the event's value (TypeScript accepts a
                // callback with fewer parameters)
                let pass_value = !g.functions[function].params.is_empty();
                let arrow = function_arrow(e, prop.kind, pass_value);
                index = register_arrow(g, prop, &arrow)?;
            } else {
                let Some(method_index) = scope::source_method(g, name) else {
                    return fail(
                        g,
                        e.loc,
                        "event handler must be a component function, plain function, setter, or inline arrow",
                    );
                };
                let m = g.methods[method_index].clone();
                let ignores_value = prop.kind != Kind::Action
                    && m.func
                        .as_ref()
                        .is_some_and(|function| function.params.is_empty());
                if ignores_value {
                    // `function save() {}` on `onChange`: `(v) => save()`
                    let arrow = function_arrow(e, prop.kind, false);
                    index = register_arrow(g, prop, &arrow)?;
                    g.events.push(Event {
                        loc: e.loc,
                        kind: prop.kind,
                        method: index,
                    });
                    return Ok(());
                }
                signature(g, &m, prop.kind, loc, false)?;
                if let Some(previous) = m.action
                    && previous != prop.kind
                {
                    return fail(
                        g,
                        e.loc,
                        "handler is used with incompatible action signatures",
                    );
                }
                g.methods[method_index].action = Some(prop.kind);
                index = method_index;
            }
        }
        ast::ExprData::Arrow(_) => {
            index = register_arrow(g, prop, e)?;
        }
        ast::ExprData::Member(_) => {
            return fail(
                g,
                e.loc,
                "functions stored in objects can't be handlers yet; pass the function itself \
                 (`onClick={save}`) or call it in an inline arrow (`onClick={() => save()}`)",
            );
        }
        _ => {
            return fail(
                g,
                e.loc,
                "event handler must be a component function, plain function, setter, or inline arrow",
            );
        }
    }
    g.events.push(Event {
        loc: e.loc,
        kind: prop.kind,
        method: index,
    });
    Ok(())
}

/// Events whose field is an `Option<ui::…Action>` the value may leave empty.
fn optional_event(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Action | Kind::TextAction | Kind::BoolAction | Kind::IndexAction
    )
}

/// `undefined` / `null` where a handler goes: no handler.
fn absent(e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::NullLit => true,
        ast::ExprData::Ident(name) => name == b"undefined",
        _ => false,
    }
}

/// The `Option<ui::…Action>` of an event attribute. As in React the value is
/// evaluated at render time: `cond ? a : b` picks its branch then, an
/// `undefined` / `null` branch and an absent optional callback prop leave the
/// event empty.
pub fn optional_event_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    kind: Kind,
) -> Result<String> {
    if absent(e) {
        return Ok("None".to_owned());
    }
    if let ast::ExprData::Cond(cond) = &e.data {
        let test = expr::condition(g, c, &cond.cond)?.code;
        let then = optional_event_value(g, c, &cond.then, kind)?;
        let otherwise = optional_event_value(g, c, &cond.else_, kind)?;
        return Ok(format!("if {test} {{ {then} }} else {{ {otherwise} }}"));
    }
    if super::components::optional_forwarded_prop(g, e, kind, e.loc)? {
        return Ok(expr::emit(g, c, e, None, false)?.code);
    }
    Ok(format!("Some({})", event_value(g, c, e, kind)?))
}

/// `ui::action(..)` for a registered handler expression.
pub fn event_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    kind: Kind,
) -> Result<String> {
    // A child component's callback prop forwarded to a ui event already is
    // the action value (`onClick={props.onSave}`).
    if super::components::forwarded_prop(g, e, kind, e.loc)? {
        return Ok(expr::emit(g, c, e, None, false)?.code);
    }
    let events = g.events.clone();
    for event in events {
        if event.loc != e.loc || event.kind != kind {
            continue;
        }
        let method = g.methods[event.method].clone();
        if method.capture_name.is_some() {
            super::captures::prepare(g, c, event.method)?;
        }
        let method = g.methods[event.method].clone();
        let root = g.current_component.is_none();
        if root {
            // the root shape is `ui::action(|s: &mut App| s._on_click1())`;
            // a handler that reads render locals gets a capture struct first
            let Some(class) = rust_text::id(g.name.as_bytes()) else {
                return fail(g, e.loc, "component name is not valid UTF-8");
            };
            let target = rust_text::id_at(g, method.name.as_bytes(), e.loc)?;
            let (params, call_args) = match kind {
                Kind::Action => (String::new(), String::new()),
                Kind::TextAction => (", v: &str".to_owned(), "v".to_owned()),
                Kind::BoolAction => (", v: bool".to_owned(), "v".to_owned()),
                Kind::IndexAction => (", v: usize".to_owned(), "v".to_owned()),
                _ => {
                    return fail(
                        g,
                        e.loc,
                        "event handler must be a component function, setter, or inline arrow",
                    );
                }
            };
            if method.captures.is_empty() {
                return Ok(format!(
                    "ui::{}(|s: &mut {class}{params}| s.{target}({call_args}))",
                    action_builder(kind)
                ));
            }
            let capture = super::captures::value(g, &method)?;
            let arguments = if call_args.is_empty() {
                "&_capture".to_owned()
            } else {
                format!("&_capture, {call_args}")
            };
            return Ok(format!(
                "ui::{}({{\nlet _capture = {capture};\nmove |s: &mut {class}{params}| s.{target}({arguments})\n}})",
                action_builder(kind)
            ));
        }
        // a child component: the shared action emitter upgrades its local
        // state slot and passes captures
        return super::generate::action_value(g, c, event.method, Some(kind));
    }
    fail(
        g,
        e.loc,
        "event handler must be a component function, setter, or inline arrow",
    )
}

pub fn is_action(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Action | Kind::TextAction | Kind::BoolAction | Kind::IndexAction
    )
}

pub fn action_type(kind: Kind) -> Type {
    match kind {
        Kind::Action => Type::Void,
        Kind::TextAction => Type::String,
        Kind::BoolAction => Type::Boolean,
        Kind::IndexAction => Type::Usize,
        _ => Type::Void,
    }
}

/// The `ui::*` constructor of an action kind.
pub fn action_builder(kind: Kind) -> &'static str {
    match kind {
        Kind::Action => "action",
        Kind::TextAction => "text_action",
        Kind::BoolAction => "bool_action",
        Kind::IndexAction => "index_action",
        _ => "action",
    }
}

/// The error text of an expected action signature.
pub fn action_signature(kind: Kind) -> &'static str {
    match kind {
        Kind::Action => "() => void",
        Kind::TextAction => "(value: string) => void",
        Kind::BoolAction => "(checked: boolean) => void",
        Kind::IndexAction => "(index: usize) => void",
        _ => "() => void",
    }
}

/// `fn _setName(&mut self, value: &str)` for a `useState` setter used as a
/// handler. The component state is a plain field.
pub fn emit_setter(g: &mut Generator<'_>, m: &Method) -> Result<String> {
    let Some(setter) = m.setter else {
        return fail(g, 0, "setter method without a state");
    };
    let state = g.states[setter].clone();
    let field = scope::state_field(g, state.name.as_bytes())?;
    let name = rust_text::id_at(g, m.name.as_bytes(), 0)?;
    let kind = m.action.unwrap_or(Kind::Action);
    let (parameter, value) = match kind {
        Kind::TextAction => (
            "&str",
            if state.ty == Type::String {
                "zant::Str::copy(value)".to_owned()
            } else {
                return fail(
                    g,
                    0,
                    format!(
                        "setter `{}` takes a string but its state is not a string",
                        m.name
                    ),
                );
            },
        ),
        Kind::BoolAction => ("bool", "value".to_owned()),
        Kind::IndexAction => (
            "usize",
            if state.ty == Type::Usize {
                "value".to_owned()
            } else if state.ty.integer() {
                format!(
                    "value.try_into().expect(\"index setter does not fit {}\")",
                    state.ty.rust(g)
                )
            } else if state.ty.float() {
                format!("value as {}", state.ty.rust(g))
            } else {
                return fail(g, 0, format!("setter `{}` cannot take an index", m.name));
            },
        ),
        _ => {
            return fail(g, 0, format!("setter `{}` has no action signature", m.name));
        }
    };
    Ok(format!(
        "fn {name}(&mut self, value: {parameter}) {{\nself.{field} = {value};\n}}\n"
    ))
}

/// The type of a method's parameter `index`: from a callback prop's signature
/// when it has one, else as [`parameter_type`].
pub fn method_param_type(
    g: &mut Generator<'_>,
    m: &Method,
    index: usize,
    param: &ast::Param,
) -> Result<Type> {
    let Some(params) = m.callback_params.as_ref() else {
        return parameter_type(g, param, m.action);
    };
    if index >= params.len() {
        return fail(
            g,
            param.name.loc,
            format!("the callback passes {} arguments", params.len()),
        );
    }
    if let Some(reference) = param.ty.as_ref() {
        let ty = types::type_ref(g, reference, false)?;
        if ty != params[index] {
            return fail(
                g,
                param.name.loc,
                "parameter type does not match the callback prop",
            );
        }
        return Ok(ty);
    }
    Ok(params[index])
}

/// Prefer an annotation, then infer a parameter from its registered action kind.
pub fn parameter_type(
    g: &mut Generator<'_>,
    param: &ast::Param,
    kind: Option<Kind>,
) -> Result<Type> {
    if param.optional || !param.destructure.is_empty() {
        return fail(
            g,
            param.name.loc,
            "optional or destructured parameters are not supported yet",
        );
    }
    if let Some(ty) = functions::param_type(g, param, b"this function")? {
        return Ok(ty);
    }
    let Some(action) = kind else {
        return fail(
            g,
            param.name.loc,
            format!(
                "parameter `{}` needs a type annotation",
                String::from_utf8_lossy(&param.name.name)
            ),
        );
    };
    Ok(action_type(action))
}

/// Item actions use their own schema; JSX nested in any supported prop is
/// scanned too.
fn scan_property(
    g: &mut Generator<'_>,
    prop: &tags::Prop,
    value: &ast::Expr,
    loc: ast::Loc,
) -> Result<()> {
    if prop.kind == Kind::ShortcutAction {
        // `(shortcut: string) => void`: a text action behind an adapter
        let mut text_prop = prop.clone();
        text_prop.kind = Kind::TextAction;
        return event(g, value, &text_prop, loc);
    }
    if is_action(prop.kind) {
        event(g, value, prop, loc)?;
    }
    if prop.kind != Kind::StructList || !matches!(value.data, ast::ExprData::Array(_)) {
        return scan_expr(g, value);
    }
    let schema = tags::Tag {
        name: "",
        builder: "",
        instance_id: false,
        content: tags::Content::None,
        content_field: "",
        props: prop.item_props.clone(),
    };
    let ast::ExprData::Array(items) = &value.data else {
        return Ok(());
    };
    for (index, item) in items.iter().enumerate() {
        let ast::ExprData::Object(properties) = &item.data else {
            continue;
        };
        for property in properties {
            if property.spread {
                continue;
            }
            if let Some(field) =
                tags::find_prop(&schema, &String::from_utf8_lossy(&property.key.name))
            {
                if let Err(err) = scan_property(g, field, &property.value, property.key.loc) {
                    if err == super::Error::Tsx {
                        let prefix = format!(
                            "in {}[{}].{}",
                            prop.name,
                            index,
                            String::from_utf8_lossy(&property.key.name)
                        );
                        let _ = super::contextualize(g, &prefix);
                    }
                    return Err(err);
                }
            } else {
                scan_expr(g, &property.value)?;
            }
        }
    }
    Ok(())
}

/// Registers an inline arrow as the handler method of `prop`.
fn register_arrow(g: &mut Generator<'_>, prop: &tags::Prop, e: &ast::Expr) -> Result<usize> {
    let ast::ExprData::Arrow(function) = &e.data else {
        return fail(
            g,
            e.loc,
            "event handler must be a component function, plain function, setter, or inline arrow",
        );
    };
    g.counter += 1;
    let base = format!("_{}{}", super::jsx::snake_case(prop.name), g.counter);
    let name = scope::unique_method(g, &base)?;
    let ret = match function.ret.as_ref() {
        Some(reference) => types::type_ref(g, reference, true)?,
        None => Type::Void,
    };
    let capture_name = scope::unique_method(g, &format!("_Capture{}", g.counter))?;
    let method = Method {
        name,
        func: Some(function.clone()),
        action: Some(prop.kind),
        ret,
        capture_name: Some(capture_name),
        ..Method::default()
    };
    signature(g, &method, prop.kind, e.loc, true)?;
    g.methods.push(method);
    Ok(g.methods.len() - 1)
}

/// A component function that reads render locals, as an arrow expression.
fn inline_function(g: &Generator<'_>, name: &[u8]) -> Option<Box<ast::Function>> {
    let index = scope::source_method(g, name)?;
    let method = &g.methods[index];
    if method.inline_only {
        method.func.clone()
    } else {
        None
    }
}

/// `fn` passed as a handler, as the arrow `() => fn()` / `(v) => fn(v)`.
fn function_arrow(e: &ast::Expr, kind: Kind, pass_value: bool) -> ast::Expr {
    let takes_value = kind != Kind::Action;
    let mut args: Vec<Box<ast::Expr>> = Vec::new();
    let mut params: Vec<ast::Param> = Vec::new();
    if takes_value {
        if pass_value {
            args.push(Box::new(ast::Expr {
                loc: e.loc,
                data: ast::ExprData::Ident(b"_value".to_vec()),
            }));
        }
        params.push(ast::Param {
            name: ast::Name {
                name: b"_value".to_vec(),
                loc: e.loc,
            },
            ty: None,
            optional: false,
            destructure: Vec::new(),
            destructure_defaults: Vec::new(),
            rest: false,
            default: None,
        });
    }
    let call = ast::Expr {
        loc: e.loc,
        data: ast::ExprData::Call(ast::Call {
            is_new: false,
            callee: Box::new(e.clone()),
            type_args: Vec::new(),
            args,
            optional: false,
        }),
    };
    let function = ast::Function {
        type_params: Vec::new(),
        loc: e.loc,
        name: Vec::new(),
        params,
        ret: None,
        body: ast::Body::Expr(Box::new(call)),
    };
    ast::Expr {
        loc: e.loc,
        data: ast::ExprData::Arrow(Box::new(function)),
    }
}

/// `fn` passed as a child component's callback prop of `count` parameters:
/// `(_a0, …) => fn(_a0, …)` (the prop gives the parameter types). Used by the
/// component layer.
pub fn forward_arrow(e: &ast::Expr, count: usize) -> ast::Expr {
    let mut args: Vec<Box<ast::Expr>> = Vec::new();
    let mut params: Vec<ast::Param> = Vec::new();
    for index in 0..count {
        let name = format!("_a{index}").into_bytes();
        args.push(Box::new(ast::Expr {
            loc: e.loc,
            data: ast::ExprData::Ident(name.clone()),
        }));
        params.push(ast::Param {
            name: ast::Name { name, loc: e.loc },
            ty: None,
            optional: false,
            destructure: Vec::new(),
            destructure_defaults: Vec::new(),
            rest: false,
            default: None,
        });
    }
    let call = ast::Expr {
        loc: e.loc,
        data: ast::ExprData::Call(ast::Call {
            is_new: false,
            callee: Box::new(e.clone()),
            type_args: Vec::new(),
            args,
            optional: false,
        }),
    };
    let function = ast::Function {
        type_params: Vec::new(),
        loc: e.loc,
        name: Vec::new(),
        params,
        ret: None,
        body: ast::Body::Expr(Box::new(call)),
    };
    ast::Expr {
        loc: e.loc,
        data: ast::ExprData::Arrow(Box::new(function)),
    }
}

fn scan_conditional(g: &mut Generator<'_>, value: &ast::Cond) -> Result<()> {
    scan_expr(g, &value.cond)?;
    scan_expr(g, &value.then)?;
    scan_expr(g, &value.else_)
}

/// Kept so the JSX composer can refer to the same diagnostic wording.
pub fn handler_diagnostic(g: &mut Generator<'_>, loc: ast::Loc) -> Result<()> {
    fail(
        g,
        loc,
        "event handler must be a component function, plain function, setter, or inline arrow",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::Generator;
    use crate::codegen::types::{Type, Value};
    use crate::tags::Kind;

    fn with_generator(f: impl FnOnce(&mut Generator<'_>)) {
        let mut diag = None;
        let mut g = Generator::new(&mut diag);
        f(&mut g);
    }

    #[test]
    fn action_kinds_map_to_rust_builders_and_signatures() {
        assert_eq!("action", action_builder(Kind::Action));
        assert_eq!("text_action", action_builder(Kind::TextAction));
        assert_eq!("bool_action", action_builder(Kind::BoolAction));
        assert_eq!("index_action", action_builder(Kind::IndexAction));
        assert_eq!(Type::Void, action_type(Kind::Action));
        assert_eq!(Type::String, action_type(Kind::TextAction));
        assert_eq!(Type::Boolean, action_type(Kind::BoolAction));
        assert_eq!(Type::Usize, action_type(Kind::IndexAction));
        assert_eq!(
            "(value: string) => void",
            action_signature(Kind::TextAction)
        );
        assert!(is_action(Kind::IndexAction));
        assert!(!is_action(Kind::ShortcutAction));
    }

    #[test]
    fn setters_emit_str_and_bool_adapters() {
        with_generator(|g| {
            g.name = "Counter".to_owned();
            g.states.push(crate::codegen::State {
                name: "name".to_owned(),
                setter: "setName".to_owned(),
                ty: Type::String,
                initial: "String::new()".to_owned(),
                kind: crate::codegen::StateKind::State,
            });
            let method = Method {
                name: "_set_name".to_owned(),
                setter: Some(0),
                action: Some(Kind::TextAction),
                ..Method::default()
            };
            let code = emit_setter(g, &method).expect("setter");
            assert_eq!(
                "fn _set_name(&mut self, value: &str) {\nself.name = zant::Str::copy(value);\n}\n",
                code
            );
        });
    }

    #[test]
    fn the_jsx_hook_contract_is_the_registered_method() {
        with_generator(|g| {
            assert_eq!(0, g.events.len());
            assert!(g.lists.is_empty());
            assert_eq!(0, g.child_sites);
            assert_eq!(0, g.scroll_depth);
            assert_eq!(0, g.loop_depth);
            let _ = Value::new(Type::Node, "ui::Node");
        });
    }
}
