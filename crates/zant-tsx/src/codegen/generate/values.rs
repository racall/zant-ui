//! Event values: actions and callbacks passed to JSX props.

use super::*;

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
        crate::codegen::captures::prepare(g, c, index)?;
    }
    let method = g.methods[index].clone();
    // handlers that call a callback prop take the runtime context
    let needs_cx = method
        .func
        .as_deref()
        .is_some_and(|function| crate::codegen::action_cx::needs_cx(g, function));
    let capture = if method.captures.is_empty() {
        None
    } else {
        Some(crate::codegen::captures::value(g, &method)?)
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
        arguments.push(crate::codegen::action_cx::CX.to_owned());
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
    // an effect cleanup also runs after its component unmounted (React):
    // it only needs what it captured, so a default instance stands in (and
    // its state updates go nowhere, like React's on an unmounted component)
    let unmounted = match g.current_component {
        Some(component) if method.cleanup => {
            let component =
                rust_text::id_at(g, g.components[component].name.clone().as_bytes(), 0)?;
            format!(
                " else {{\nlet mut _unmounted = {component}::default();\n_unmounted.{name}({child_arguments});\n}}"
            )
        }
        _ => String::new(),
    };
    Ok(format!(
        "{constructor}::new({{\n{prefix}let _state = {state};\n\
         move |_cx{closure_params}| {{\n\
         if let Some(state) = _state.upgrade() {{\n\
         state.borrow_mut().{name}({child_arguments});\n\
         }}{unmounted}\n}}\n}})"
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
        crate::codegen::captures::prepare(g, c, index)?;
    }
    let method = g.methods[index].clone();
    // handlers that call a callback prop take the runtime context
    let needs_cx = method
        .func
        .as_deref()
        .is_some_and(|function| crate::codegen::action_cx::needs_cx(g, function));
    let capture = if method.captures.is_empty() {
        None
    } else {
        Some(crate::codegen::captures::value(g, &method)?)
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
        call_args.push(crate::codegen::action_cx::CX.to_owned());
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
