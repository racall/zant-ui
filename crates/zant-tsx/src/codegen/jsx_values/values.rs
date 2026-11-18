//! Prop values: nodes, children, string lists and struct lists.

use super::*;

/// The address of an AST expression, compared when deciding whether all
/// edges share one source value.
pub fn expression_address(e: &ast::Expr) -> usize {
    std::ptr::from_ref(e) as usize
}

/// Optional nodes accept null; required nodes always retain a concrete ui.Node.
pub fn node(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    prop: &Prop,
    e: &ast::Expr,
) -> Result<String> {
    let value = expr::emit(g, c, e, None, false)?;
    let optional = prop.kind == Kind::OptionalNode;
    if value.ty == Type::Node {
        // the option field is `Option<Node>`: wrap the element
        return Ok(if optional {
            format!("Some({})", value.code)
        } else {
            value.code
        });
    }
    if optional && (value.ty == Type::OptionalNode || value.ty == Type::Null) {
        return Ok(types::coerce(g, value, Type::OptionalNode, e.loc)?.code);
    }
    if model::optional_child(g, value.ty).is_some() {
        return fail(
            g,
            e.loc,
            format!(
                "prop `{}` needs a node; unwrap the optional with ?? or !",
                prop.name
            ),
        );
    }
    fail(
        g,
        e.loc,
        format!(
            "prop `{}` requires {}; wrap text in <Text>",
            prop.name,
            if optional {
                "an element or null"
            } else {
                "an element"
            }
        ),
    )
}

/// Arrays remain restricted to prop positions until general arrays are implemented.
/// (The AST stores array items boxed; cloning them keeps handler identities.)
#[allow(clippy::vec_box)]
pub(super) fn array(
    g: &mut Generator<'_>,
    prop: &Prop,
    e: &ast::Expr,
) -> Result<Vec<Box<ast::Expr>>> {
    if let ast::ExprData::Array(items) = &e.data {
        return Ok(items.clone());
    }
    fail(
        g,
        e.loc,
        format!("prop `{}` requires an array literal", prop.name),
    )
}

/// Reuse child flattening and optional filtering for an explicit children array.
pub fn children(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    prop: &Prop,
    e: &ast::Expr,
) -> Result<String> {
    let mut list = ChildList::default();
    if let ast::ExprData::Array(items) = &e.data {
        for item in items {
            if let ast::ExprData::Spread(inner) = &item.data {
                let value = expr::emit(g, c, inner, None, false)?;
                if model::element(g, value.ty).is_none() {
                    return fail(g, item.loc, "spread needs an array");
                }
                list.node(g, c, value, None, item.loc)?;
            } else {
                list.expression(g, c, item, None)?;
            }
        }
    } else {
        let value = expr::emit(g, c, e, None, true)?;
        if model::element(g, value.ty).is_none() {
            return fail(
                g,
                e.loc,
                format!("prop `{}` requires an array of nodes", prop.name),
            );
        }
        list.node(g, c, value, None, e.loc)?;
    }
    let code = children_code(g, &list);
    list.wrap(g, c, &code)
}

/// `nodes![a, b]`: the macro flattens nodes, optional nodes and node lists.
pub fn children_code(_g: &Generator<'_>, list: &ChildList) -> String {
    if list.values.is_empty() {
        return "nodes![]".to_owned();
    }
    let codes: Vec<String> = list.values.iter().map(|value| value.code.clone()).collect();
    format!("nodes![{}]", codes.join(", "))
}

/// A string-list field gives each expression its `zant::Str` result target.
pub fn strings(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    prop: &Prop,
    e: &ast::Expr,
) -> Result<String> {
    let mut spread = false;
    if let ast::ExprData::Array(items) = &e.data {
        spread = items
            .iter()
            .any(|item| matches!(item.data, ast::ExprData::Spread(_)));
    }
    if !matches!(e.data, ast::ExprData::Array(_)) || spread {
        let expected = model::array(g, Type::String)?;
        let code = expr::emit(g, c, e, Some(expected), false)?.code;
        // `SelectOptions.options` is a `Vec<Str>` (List<Str> → Vec<Str>)
        return Ok(format!("({code}).as_slice().to_vec()"));
    }
    let mut codes: Vec<String> = Vec::new();
    for item in array(g, prop, e)? {
        codes.push(expr::emit(g, c, &item, Some(Type::String), false)?.code);
    }
    if codes.is_empty() {
        return Ok("Vec::new()".to_owned());
    }
    Ok(format!("vec![{}]", codes.join(", ")))
}

