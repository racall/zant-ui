//! Interned compound types: arrays, optionals, unions / string-literal enums
//! and interface structs.
//!
//! A compound is a [`types::Type::Compound`], an index into
//! [`Generator::compounds`]. Interface declarations are emitted as
//! `#[derive(Clone, PartialEq, Default)] pub struct` and string-literal unions
//! as `#[derive(Clone, Copy, Default, PartialEq, Eq)] pub enum` with an
//! `as_str()` accessor.

use crate::ast;
use crate::tags;

use super::rust_text;
use super::types::{self, Type};
use super::{Generator, Result, fail};

mod lookup;
mod objects;
#[cfg(test)]
mod tests;
mod variants;

pub use self::lookup::*;
pub use self::objects::*;
pub(in crate::codegen) use self::variants::*;

#[derive(Clone, Debug)]
pub struct Field {
    pub name: Vec<u8>,
    pub ty: Type,
    pub optional: bool,
    /// The Rust field when it differs from the TSX name (`colorPrimary` →
    /// `color_primary` on the theme).
    pub rust: Option<String>,
}

/// The Rust spelling of field `name` of `ty` (its TSX name unless aliased).
pub fn field_rust(g: &Generator<'_>, ty: Type, name: &[u8]) -> Option<String> {
    let entry = info(g, ty)?;
    entry
        .fields
        .iter()
        .find(|field| field.name == name)
        .and_then(|field| field.rust.clone())
}

/// `zant::theme::Theme` as an object type with camelCase fields (`useTheme()`).
pub fn theme_type(g: &mut Generator<'_>) -> Result<Type> {
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.rust == "zant::theme::tokens::Theme" {
            return Ok(Type::Compound(index as u32));
        }
    }
    let fields = tags::theme_fields()
        .iter()
        .map(|prop| Field {
            name: prop.name.as_bytes().to_vec(),
            ty: if prop.kind == tags::Kind::Number {
                Type::F32
            } else {
                Type::U32
            },
            optional: false,
            rust: Some(prop.field.to_owned()),
        })
        .collect();
    add(
        g,
        Info {
            kind: InfoKind::Object,
            rust: "zant::theme::tokens::Theme".to_owned(),
            copy: true,
            fields,
            ..Info::default()
        },
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InfoKind {
    #[default]
    Array,
    Object,
    Optional,
    Enumeration,
    Callback,
    /// `[A, B]`: the element types are `params`.
    Tuple,
    /// `Record<K, V>` / `{ [key: string]: V }`: `params[0]` is K, `child` V.
    Record,
    /// `Map<K, V>`: `params[0]` is K, `child` V.
    Map,
    /// `Set<T>`: `child` is T.
    Set,
    /// `string | number`, discriminated object unions (`unions`).
    Union,
    /// `(A) => R` with a return value: `params` are A, `child` R (`closures`).
    Function,
}

#[derive(Clone, Debug)]
pub struct Info {
    pub kind: InfoKind,
    /// The Rust type spelling.
    pub rust: String,
    pub child: Type,
    pub fields: Vec<Field>,
    pub members: Vec<Vec<u8>>,
    pub declaration: Option<String>,
    /// `.callback`: the parameter types (callbacks return nothing).
    pub params: Vec<Type>,
    /// Whether the type is `Copy` (string-literal enums, the theme).
    pub copy: bool,
}

impl Default for Info {
    fn default() -> Self {
        Info {
            kind: InfoKind::Array,
            rust: String::new(),
            child: Type::Void,
            fields: Vec::new(),
            members: Vec::new(),
            declaration: None,
            params: Vec::new(),
            copy: false,
        }
    }
}

/// The info of a compound type, cloned so callers may keep mutating the
/// generator.
pub fn info(g: &Generator<'_>, ty: Type) -> Option<Info> {
    match ty {
        Type::Compound(index) => g.compounds.get(index as usize).cloned(),
        _ => None,
    }
}

fn info_mut<'g>(g: &'g mut Generator<'_>, ty: Type) -> &'g mut Info {
    match ty {
        Type::Compound(index) => &mut g.compounds[index as usize],
        _ => unreachable!("compound type expected"),
    }
}

pub fn add(g: &mut Generator<'_>, value: Info) -> Result<Type> {
    let index = g.compounds.len() as u32;
    g.compounds.push(value);
    Ok(Type::Compound(index))
}

pub fn array(g: &mut Generator<'_>, child: Type) -> Result<Type> {
    for (index, entry) in g.compounds.iter().enumerate() {
        // `tsx::Bytes` / `&'static [u8]` are interned as byte arrays too
        // (`rust_imports::bytes`, `zant_api::bytes_owned`); a `u8[]` must not
        // reuse them, so only `tsx::List` entries intern as general arrays.
        if entry.kind == InfoKind::Array
            && entry.child == child
            && entry.rust.starts_with("tsx::List<")
        {
            return Ok(Type::Compound(index as u32));
        }
    }
    let rust = format!("tsx::List<{}>", child.rust(g));
    add(
        g,
        Info {
            kind: InfoKind::Array,
            rust,
            child,
            ..Info::default()
        },
    )
}

/// Whether `ty` is the `Uint8Array` type: the runtime's static byte slice
/// (`&'static [u8]`, see `rust_imports::bytes`), not a `tsx::List<u8>`.
pub fn is_static_bytes(g: &Generator<'_>, ty: Type) -> bool {
    matches!(
        ty,
        Type::Compound(index) if g.compounds[index as usize].rust == "&'static [u8]"
    )
}

pub fn optional(g: &mut Generator<'_>, child: Type) -> Result<Type> {
    if child == Type::Null {
        return Ok(child);
    }
    if child == Type::Node {
        return Ok(Type::OptionalNode);
    }
    if optional_child(g, child).is_some() {
        return Ok(child);
    }
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.kind == InfoKind::Optional && entry.child == child {
            return Ok(Type::Compound(index as u32));
        }
    }
    let rust = format!("Option<{}>", child.rust(g));
    add(
        g,
        Info {
            kind: InfoKind::Optional,
            rust,
            child,
            copy: child.copy(g),
            ..Info::default()
        },
    )
}

