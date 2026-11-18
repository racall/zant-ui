//! Emitting a call of a child component: instance id, props struct, children and prop values.

use super::*;

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
pub(super) fn instance_id(
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

pub(super) fn field_list(g: &Generator<'_>, component: &Component) -> String {
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
pub(super) fn children_value(
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
    // `children?: …` is an `Option<…>` field: the given children are `Some`
    let optional = model::optional_child(g, ty);
    let base = optional.unwrap_or(ty);
    let wrap = |code: String| {
        if optional.is_some() {
            format!("Some({code})")
        } else {
            code
        }
    };
    if base == Type::Node {
        if parts.len() != 1 || parts[0].ty != Type::Node {
            return fail(
                g,
                j.loc,
                format!("<{}> takes one child element", component.name),
            );
        }
        return Ok(Some(wrap(parts[0].code.clone())));
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
            if part.ty == base {
                // a `children: ReactNode[]` value is a `tsx::List<Node>`
                format!("({}).as_slice().to_vec()", part.code)
            } else {
                part.code
            }
        })
        .collect();
    Ok(Some(wrap(format!(
        "tsx::List::from_vec(nodes![{}])",
        codes.join(", ")
    ))))
}

/// `children: string`: `<Badge>new {count}</Badge>` passes its text.
pub(super) fn text_children(
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

pub(super) fn prop_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    ty: Type,
    value: &ast::Expr,
) -> Result<String> {
    if let Some(entry) = callback_info(g, ty) {
        let optional = model::optional_child(g, ty).is_some();
        return callback_choice(g, c, &entry, optional, value);
    }
    Ok(expr::emit(g, c, value, Some(ty), false)?.code)
}
