//! TypeScript's structural typing between object types: a value converts to
//! any object type whose fields it has (`types::coerce` asks here).

use crate::ast;

use super::model;
use super::rust_text;
use super::types::{Type, Value, coerce};
use super::{Generator, Result, fail};

/// TypeScript's structural assignability between object types: a value
/// whose type has every field the target requires (with assignable types)
/// converts to the target struct, field by field; extra fields are left
/// out and missing optional ones are `None`. `None` when either side is not
/// a known object type.
pub fn convert(g: &mut Generator<'_>, v: &Value, t: Type, loc: ast::Loc) -> Result<Option<Value>> {
    let (Some(source), Some(target)) = (model::info(g, v.ty), model::info(g, t)) else {
        return Ok(None);
    };
    let known = |info: &model::Info| {
        info.kind == model::InfoKind::Object
            && (info.declaration.is_some() || !info.fields.is_empty())
    };
    if !known(&source) || !known(&target) || target.rust.starts_with("zant::") {
        return Ok(None);
    }
    let mut fields = Vec::new();
    for field in &target.fields {
        let name = String::from_utf8_lossy(&field.name).into_owned();
        let rust = field
            .rust
            .clone()
            .map_or_else(|| rust_text::id_at(g, &field.name, loc), Ok)?;
        let Some(from) = source
            .fields
            .iter()
            .find(|candidate| candidate.name == field.name)
        else {
            if field.optional {
                fields.push(format!("{rust}: None"));
                continue;
            }
            return fail(
                g,
                loc,
                format!(
                    "{} is missing field `{name}` that {} requires",
                    v.ty.rust(g),
                    t.rust(g)
                ),
            );
        };
        let from_rust = from
            .rust
            .clone()
            .map_or_else(|| rust_text::id_at(g, &from.name, loc), Ok)?;
        let value = coerce(
            g,
            Value::new(from.ty, format!("_from.{from_rust}")),
            field.ty,
            loc,
        )?;
        fields.push(format!("{rust}: {}", value.code));
    }
    let source_rust = v.ty.rust(g).to_owned();
    Ok(Some(Value {
        ty: t,
        code: format!(
            "{{ let _from: {source_rust} = {}; {} {{ {} }} }}",
            v.code,
            rust_text::expression_path(t.rust(g)),
            fields.join(", ")
        ),
        ..Value::default()
    }))
}
