//! Array, tuple and object literals, with the object type inferred from the fields.

use super::*;

/// General arrays become `tsx::List` values.
pub fn array(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    items: &[Box<ast::Expr>],
    expected: Option<Type>,
) -> Result<Value> {
    if let Some(items_ty) = expected.and_then(|ty| model::tuple_items(g, unwrap(g, ty))) {
        return tuple(g, c, loc, items, expected.unwrap(), &items_ty);
    }
    let mut child: Option<Type> = match expected {
        Some(ty) => model::element(g, unwrap(g, ty)),
        None => None,
    };
    let mut values: Vec<Value> = Vec::new();
    for item in items {
        let spread = matches!(item.data, ast::ExprData::Spread(_));
        let context = if expected.is_some() {
            child
        } else {
            child.filter(|ty| {
                !(ty.numeric() || *ty == Type::Null || matches!(item.data, ast::ExprData::NullLit))
            })
        };
        let target: &ast::Expr = match &item.data {
            ast::ExprData::Spread(inner) => inner,
            _ => item,
        };
        // `[...xs]` copies the array: a later change of `xs` is not seen
        // here unless the items themselves are shared objects
        if spread {
            crate::codegen::place::project(c, target);
        }
        let value = expr::emit(g, c, target, if spread { None } else { context }, false)?;
        if spread
            && let ast::ExprData::Ident(name) = &target.data
            && let Some(index) = scope::local_index(c, name)
            && model::element(g, value.ty).is_some_and(|item| model::owned(g, item))
        {
            c.bindings[index].escaped = true;
        }
        if spread && model::optional_child(g, value.ty).is_some() {
            return fail(g, item.loc, "optional array needs ?? or ! before spread");
        }
        let element = if spread {
            match model::element(g, value.ty) {
                Some(element) => element,
                None => return fail(g, item.loc, "spread needs an array"),
            }
        } else {
            value.ty
        };
        child = match child {
            Some(ty) => Some(types::unify(g, ty, element, item.loc)?),
            None => Some(element.concrete()),
        };
        values.push(value);
    }
    let Some(element) = child else {
        return fail(g, loc, "empty array needs a type annotation");
    };
    let element = element.concrete();
    let ty = model::array(g, element)?;
    let element_rust = element.rust(g).to_owned();
    // in source order: `[...xs, x]` appends, `[x, ...xs]` prepends
    let mut parts: Vec<(bool, String)> = Vec::new();
    for (item, raw) in items.iter().zip(values) {
        if matches!(item.data, ast::ExprData::Spread(_)) {
            if model::element(g, raw.ty) != Some(element) {
                return fail(g, item.loc, "spread array element type does not match");
            }
            parts.push((
                true,
                rust_text::operand(&raw, Precedence::Suffix, false).to_string(),
            ));
        } else {
            let value = types::coerce(g, raw, element, item.loc)?;
            parts.push((false, value.code));
        }
    }
    if parts.iter().all(|(spread, _)| !spread) {
        let parts: Vec<String> = parts.into_iter().map(|(_, code)| code).collect();
        let code = if parts.is_empty() {
            // an empty `vec![]` would need the type from context; name it
            format!("tsx::List::from_vec(Vec::<{element_rust}>::new())")
        } else {
            format!("tsx::List::from_vec(vec![{}])", parts.join(", "))
        };
        return Ok(Value {
            ty,
            code,
            ..Value::default()
        });
    }
    let temp = temporary(g, c, b"_items")?;
    let mut block = format!("{{\n    let mut {temp}: Vec<{element_rust}> = Vec::new();\n");
    for (spread, code) in parts {
        if spread {
            block.push_str(&format!("    {temp}.extend({code}.iter().cloned());\n"));
        } else {
            block.push_str(&format!("    {temp}.push({code});\n"));
        }
    }
    block.push_str(&format!("    tsx::List::from_vec({temp})\n}}"));
    Ok(Value {
        ty,
        code: block,
        precedence: Precedence::Conditional,
        ..Value::default()
    })
}

/// `[a, b]` where a tuple `[A, B]` is expected.
pub(super) fn tuple(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    items: &[Box<ast::Expr>],
    expected: Type,
    types_of: &[Type],
) -> Result<Value> {
    if items.len() != types_of.len()
        || items
            .iter()
            .any(|item| matches!(item.data, ast::ExprData::Spread(_)))
    {
        return fail(
            g,
            loc,
            format!("this tuple has {} elements", types_of.len()),
        );
    }
    let mut parts = Vec::new();
    for (item, ty) in items.iter().zip(types_of) {
        let value = expr::emit(g, c, item, Some(*ty), false)?;
        parts.push(types::coerce(g, value, *ty, item.loc)?.code);
    }
    let tuple_ty = unwrap(g, expected);
    let code = if parts.len() == 1 {
        format!("({},)", parts[0])
    } else {
        format!("({})", parts.join(", "))
    };
    Ok(Value::new(tuple_ty, code))
}