pub fn optional_child(g: &Generator<'_>, ty: Type) -> Option<Type> {
    if ty == Type::OptionalNode {
        return Some(Type::Node);
    }
    let entry = info(g, ty)?;
    if entry.kind == InfoKind::Optional {
        Some(entry.child)
    } else {
        None
    }
}

pub fn element(g: &Generator<'_>, ty: Type) -> Option<Type> {
    let entry = info(g, ty)?;
    if entry.kind == InfoKind::Array {
        Some(entry.child)
    } else {
        None
    }
}

/// The parameter types when `ty` is (an optional) callback.
fn callback_params(g: &Generator<'_>, ty: Type) -> Option<Vec<Type>> {
    let entry = info(g, ty)?;
    if entry.kind == InfoKind::Callback {
        return Some(entry.params.clone());
    }
    if entry.kind == InfoKind::Optional {
        let child = info(g, entry.child)?;
        if child.kind == InfoKind::Callback {
            return Some(child.params.clone());
        }
    }
    None
}

/// Whether `ty` is an optional callback (default `None`) rather than a bare
/// callback (default no-op closure).
fn model_optional_child_is_none(g: &Generator<'_>, ty: Type) -> bool {
    info(g, ty).is_some_and(|entry| entry.kind == InfoKind::Optional)
}

pub fn owned(g: &Generator<'_>, ty: Type) -> bool {
    let Some(entry) = info(g, ty) else {
        return false;
    };
    match entry.kind {
        InfoKind::Array | InfoKind::Object => true,
        InfoKind::Optional => owned(g, entry.child) || entry.child == Type::String,
        InfoKind::Enumeration | InfoKind::Callback => false,
        InfoKind::Tuple => entry
            .params
            .iter()
            .any(|item| owned(g, *item) || *item == Type::String),
        InfoKind::Record | InfoKind::Map | InfoKind::Set | InfoKind::Union | InfoKind::Function => {
            true
        }
    }
}

pub fn enum_type(g: &Generator<'_>, ty: Type) -> bool {
    info(g, ty).is_some_and(|entry| entry.kind == InfoKind::Enumeration)
}

/// Resolve nullable unions, immutable arrays, nested objects, and literal
/// enums.
pub fn resolve(
    g: &mut Generator<'_>,
    reference: &ast::TypeRef,
    name: Option<&[u8]>,
) -> Result<Type> {
    let Some(detail) = reference.detail.as_deref() else {
        return types::type_ref(g, reference, false);
    };
    match detail {
        ast::TypeDetail::Array(child) => {
            let child = types::type_ref(g, child, false)?;
            array(g, child)
        }
        ast::TypeDetail::Object(fields) => object(g, fields, name),
        ast::TypeDetail::Union(members) => union_type(g, reference.loc, members, name),
        ast::TypeDetail::StringLit(member) => enumeration(g, std::slice::from_ref(member), name),
        ast::TypeDetail::Null => Ok(Type::Null),
        ast::TypeDetail::Func(f) => callback_type(g, reference.loc, f),
        ast::TypeDetail::Intersection(members) => intersection(g, members, name),
        ast::TypeDetail::Keyof(inner) => super::type_ops::keyof(g, inner, name),
        ast::TypeDetail::Typeof(value) => super::type_ops::type_of(g, value),
        ast::TypeDetail::Indexed(inner, key) => {
            super::type_ops::indexed(g, inner, key, reference.loc)
        }
        // number literal types are checked as `number`
        ast::TypeDetail::NumberLit(_) => Ok(Type::F64),
        ast::TypeDetail::Tuple(members) => {
            let mut items = Vec::with_capacity(members.len());
            for member in members {
                items.push(types::type_ref(g, member, false)?);
            }
            tuple(g, items)
        }
    }
}
