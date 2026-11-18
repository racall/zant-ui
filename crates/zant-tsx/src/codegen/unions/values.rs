//! Union values: wrapping, object literals, `typeof`, text, member reads, narrowing facts and `switch` coverage.

use super::*;

/// A value of one member converted to union `t` (`U::Num(v)`); `None` when
/// `t` is not a union or `v` is not one of its members.
pub fn wrap(g: &mut Generator<'_>, v: &Value, t: Type, loc: ast::Loc) -> Result<Option<Value>> {
    let Some(variants) = variants(g, t) else {
        return Ok(None);
    };
    let ty = v.ty;
    let found = variants
        .iter()
        .find(|variant| variant.ty == ty)
        .or_else(|| variants.iter().find(|variant| variant.ty == ty.concrete()))
        .or_else(|| {
            // a string literal for a member string-literal enum
            (ty == Type::String && v.text.is_some())
                .then(|| {
                    variants
                        .iter()
                        .find(|variant| model::enum_type(g, variant.ty))
                })
                .flatten()
        })
        .cloned();
    let Some(variant) = found else {
        return Ok(None);
    };
    let inner = types::coerce(g, v.clone(), variant.ty, loc)?;
    Ok(Some(Value::new(
        t,
        format!("{}::{}({})", t.rust(g), variant.rust, inner.code),
    )))
}

/// `{ kind: "circle", r: 1 }` where a union is expected: the member its
/// discriminant (or its fields) selects.
pub fn object_literal(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    properties: &[ast::Property],
    ty: Type,
) -> Result<Value> {
    let variants = variants(g, ty).unwrap_or_default();
    let field = discriminant_field(g, ty);
    let chosen = match &field {
        Some(field) => properties
            .iter()
            .find(|property| !property.spread && property.key.name == *field)
            .and_then(|property| match &property.value.data {
                ast::ExprData::String(text) => variants
                    .iter()
                    .find(|variant| variant.tag.as_deref() == Some(text)),
                _ => None,
            }),
        None => {
            // the one object member with every given field
            let fits: Vec<&Variant> = variants
                .iter()
                .filter(|variant| {
                    model::info(g, variant.ty).is_some_and(|info| {
                        info.kind == InfoKind::Object
                            && properties.iter().all(|property| {
                                property.spread
                                    || info.fields.iter().any(|f| f.name == property.key.name)
                            })
                    })
                })
                .collect();
            (fits.len() == 1).then(|| fits[0])
        }
    };
    let Some(variant) = chosen.cloned() else {
        let hint = match field {
            Some(field) => format!(
                "give the object its `{}` (one of the union's members)",
                String::from_utf8_lossy(&field)
            ),
            None => "annotate the object with the member type it is".to_owned(),
        };
        return fail(
            g,
            loc,
            format!("cannot tell which union member this object is; {hint}"),
        );
    };
    let value = crate::codegen::aggregate::object(g, c, loc, properties, Some(variant.ty))?;
    Ok(Value::new(
        ty,
        format!("{}::{}({})", ty.rust(g), variant.rust, value.code),
    ))
}

/// `match` over a union value, one arm per variant.
pub(super) fn arms(g: &Generator<'_>, ty: Type, arm: impl Fn(&Variant) -> String) -> String {
    let rust = ty.rust(g).to_owned();
    variants(g, ty)
        .unwrap_or_default()
        .iter()
        .map(|variant| format!("{rust}::{}(_member) => {},\n", variant.rust, arm(variant)))
        .collect()
}

/// `typeof x` (any value; a union decides at run time).
pub fn type_of(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    operand: &ast::Expr,
) -> Result<Value> {
    let value = expr::emit(g, c, operand, None, false)?;
    if let Some(text) = crate::codegen::exceptions::type_of(g, &value) {
        return Ok(text);
    }
    if is_union(g, value.ty) {
        let code = format!(
            "match &{} {{\n{}}}",
            rust_text::operand(&value, rust_text::Precedence::Suffix, false),
            arms(g, value.ty, |variant| format!(
                "zant::Str::Static({:?})",
                variant.type_of
            ))
        );
        return Ok(Value::new(Type::String, code));
    }
    if let Some(child) = model::optional_child(g, value.ty) {
        let name = type_of_name(g, child);
        return Ok(Value::new(
            Type::String,
            format!(
                "zant::Str::Static(if {}.is_some() {{ {name:?} }} else {{ \"undefined\" }})",
                rust_text::operand(&value, rust_text::Precedence::Suffix, false)
            ),
        ));
    }
    Ok(Value::new(
        Type::String,
        format!("zant::Str::Static({:?})", type_of_name(g, value.ty)),
    ))
}

/// A union shown as text (`String(x)`, `` `${x}` ``); also a caught value or
/// an `Error` (`exceptions`).
pub fn text(g: &mut Generator<'_>, value: &Value, loc: ast::Loc) -> Result<Option<Value>> {
    if let Some(text) = crate::codegen::exceptions::text(g, value) {
        return Ok(Some(text));
    }
    if crate::codegen::regexps::is_regexp(g, value.ty) {
        return Ok(Some(Value::new(
            Type::String,
            format!(
                "{}.to_str()",
                rust_text::operand(value, rust_text::Precedence::Suffix, false)
            ),
        )));
    }
    let Some(variants) = variants(g, value.ty) else {
        return Ok(None);
    };
    let rust = value.ty.rust(g).to_owned();
    let mut code = format!(
        "match &{} {{\n",
        rust_text::operand(value, rust_text::Precedence::Suffix, false)
    );
    for variant in &variants {
        let read = if variant.ty == Type::String {
            "_member.clone()".to_owned()
        } else if variant.ty.numeric() || variant.ty == Type::Boolean {
            "tsx::builtins::string(*_member)".to_owned()
        } else if model::enum_type(g, variant.ty) {
            "zant::Str::Static(_member.as_str())".to_owned()
        } else {
            return fail(
                g,
                loc,
                "this union has object members; narrow it before showing it as text",
            );
        };
        code.push_str(&format!("{rust}::{}(_member) => {read},\n", variant.rust));
    }
    code.push('}');
    Ok(Some(Value::new(Type::String, code)))
}

