//! Type operators: `keyof T`, `typeof constant` and `T["field"]`.
//!
//! `keyof` of an object type is the union of its field names (a string
//! union enum, so `obj[key]` can select a field), of a record its key type.
//! `typeof` reads a top-level constant's annotated or inferred type.

use crate::ast;

use super::expr;
use super::model::{self, InfoKind};
use super::scope;
use super::types::{self, Type};
use super::{Generator, Result, fail};

/// `keyof T`.
pub fn keyof(g: &mut Generator<'_>, inner: &ast::TypeRef, name: Option<&[u8]>) -> Result<Type> {
    let ty = types::type_ref(g, inner, false)?;
    let Some(info) = model::info(g, ty) else {
        return fail(g, inner.loc, "`keyof` takes an object or record type");
    };
    match info.kind {
        InfoKind::Object => {
            if info.fields.is_empty() {
                return fail(g, inner.loc, "`keyof` of a type without fields");
            }
            let names: Vec<Vec<u8>> = info.fields.iter().map(|field| field.name.clone()).collect();
            model::enumeration(g, &names, name)
        }
        InfoKind::Record => Ok(info.params[0]),
        _ => fail(g, inner.loc, "`keyof` takes an object or record type"),
    }
}

/// `typeof constant` (a top-level constant of this file).
pub fn type_of(g: &mut Generator<'_>, value: &ast::Name) -> Result<Type> {
    let Some(constant) = g
        .const_decls
        .iter()
        .find(|constant| constant.name.name == value.name)
        .cloned()
    else {
        return fail(
            g,
            value.loc,
            format!(
                "`typeof {}`: only top-level constants of this file can be named here",
                String::from_utf8_lossy(&value.name)
            ),
        );
    };
    if let Some(annotation) = constant.ty.as_ref() {
        return types::type_ref(g, annotation, false);
    }
    // the type the initializer has (an object literal gets its inferred,
    // shape-shared struct, the one the constant itself will have)
    let mut c = scope::Context {
        render: true,
        constant: true,
        ret: Type::Void,
        ..scope::Context::default()
    };
    let initial = expr::emit(g, &mut c, &constant.value, None, false)?;
    Ok(initial.ty.concrete())
}

/// `T["field"]`.
pub fn indexed(
    g: &mut Generator<'_>,
    inner: &ast::TypeRef,
    key: &[u8],
    loc: ast::Loc,
) -> Result<Type> {
    let ty = types::type_ref(g, inner, false)?;
    if let Some(info) = model::info(g, ty)
        && matches!(info.kind, InfoKind::Record | InfoKind::Map)
    {
        return Ok(info.child);
    }
    model::field_type(g, ty, key, loc)
}

/// `obj[key]` where `key` is a field-name union (`keyof typeof obj`): a
/// `match` over the fields, which must share one type. `None` when the
/// object is not a struct.
pub fn field_by_key(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    object: &super::types::Value,
    key: &ast::Expr,
) -> Result<Option<super::types::Value>> {
    let Some(info) = model::info(g, object.ty).filter(|info| info.kind == InfoKind::Object) else {
        return Ok(None);
    };
    let value = expr::emit(g, c, key, None, false)?;
    let Some(keys) = model::info(g, value.ty).filter(|keys| keys.kind == InfoKind::Enumeration)
    else {
        return fail(
            g,
            key.loc,
            "an object is indexed by a key union (`keyof typeof obj`); use a Record for string keys",
        );
    };
    let mut item: Option<Type> = None;
    let mut arms = String::new();
    let operand = super::rust_text::operand(object, super::rust_text::Precedence::Suffix, false);
    for member in &keys.members {
        let Some(field) = info.fields.iter().find(|field| field.name == *member) else {
            return fail(
                g,
                key.loc,
                format!(
                    "`{}` is not a field of this object",
                    String::from_utf8_lossy(member)
                ),
            );
        };
        match item {
            Some(ty) if ty != field.ty => {
                return fail(
                    g,
                    key.loc,
                    "the selected fields have different types; read them one by one or use a Record",
                );
            }
            _ => item = Some(field.ty),
        }
        let Some(variant) = super::rust_text::enum_variant(member) else {
            return fail(g, key.loc, "this key cannot be a union member");
        };
        let rust = match &field.rust {
            Some(rust) => rust.clone(),
            None => super::rust_text::id_at(g, &field.name, key.loc)?,
        };
        arms.push_str(&format!(
            "{}::{variant} => _object.{rust}.clone(),\n",
            keys.rust
        ));
    }
    let ty = item.unwrap_or(Type::Void);
    Ok(Some(super::types::Value::new(
        ty,
        format!(
            "{{ let _object = &{operand}; match {} {{\n{arms}}} }}",
            value.code
        ),
    )))
}
