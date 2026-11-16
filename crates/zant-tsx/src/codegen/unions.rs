//! Unions other than string-literal unions and `T | null`:
//! `string | number`, and discriminated object unions
//! (`{ kind: "circle"; r: number } | { kind: "square"; w: number }`), as
//! Rust payload enums — the sum types of Rust / Swift / Kotlin — narrowed
//! by TypeScript's control-flow checks:
//!
//! - `typeof x === "string"` / `"number"` / `"boolean"` / `"object"`;
//! - `s.kind === "circle"` on the discriminant field (also `switch (s.kind)`
//!   and `switch (typeof x)`, exhaustive when every member is handled).
//!
//! Inside the narrowed branch the name reads as that member. Without
//! narrowing, a union can be shown as text, compared with `typeof`, and read
//! through a field every object member has (the discriminant reads as a
//! string).
//!
//! `Info` of a union: `params` are the member types, `members` the Rust
//! variant names, and `fields` holds one entry naming the discriminant
//! field (absent for unions without one).

use crate::ast;

use super::expr;
use super::model::{self, Field, Info, InfoKind};
use super::narrowing::{self, Narrow};
use super::rust_text;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

/// One member of a union.
#[derive(Clone, Debug)]
pub struct Variant {
    pub rust: String,
    pub ty: Type,
    /// The discriminant's literal (`"circle"`) for object members.
    pub tag: Option<Vec<u8>>,
    /// What `typeof` says for it.
    pub type_of: &'static str,
}

pub fn is_union(g: &Generator<'_>, ty: Type) -> bool {
    model::info(g, ty).is_some_and(|info| info.kind == InfoKind::Union)
}

fn type_of_name(g: &Generator<'_>, ty: Type) -> &'static str {
    if ty == Type::String {
        "string"
    } else if ty.numeric() {
        "number"
    } else if ty == Type::Boolean {
        "boolean"
    } else if model::enum_type(g, ty) {
        "string"
    } else {
        "object"
    }
}

/// The single literal of a one-member string enum (`kind: "circle"`).
fn literal(g: &Generator<'_>, ty: Type) -> Option<Vec<u8>> {
    let info = model::info(g, ty)?;
    (info.kind == InfoKind::Enumeration && info.members.len() == 1).then(|| info.members[0].clone())
}

/// A discriminant field and each member's tag (`None` for non-objects).
type Discriminant = (Vec<u8>, Vec<Option<Vec<u8>>>);

/// The discriminant field and each object member's tag.
fn discriminant(g: &Generator<'_>, types: &[Type]) -> Option<Discriminant> {
    let objects: Vec<Info> = types
        .iter()
        .filter_map(|ty| model::info(g, *ty).filter(|info| info.kind == InfoKind::Object))
        .collect();
    if objects.len() < 2 {
        return None;
    }
    for field in &objects[0].fields {
        let mut tags: Vec<Vec<u8>> = Vec::new();
        let all = objects.iter().all(|object| {
            object
                .fields
                .iter()
                .find(|candidate| candidate.name == field.name && !candidate.optional)
                .and_then(|candidate| literal(g, candidate.ty))
                .is_some_and(|tag| {
                    let fresh = !tags.contains(&tag);
                    tags.push(tag);
                    fresh
                })
        });
        if all {
            let per_type = types
                .iter()
                .map(|ty| {
                    model::info(g, *ty)
                        .filter(|info| info.kind == InfoKind::Object)
                        .and_then(|info| {
                            info.fields
                                .iter()
                                .find(|f| f.name == field.name)
                                .map(|f| f.ty)
                        })
                        .and_then(|ty| literal(g, ty))
                })
                .collect();
            return Some((field.name.clone(), per_type));
        }
    }
    None
}

/// The members of union `ty`.
pub fn variants(g: &Generator<'_>, ty: Type) -> Option<Vec<Variant>> {
    let info = model::info(g, ty).filter(|info| info.kind == InfoKind::Union)?;
    let tags = discriminant(g, &info.params).map(|(_, tags)| tags);
    Some(
        info.params
            .iter()
            .zip(&info.members)
            .enumerate()
            .map(|(index, (ty, name))| Variant {
                rust: String::from_utf8_lossy(name).into_owned(),
                ty: *ty,
                tag: tags.as_ref().and_then(|tags| tags[index].clone()),
                type_of: type_of_name(g, *ty),
            })
            .collect(),
    )
}

/// The discriminant field's name of union `ty`, if it has one.
pub fn discriminant_field(g: &Generator<'_>, ty: Type) -> Option<Vec<u8>> {
    model::info(g, ty)
        .filter(|info| info.kind == InfoKind::Union)
        .and_then(|info| info.fields.first().map(|field| field.name.clone()))
}

fn pascal(text: &[u8]) -> String {
    let mut out = String::new();
    let mut upper = true;
    for c in String::from_utf8_lossy(text).chars() {
        if c.is_ascii_alphanumeric() {
            if upper {
                out.extend(c.to_uppercase());
            } else {
                out.push(c);
            }
            upper = false;
        } else {
            upper = true;
        }
    }
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, 'V');
    }
    out
}