/// `s.field` on an un-narrowed union: a field every object member has with
/// one type, or the discriminant (as a string).
pub fn member(g: &mut Generator<'_>, value: &Value, name: &ast::Name) -> Result<Option<Value>> {
    let Some(variants) = variants(g, value.ty) else {
        return Ok(None);
    };
    let operand = rust_text::operand(value, rust_text::Precedence::Suffix, false);
    if discriminant_field(g, value.ty).as_deref() == Some(name.name.as_slice()) {
        let code = format!(
            "match &{operand} {{\n{}}}",
            arms(g, value.ty, |variant| format!(
                "zant::Str::Static({:?})",
                String::from_utf8_lossy(variant.tag.as_deref().unwrap_or_default())
            ))
        );
        return Ok(Some(Value::new(Type::String, code)));
    }
    let mut shared: Option<Type> = None;
    for variant in &variants {
        let field = model::info(g, variant.ty)
            .filter(|info| info.kind == InfoKind::Object)
            .and_then(|info| {
                info.fields
                    .into_iter()
                    .find(|field| field.name == name.name)
            });
        match (field, shared) {
            (Some(field), None) => shared = Some(field.ty),
            (Some(field), Some(ty)) if field.ty == ty => {}
            _ => {
                return fail(
                    g,
                    name.loc,
                    format!(
                        "not every member of this union has `{}`; narrow it first (`if (x.kind === \"…\")` / `typeof x === \"…\"`)",
                        String::from_utf8_lossy(&name.name)
                    ),
                );
            }
        }
    }
    let field = rust_text::id_at(g, &name.name, name.loc)?;
    let code = format!(
        "match &{operand} {{\n{}}}",
        arms(g, value.ty, |_| format!("_member.{field}.clone()"))
    );
    Ok(Some(Value::new(shared.unwrap_or(Type::Void), code)))
}

/// What `typeof x === "…"` / `x.kind === "…"` proves about `x` (pushed onto
/// `c.narrowed`); false when `b` is no such test.
pub fn fact(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    b: &ast::Binary,
    positive: bool,
) -> Result<bool> {
    let equal = b.op == b"===" || b.op == b"==";
    let not_equal = b.op == b"!==" || b.op == b"!=";
    if !equal && !not_equal {
        return Ok(false);
    }
    let (test, literal) = match (&b.lhs.data, &b.rhs.data) {
        (_, ast::ExprData::String(text)) => (&*b.lhs, text.clone()),
        (ast::ExprData::String(text), _) => (&*b.rhs, text.clone()),
        _ => return Ok(false),
    };
    let (subject, by_typeof) = match &test.data {
        ast::ExprData::Unary(unary) if unary.op == b"typeof" => (&*unary.operand, true),
        ast::ExprData::Member(member) => (&*member.object, false),
        _ => return Ok(false),
    };
    let Some(path) = narrowing::path(subject) else {
        return Ok(false);
    };
    let binding = scope::local_index(c, &narrowing::base(subject));
    let value = expr::emit(g, c, subject, None, false)?;
    let Some(variants) = variants(g, value.ty) else {
        return Ok(false);
    };
    if let ast::ExprData::Member(member) = &test.data
        && discriminant_field(g, value.ty).as_deref() != Some(member.name.name.as_slice())
    {
        return Ok(false);
    }
    let matches = |variant: &Variant| {
        if by_typeof {
            variant.type_of.as_bytes() == literal.as_slice()
        } else {
            variant.tag.as_deref() == Some(literal.as_slice())
        }
    };
    let proves_match = equal == positive;
    let remaining: Vec<&Variant> = variants
        .iter()
        .filter(|variant| matches(variant) == proves_match)
        .collect();
    let [variant] = remaining.as_slice() else {
        return Ok(true);
    };
    let rust = value.ty.rust(g).to_owned();
    c.narrowed.push(Narrow {
        name: String::from_utf8_lossy(&path).into_owned(),
        binding,
        generation: narrowing::generation(c, binding),
        ty: variant.ty,
        code: format!(
            "(match {} {{ {rust}::{}(_member) => _member, _ => unreachable!(\"narrowed by the condition\") }})",
            rust_text::operand(&value, rust_text::Precedence::Suffix, false),
            variant.rust
        ),
    });
    Ok(true)
}

/// For `switch (x.kind)` / `switch (typeof x)`: the subject's union and
/// whether the switch handles every member (by the cases' literals).
pub fn switch_covers(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    value: &ast::Expr,
    cases: &[Vec<u8>],
) -> Result<bool> {
    let (subject, by_typeof) = match &value.data {
        ast::ExprData::Unary(unary) if unary.op == b"typeof" => (&*unary.operand, true),
        ast::ExprData::Member(member) => (&*member.object, false),
        _ => return Ok(false),
    };
    let ty = match expr::emit(g, c, subject, None, false) {
        Ok(value) => value.ty,
        Err(_) => {
            *g.diag = None;
            return Ok(false);
        }
    };
    let Some(variants) = variants(g, ty) else {
        return Ok(false);
    };
    Ok(variants.iter().all(|variant| {
        let label: &[u8] = if by_typeof {
            variant.type_of.as_bytes()
        } else {
            variant.tag.as_deref().unwrap_or_default()
        };
        cases.iter().any(|case| case.as_slice() == label)
    }))
}
