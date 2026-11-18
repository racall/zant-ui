//! The row closures of a `<List>`: binding the item, the `renderItem` closure and the item id closure.

use super::*;

/// Bind `renderItem`'s / `keyExtractor`'s params to `rows[index]` and the
/// index, and return the declarations that must open the closure body. The
/// index parameter is allocated first, so a user param called `index` gets
/// renamed instead of shadowing it.
pub(super) fn bind_item(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    function: &ast::Function,
    element: Type,
    rows: &str,
) -> Result<String> {
    let index_param = scope::bind(g, c, b"_index", Type::Usize, false)?;
    let index_name = c.bindings[index_param].rust.clone();
    let item = &function.params[0];
    if let Some(reference) = item.ty.as_ref()
        && types::type_ref(g, reference, false)? != element
    {
        return fail(g, item.name.loc, "the item type does not match `data`");
    }
    let binding = scope::bind(g, c, &item.name.name, element, false)?;
    let item_name = c.bindings[binding].rust.clone();
    let item_code = if element.copy(g) {
        format!("{rows}[{index_name}]")
    } else {
        format!("{rows}[{index_name}].clone()")
    };
    let mut prologue = format!("let {item_name}: {} = {item_code};\n", element.rust(g));
    if function.params.len() == 2 {
        let index = &function.params[1];
        if let Some(reference) = index.ty.as_ref()
            && types::type_ref(g, reference, false)? != Type::Usize
        {
            return fail(g, index.name.loc, "the index is a usize");
        }
        let binding = scope::bind(g, c, &index.name.name, Type::Usize, false)?;
        let name = c.bindings[binding].rust.clone();
        prologue.push_str(&format!("let {name}: usize = {index_name};\n"));
    }
    Ok(prologue)
}

/// `move |_index: usize| -> ui::Node`: renderItem in render context.
pub(super) fn item_closure(
    g: &mut Generator<'_>,
    source: &mut scope::Context,
    function: &ast::Function,
    element: Type,
    captures: &[Capture],
    rows: &str,
) -> Result<String> {
    let mut c = scope::Context {
        render: false,
        ret: Type::Node,
        capture_source: Some(source.capture_source()),
        ..scope::Context::default()
    };
    // captured values are declared before the closure; the body reads them by
    // name (the binding's Rust identifier is the `_cap_*` local)
    for capture in captures {
        let binding = scope::bind(g, &mut c, &capture.name, capture.ty, false)?;
        c.bindings[binding].rust = capture.local.clone();
    }
    // the closure is built while rendering the component: it must not borrow
    // `self` (it outlives the `&mut self` build), so prop reads go through the
    // `_cap_props` local and actions upgrade the hoisted `_state` local
    let previous = if g.current_component.is_some() {
        g.handler_state.replace("_state".to_owned())
    } else {
        None
    };
    let prologue = bind_item(g, &mut c, function, element, rows)?;
    let mut body = prologue;
    match &function.body {
        ast::Body::Expr(e) => {
            let value = expr::emit(g, &mut c, e, Some(Type::Node), false)?;
            body.push_str(&format!("return {};\n", value.code));
        }
        ast::Body::Block(statements) => {
            body.push_str(&stmt::statements(g, &mut c, statements, true)?);
            // checked after generating: an exhaustive switch is known then
            if !stmt::returns_all(statements) {
                return fail(
                    g,
                    function.loc,
                    "renderItem must return an element on every path",
                );
            }
        }
    }
    let finished = scope::finish(g, &c, body)?;
    g.handler_state = previous;
    Ok(format!(
        "std::rc::Rc::new(move |_index: usize| -> ui::Node {{\n{finished}}})"
    ))
}

/// `move |_index: usize| -> u64` from keyExtractor: integer keys as they are,
/// floats truncated, strings hashed.
pub(super) fn id_closure(
    g: &mut Generator<'_>,
    function: &ast::Function,
    element: Type,
    rows: &str,
    captures: &[Capture],
) -> Result<String> {
    let mut c = scope::Context {
        render: false,
        ret: Type::U64,
        ..scope::Context::default()
    };
    for capture in captures {
        let binding = scope::bind(g, &mut c, &capture.name, capture.ty, false)?;
        c.bindings[binding].rust = capture.local.clone();
    }
    let component = std::mem::take(&mut g.current_component);
    let prologue = bind_item(g, &mut c, function, element, rows)?;
    let e = match &function.body {
        ast::Body::Expr(e) => e,
        ast::Body::Block(_) => {
            return fail(
                g,
                function.loc,
                "keyExtractor must be a single expression: (item) => item.id",
            );
        }
    };
    let emitted = expr::emit(g, &mut c, e, None, false)?;
    let value = types::concrete(g, emitted)?;
    let id = if value.ty == Type::String {
        format!("zant::core::hash::hash(0, ({}).as_str())", value.code)
    } else if value.ty.integer() || value.ty.float() {
        format!("({}) as u64", value.code)
    } else {
        return fail(g, e.loc, "keyExtractor must return a string or a number");
    };
    let finished = scope::finish(g, &c, format!("{prologue}return {id};\n"))?;
    g.current_component = component;
    Ok(format!(
        "std::rc::Rc::new(move |_index: usize| -> u64 {{\n{finished}}})"
    ))
}