/// `A | B | …` (not all string literals; `null` already removed).
pub fn declare(
    g: &mut Generator<'_>,
    loc: ast::Loc,
    members: &[ast::TypeRef],
    name: Option<&[u8]>,
) -> Result<Type> {
    let mut types_list: Vec<Type> = Vec::new();
    for member in members {
        let ty = if matches!(
            member.detail.as_deref(),
            Some(ast::TypeDetail::StringLit(_))
        ) {
            // a string literal next to other types widens to `string`
            Type::String
        } else if matches!(
            member.detail.as_deref(),
            Some(ast::TypeDetail::NumberLit(_))
        ) {
            Type::F64
        } else {
            types::type_ref(g, member, false)?.concrete()
        };
        if model::optional_child(g, ty).is_some() || ty == Type::Node || ty == Type::Void {
            return fail(g, member.loc, "this type cannot be a union member");
        }
        if !types_list.contains(&ty) {
            types_list.push(ty);
        }
    }
    if types_list.len() == 1 {
        return Ok(types_list[0]);
    }
    if name.is_none()
        && let Some(index) = g.compounds.iter().position(|entry| {
            entry.kind == InfoKind::Union
                && entry.params == types_list
                && entry.rust.starts_with("_Union")
        })
    {
        return Ok(Type::Compound(index as u32));
    }
    let discriminant = discriminant(g, &types_list);
    let mut variants: Vec<String> = Vec::new();
    for (index, ty) in types_list.iter().enumerate() {
        let tag = discriminant
            .as_ref()
            .and_then(|(_, tags)| tags[index].clone());
        let base = match (tag, model::info(g, *ty)) {
            (Some(tag), _) => pascal(&tag),
            (None, _) if *ty == Type::String => "Str".to_owned(),
            (None, _) if ty.numeric() => "Num".to_owned(),
            (None, _) if *ty == Type::Boolean => "Bool".to_owned(),
            (None, Some(info)) if info.kind == InfoKind::Array => "List".to_owned(),
            (None, Some(info)) if info.kind == InfoKind::Object && !info.rust.starts_with('_') => {
                pascal(info.rust.rsplit("::").next().unwrap_or("V").as_bytes())
            }
            _ => format!("V{index}"),
        };
        let mut variant = base.clone();
        let mut suffix = 2;
        while variants.contains(&variant) {
            variant = format!("{base}{suffix}");
            suffix += 1;
        }
        variants.push(variant);
    }
    let rust = match name {
        Some(name) => rust_text::id_at(g, name, loc)?,
        None => format!("_Union{}", g.compounds.len()),
    };
    let mut body = String::new();
    let mut save = String::new();
    let mut load = String::new();
    for (index, (variant, ty)) in variants.iter().zip(&types_list).enumerate() {
        let spelled = ty.rust(g).to_owned();
        body.push_str(&format!("    {variant}({spelled}),\n"));
        save.push_str(&format!(
            "            {rust}::{variant}(value) => tsx::hot::Value::Array(vec![tsx::hot::Value::from({index}u64), tsx::hot::Hot::save(value)]),\n"
        ));
        load.push_str(&format!(
            "            Some({index}) => Some({rust}::{variant}(<{spelled} as tsx::hot::Hot>::load(items.get(1)?)?)),\n"
        ));
    }
    let json_variants: Vec<(String, String)> = variants
        .iter()
        .zip(&types_list)
        .map(|(variant, ty)| (variant.clone(), ty.rust(g).to_owned()))
        .collect();
    let first = &variants[0];
    let mut declaration = format!(
        "#[derive(Clone, PartialEq)]\npub enum {rust} {{\n{body}}}\n\nimpl Default for {rust} {{\n    fn default() -> Self {{\n        {rust}::{first}(Default::default())\n    }}\n}}\n\nimpl tsx::hot::Hot for {rust} {{\n    fn save(&self) -> tsx::hot::Value {{\n        match self {{\n{save}        }}\n    }}\n    fn load(value: &tsx::hot::Value) -> Option<Self> {{\n        let items = value.as_array()?;\n        match items.first()?.as_u64() {{\n{load}            _ => None,\n        }}\n    }}\n}}\n\n"
    );
    // data members only (a function member has no JSON form)
    if types_list
        .iter()
        .all(|member| super::json::check(g, *member).is_ok())
    {
        declaration.push_str(&super::json::union_impl(&rust, &json_variants));
    }
    let ty = model::add(
        g,
        Info {
            kind: InfoKind::Union,
            rust: rust.clone(),
            params: types_list,
            members: variants.into_iter().map(String::into_bytes).collect(),
            fields: discriminant
                .map(|(field, _)| {
                    vec![Field {
                        name: field,
                        ty: Type::String,
                        optional: false,
                        rust: None,
                    }]
                })
                .unwrap_or_default(),
            declaration: Some(declaration),
            ..Info::default()
        },
    )?;
    if let Some(name) = name {
        g.type_aliases.push(super::TypeAlias {
            name: String::from_utf8_lossy(name).into_owned(),
            ty,
        });
    }
    Ok(ty)
}

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
    let value = super::aggregate::object(g, c, loc, properties, Some(variant.ty))?;
    Ok(Value::new(
        ty,
        format!("{}::{}({})", ty.rust(g), variant.rust, value.code),
    ))
}

/// `match` over a union value, one arm per variant.
fn arms(g: &Generator<'_>, ty: Type, arm: impl Fn(&Variant) -> String) -> String {
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
    if let Some(text) = super::exceptions::type_of(g, &value) {
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
    if let Some(text) = super::exceptions::text(g, value) {
        return Ok(Some(text));
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
    if binding.is_some_and(|index| c.bindings[index].mutable) {
        return Ok(false);
    }
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