/// Typed item arrays also work when empty, without depending on peer typing.
pub fn structs(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    prop: &Prop,
    e: &ast::Expr,
) -> Result<String> {
    let items = array(g, prop, e)?;
    let mut codes: Vec<String> = Vec::new();
    let item_path = type_path(prop.item_type);
    for (index, item) in items.iter().enumerate() {
        let owner = format!("{}[{}]", prop.name, index);
        codes.push(struct_item(g, c, prop, &item_path, item, &owner)?);
    }
    if codes.is_empty() {
        return Ok(format!("Vec::<{item_path}>::new()"));
    }
    Ok(format!("vec![{}]", codes.join(", ")))
}

/// One object literal for a struct prop (`labels={{ copy: "…" }}`); omitted
/// keys keep the struct's defaults.
pub fn object(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    prop: &Prop,
    e: &ast::Expr,
) -> Result<String> {
    let item_path = type_path(prop.item_type);
    struct_item(g, c, prop, &item_path, e, prop.name)
}

/// Item keys use the same prop conversions recursively, with index/key diagnostics.
pub(super) fn struct_item(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    prop: &Prop,
    item_path: &str,
    item: &ast::Expr,
    owner: &str,
) -> Result<String> {
    if matches!(item.data, ast::ExprData::Spread(_)) {
        return fail(
            g,
            item.loc,
            format!("{owner}: array spread is not supported yet"),
        );
    }
    let ast::ExprData::Object(properties) = &item.data else {
        return fail(g, item.loc, format!("{owner} must be an object literal"));
    };
    let schema = Tag {
        name: "",
        builder: "",
        instance_id: false,
        content: tags::Content::None,
        content_field: "",
        props: prop.item_props.clone(),
    };
    let mut fields = Fields::default();
    for property in properties {
        if property.spread {
            return fail(
                g,
                property.value.loc,
                format!("{owner}: object spread is not supported yet"),
            );
        }
        let Some(field) = tags::find_prop(&schema, &String::from_utf8_lossy(&property.key.name))
        else {
            return fail(
                g,
                property.key.loc,
                format!(
                    "{owner}: unknown key `{}`; valid keys: {}",
                    String::from_utf8_lossy(&property.key.name),
                    jsx::valid_props(&schema)
                ),
            );
        };
        let result = jsx::property(
            g,
            c,
            prop.item_type,
            field,
            &mut fields,
            Some(property.value.as_ref()),
            property.key.loc,
        );
        if let Err(err) = result {
            if err == crate::codegen::Error::Tsx {
                let prefix = format!("in {owner}.{}", String::from_utf8_lossy(&property.key.name));
                let _ = crate::codegen::contextualize(g, &prefix);
            }
            return Err(err);
        }
    }
    required(g, &prop.item_props, &fields, item.loc, owner, "key")?;
    // item structs with only required fields (e.g. `ui::TabItem`) do not
    // implement `Default`
    let defaulted = prop
        .item_props
        .iter()
        .any(|field| !field.required && !fields.has_prop(field));
    Ok(fields.emit_with_default(item_path, defaulted))
}

/// `ui.MenuItem` → `ui::MenuItem` (Rust paths use `::`).
pub fn type_path(name: &str) -> String {
    name.replace('.', "::")
}

/// Check emitted field paths so aliases and JSX content satisfy required props too.
pub fn required(
    g: &mut Generator<'_>,
    props: &[Prop],
    fields: &Fields,
    loc: ast::Loc,
    owner: &str,
    noun: &str,
) -> Result<()> {
    for prop in props {
        if prop.required && !fields.has_prop(prop) {
            return fail(
                g,
                loc,
                format!("{owner} is missing required {noun} `{}`", prop.name),
            );
        }
    }
    Ok(())
}
