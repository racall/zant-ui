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

mod values;
mod variants;

pub use self::values::*;
pub use self::variants::*;

pub fn is_union(g: &Generator<'_>, ty: Type) -> bool {
    model::info(g, ty).is_some_and(|info| info.kind == InfoKind::Union)
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