/// The type of an object literal without a contextual type, as TypeScript
/// infers it: each property's (widened) value type, in source order. A lone
/// spread base (`{ ...todo, done: true }`) keeps the base's type.
pub(super) fn infer_object(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    properties: &[ast::Property],
) -> Result<Type> {
    let spreads: Vec<&ast::Property> = properties.iter().filter(|p| p.spread).collect();
    if let [base] = spreads.as_slice() {
        let value = expr::emit(g, c, &base.value, None, false)?;
        if model::info(g, value.ty).is_some_and(|info| info.kind == model::InfoKind::Object) {
            return Ok(value.ty);
        }
        return fail(g, base.value.loc, "spread needs an object");
    }
    if !spreads.is_empty() {
        return fail(
            g,
            loc,
            "an object with several spreads needs a type annotation (`const x: T = { … }`)",
        );
    }
    let mut members: Vec<model::Member> = Vec::new();
    for property in properties {
        if matches!(property.value.data, ast::ExprData::NullLit) {
            return fail(
                g,
                property.value.loc,
                format!(
                    "`{}` is null: give the object a type so the field's type is known (`const x: T = {{ … }}`)",
                    String::from_utf8_lossy(&property.key.name)
                ),
            );
        }
        let value = expr::emit(g, c, &property.value, None, false)?;
        if value.ty == Type::Void || value.ty == Type::Node || value.ty == Type::OptionalNode {
            return fail(
                g,
                property.value.loc,
                "this value cannot be an object field",
            );
        }
        let ty = value.ty.concrete();
        // a repeated key: the last one wins, as in JavaScript
        members.retain(|member| member.name != property.key.name);
        members.push(model::Member {
            name: property.key.name.clone(),
            loc: property.key.loc,
            ty,
            optional: false,
        });
    }
    model::literal_object(g, members)
}

/// Copy a spread exactly once, then apply fields in source order (last write
/// wins).
pub fn object(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    properties: &[ast::Property],
    expected: Option<Type>,
) -> Result<Value> {
    let expected = match expected {
        Some(expected) => expected,
        None => infer_object(g, c, loc, properties)?,
    };
    let ty = unwrap(g, expected);
    if crate::codegen::unions::is_union(g, ty) {
        let value = crate::codegen::unions::object_literal(g, c, loc, properties, ty)?;
        return Ok(if expected == ty {
            value
        } else {
            Value {
                ty: expected,
                code: format!("Some({})", value.code),
                ..Value::default()
            }
        });
    }
    if crate::codegen::dicts::kind(g, ty) == Some(model::InfoKind::Record) {
        let value = crate::codegen::dicts::literal(g, c, loc, properties, ty)?;
        return Ok(Value {
            ty: expected,
            code: if expected == ty {
                value.code
            } else {
                format!("Some({})", value.code)
            },
            ..Value::default()
        });
    }
    let Some(entry) = model::info(g, ty) else {
        return fail(g, loc, "object literal needs a struct type");
    };
    if entry.kind != model::InfoKind::Object {
        return fail(g, loc, "object literal needs a struct type");
    }
    let fields = entry.fields;
    let temp = temporary(g, c, b"_object")?;
    let mut values: Vec<Value> = Vec::new();
    let mut writes: Vec<String> = Vec::new();
    let mut spread = false;
    for property in properties {
        let field_ty = if property.spread {
            ty
        } else {
            model::field_type(g, ty, &property.key.name, property.key.loc)?
        };
        let value = expr::emit(g, c, &property.value, Some(field_ty), false)?;
        let value_code = value.code.clone();
        if property.spread {
            spread = true;
            writes.push(format!("{temp} = {value_code};"));
        } else {
            writes.push(format!(
                "{temp}.{} = {value_code};",
                rust_text::id_at(g, &property.key.name, property.key.loc)?
            ));
        }
        values.push(value);
    }
    if spread {
        let ty_rust = ty.rust(g).to_owned();
        let path = rust_text::expression_path(&ty_rust);
        let mut block = format!("{{\n    let mut {temp}: {ty_rust} = {path}::default();\n");
        for write in &writes {
            block.push_str(&format!("    {write}\n"));
        }
        block.push_str(&format!("    {temp}\n}}"));
        return Ok(Value {
            ty,
            code: block,
            precedence: Precedence::Conditional,
            ..Value::default()
        });
    }
    for field in &fields {
        let present = properties
            .iter()
            .any(|property| property.key.name == field.name);
        if !present && !field.optional {
            return fail(
                g,
                loc,
                format!(
                    "{} is missing field `{}`",
                    ty.rust(g),
                    String::from_utf8_lossy(&field.name)
                ),
            );
        }
    }
    // Source order matters when field values call component functions.
    let mut initial: Vec<String> = Vec::new();
    for (index, property) in properties.iter().enumerate() {
        let replaced = properties[index + 1..]
            .iter()
            .any(|later| later.key.name == property.key.name);
        if replaced {
            continue;
        }
        initial.push(format!(
            "    {}: {},",
            rust_text::id_at(g, &property.key.name, property.key.loc)?,
            values[index].code
        ));
    }
    let all_present = fields.iter().all(|field| {
        properties
            .iter()
            .any(|property| property.key.name == field.name)
    });
    if !all_present {
        initial.push("    ..Default::default()".to_owned());
    }
    Ok(Value {
        ty,
        code: format!(
            "{} {{\n{}\n}}",
            rust_text::expression_path(ty.rust(g)),
            initial.join("\n")
        ),
        ..Value::default()
    })
}
