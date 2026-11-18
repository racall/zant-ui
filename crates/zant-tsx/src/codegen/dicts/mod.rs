//! Dictionaries: `Record<K, V>` / `{ [key: string]: V }`, `Map<K, V>` and
//! `Set<T>`, on the runtime's insertion-ordered `tsx::Dict` / `tsx::Set`.
//!
//! - A record is read and written with `[]` and `.` (`labels[key]`,
//!   `labels.title = …`); a read of a string-keyed record is `V | undefined`
//!   (TypeScript's `noUncheckedIndexedAccess`), of a union-keyed one `V`.
//! - `Map` / `Set` use their methods (`get`, `set`, `has`, `delete`, `add`,
//!   `clear`, `size`, `keys()`, `values()`, `entries()`); iterators are
//!   arrays here.
//! - `Object.keys / values / entries / fromEntries` read records (and the
//!   field names / values of plain objects).
//!
//! Keys are strings or string unions (records), and numbers or booleans
//! too (maps and sets).

use crate::ast;

use super::expr;
use super::model::{self, Info, InfoKind};
use super::place;
use super::rust_text::{self, Precedence};
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

mod access;
mod maps;
mod objects;

pub use self::access::*;
pub use self::maps::*;
pub use self::objects::*;

/// `Record<K, V>`.
pub fn record_of(g: &mut Generator<'_>, key: Type, value: Type) -> Result<Type> {
    intern(g, InfoKind::Record, Some(key), value)
}

fn intern(g: &mut Generator<'_>, kind: InfoKind, key: Option<Type>, value: Type) -> Result<Type> {
    let params: Vec<Type> = key.into_iter().collect();
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.kind == kind && entry.child == value && entry.params == params {
            return Ok(Type::Compound(index as u32));
        }
    }
    let rust = match key {
        Some(key) => format!("tsx::Dict<{}, {}>", key.rust(g), value.rust(g)),
        None => format!("tsx::Set<{}>", value.rust(g)),
    };
    model::add(
        g,
        Info {
            kind,
            rust,
            child: value,
            params,
            ..Info::default()
        },
    )
}

/// The kind of dictionary `ty` is, if any.
pub fn kind(g: &Generator<'_>, ty: Type) -> Option<InfoKind> {
    model::info(g, ty)
        .map(|info| info.kind)
        .filter(|kind| matches!(kind, InfoKind::Record | InfoKind::Map | InfoKind::Set))
}

fn key_of(g: &Generator<'_>, ty: Type) -> Type {
    model::info(g, ty)
        .and_then(|info| info.params.first().copied())
        .unwrap_or(Type::Void)
}

fn value_of(g: &Generator<'_>, ty: Type) -> Type {
    model::info(g, ty)
        .map(|info| info.child)
        .unwrap_or(Type::Void)
}

/// `Record<K, V>`, `Map<K, V>`, `Set<T>` (and `ReadonlyMap` / `ReadonlySet`).
pub fn named(g: &mut Generator<'_>, reference: &ast::TypeRef) -> Result<Option<Type>> {
    let name = reference.name.as_slice();
    let arity = match name {
        b"Record" | b"Map" | b"ReadonlyMap" => 2,
        b"Set" | b"ReadonlySet" => 1,
        _ => return Ok(None),
    };
    let text = String::from_utf8_lossy(name).into_owned();
    if reference.args.len() != arity {
        return fail(
            g,
            reference.loc,
            format!("`{text}` takes {arity} type arguments"),
        );
    }
    let mut args = Vec::new();
    for arg in &reference.args {
        args.push(types::type_ref(g, arg, false)?.concrete());
    }
    let key = args[0];
    let string_key = key == Type::String || model::enum_type(g, key);
    let any_key = string_key || key == Type::F64 || key == Type::Boolean;
    match name {
        b"Record" if !string_key => fail(
            g,
            reference.args[0].loc,
            "record keys are strings or a string union; use Map<number, V> for number keys",
        ),
        b"Record" => intern(g, InfoKind::Record, Some(key), args[1]).map(Some),
        _ if !any_key => fail(
            g,
            reference.args[0].loc,
            "map and set keys are strings, numbers, booleans or a string union",
        ),
        b"Map" | b"ReadonlyMap" => intern(g, InfoKind::Map, Some(key), args[1]).map(Some),
        _ => intern(g, InfoKind::Set, None, key).map(Some),
    }
}

/// A key expression as the key type (`"a"`, a string, an enum member).
fn key_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    key: Type,
    e: &ast::Expr,
) -> Result<String> {
    let value = expr::emit(g, c, e, Some(key), false)?;
    Ok(types::coerce(g, value, key, e.loc)?.code)
}

/// A field name used as a record key (`labels.title`, `{ title: … }`).
fn literal_key(g: &mut Generator<'_>, key: Type, name: &ast::Name) -> Result<String> {
    let literal = Value {
        ty: Type::String,
        code: format!(
            "zant::Str::from({})",
            rust_text::quote_at(g, &name.name, name.loc)?
        ),
        text: Some(name.name.clone()),
        ..Value::default()
    };
    Ok(types::coerce(g, literal, key, name.loc)?.code)
}

/// `{ a: 1, ...base }` where a record is expected.
pub fn literal(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    properties: &[ast::Property],
    ty: Type,
) -> Result<Value> {
    let key = key_of(g, ty);
    let value_ty = value_of(g, ty);
    let mut code = format!("{{\nlet mut _dict: {} = tsx::Dict::new();\n", ty.rust(g));
    let mut seen: Vec<Vec<u8>> = Vec::new();
    for property in properties {
        if property.spread {
            let base = expr::emit(g, c, &property.value, Some(ty), false)?;
            let base = types::coerce(g, base, ty, property.value.loc)?;
            code.push_str(&format!(
                "for (_key, _value) in {}.iter() {{ _dict.insert(_key.clone(), _value.clone()); }}\n",
                rust_text::operand(&base, Precedence::Suffix, false)
            ));
            seen.clear();
            continue;
        }
        let key_code = literal_key(g, key, &property.key)?;
        let value = expr::emit(g, c, &property.value, Some(value_ty), false)?;
        let value = types::coerce(g, value, value_ty, property.value.loc)?;
        code.push_str(&format!("_dict.insert({key_code}, {});\n", value.code));
        seen.push(property.key.name.clone());
    }
    // a union-keyed record has every key (TypeScript requires them)
    if model::enum_type(g, key) && !properties.iter().any(|property| property.spread) {
        let members = model::info(g, key)
            .map(|info| info.members)
            .unwrap_or_default();
        if let Some(missing) = members.iter().find(|member| !seen.contains(member)) {
            return fail(
                g,
                loc,
                format!("missing key `{}`", String::from_utf8_lossy(missing)),
            );
        }
    }
    code.push_str("_dict\n}");
    Ok(Value::new(ty, code))
}
