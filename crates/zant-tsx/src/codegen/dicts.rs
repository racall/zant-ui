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

/// A read `record[key]` / `record.key`.
fn read(g: &mut Generator<'_>, operand: &str, ty: Type, key_code: String) -> Result<Value> {
    let key = key_of(g, ty);
    let value = value_of(g, ty);
    if model::enum_type(g, key) {
        return Ok(Value {
            ty: value,
            code: format!(
                "{operand}.get(&{key_code}).expect(\"the record has every key of its union\")"
            ),
            precedence: Precedence::Suffix,
            ..Value::default()
        });
    }
    let optional = model::optional(g, value)?;
    Ok(Value {
        ty: optional,
        code: format!("{operand}.get(&{key_code})"),
        precedence: Precedence::Suffix,
        ..Value::default()
    })
}

/// `record[key]` (`aggregate::index`).
pub fn index(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    object: Value,
    key: &ast::Expr,
) -> Result<Value> {
    if kind(g, object.ty) != Some(InfoKind::Record) {
        return fail(
            g,
            key.loc,
            "only records are indexed by key; maps use `.get(key)`",
        );
    }
    let key_ty = key_of(g, object.ty);
    let key_code = key_value(g, c, key_ty, key)?;
    let operand = rust_text::operand(&object, Precedence::Suffix, false);
    read(g, &operand, object.ty, key_code)
}

/// `record.key` / `map.size` / `set.size` (`aggregate::member`).
pub fn member(g: &mut Generator<'_>, object: &Value, name: &ast::Name) -> Result<Option<Value>> {
    let Some(kind) = kind(g, object.ty) else {
        return Ok(None);
    };
    let operand = rust_text::operand(object, Precedence::Suffix, false);
    if kind != InfoKind::Record {
        if name.name == b"size" {
            return Ok(Some(Value::new(Type::Usize, format!("{operand}.len()"))));
        }
        return fail(
            g,
            name.loc,
            format!(
                "`{}` is not a property of a map or set; call its methods (`get`, `has`, `size`)",
                String::from_utf8_lossy(&name.name)
            ),
        );
    }
    let key_code = literal_key(g, key_of(g, object.ty), name)?;
    read(g, &operand, object.ty, key_code).map(Some)
}

/// `record[key] = v` / `record.key = v` (`place::assign`): the code, or
/// `None` when the target is not a record entry.
pub fn assign(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    target: &ast::Expr,
    value: &ast::Expr,
) -> Result<Option<String>> {
    let (object, key) = match &target.data {
        ast::ExprData::Index(index) => (&index.object, Some(&index.index)),
        ast::ExprData::Member(member) => (&member.object, None),
        _ => return Ok(None),
    };
    let Some(record) = place_type(g, c, object)? else {
        return Ok(None);
    };
    let place = place::resolve(g, c, object)?;
    let key_ty = key_of(g, record);
    let key_code = match (key, &target.data) {
        (Some(key), _) => key_value(g, c, key_ty, key)?,
        (None, ast::ExprData::Member(member)) => literal_key(g, key_ty, &member.name)?,
        _ => unreachable!("record target"),
    };
    let value_ty = value_of(g, record);
    let value = expr::emit(g, c, value, Some(value_ty), false)?;
    let value = types::coerce(g, value, value_ty, target.loc)?;
    Ok(Some(format!(
        "{{\nlet _key: {} = {key_code};\nlet _value: {} = {};\n{}.insert(_key, _value);\n}}\n",
        key_ty.rust(g),
        value_ty.rust(g),
        value.code,
        place.code
    )))
}

/// The record type of `object` when it is one (without emitting it twice
/// for anything but a plain local).
fn place_type(
    g: &mut Generator<'_>,
    c: &scope::Context,
    object: &ast::Expr,
) -> Result<Option<Type>> {
    let ast::ExprData::Ident(name) = &object.data else {
        return Ok(None);
    };
    let Some(binding) = scope::local(c, name) else {
        return Ok(None);
    };
    let ty = binding.ty;
    Ok((kind(g, ty) == Some(InfoKind::Record)).then_some(ty))
}

/// `delete record[key]` (a statement).
pub fn delete(g: &mut Generator<'_>, c: &mut scope::Context, target: &ast::Expr) -> Result<String> {
    let (object, key) = match &target.data {
        ast::ExprData::Index(index) => (&*index.object, Some(&*index.index)),
        ast::ExprData::Member(member) => (&*member.object, None),
        _ => {
            return fail(
                g,
                target.loc,
                "`delete` removes a record entry: `delete record[key]`",
            );
        }
    };
    let Some(record) = place_type(g, c, object)? else {
        return fail(
            g,
            target.loc,
            "`delete` removes an entry of a local record; objects keep their fields",
        );
    };
    let place = place::resolve(g, c, object)?;
    let key_ty = key_of(g, record);
    let key_code = match (key, &target.data) {
        (Some(key), _) => key_value(g, c, key_ty, key)?,
        (None, ast::ExprData::Member(member)) => literal_key(g, key_ty, &member.name)?,
        _ => unreachable!("record target"),
    };
    Ok(format!(
        "{{ let _key: {} = {key_code}; {}.remove(&_key); }}\n",
        key_ty.rust(g),
        place.code
    ))
}

/// `key in record` / `key in map`.
pub fn contains(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    key: &ast::Expr,
    object: &ast::Expr,
) -> Result<Value> {
    place::project(c, object);
    let object = expr::emit(g, c, object, None, false)?;
    if !matches!(kind(g, object.ty), Some(InfoKind::Record | InfoKind::Map)) {
        return fail(g, key.loc, "`in` checks a record's keys (`key in record`)");
    }
    let key_ty = key_of(g, object.ty);
    let key_code = key_value(g, c, key_ty, key)?;
    Ok(Value::new(
        Type::Boolean,
        format!(
            "{}.contains(&{key_code})",
            rust_text::operand(&object, Precedence::Suffix, false)
        ),
    ))
}

/// `new Map<K, V>(entries?)` / `new Set<T>(values?)`.
pub fn construct(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    expected: Option<Type>,
) -> Result<Value> {
    let ast::ExprData::Ident(name) = &call.callee.data else {
        return fail(g, loc, "`new` creates a Map or a Set here");
    };
    let set = match name.as_slice() {
        b"Map" => false,
        b"Set" => true,
        b"Promise" => {
            return fail(
                g,
                loc,
                "`new Promise(…)` is not supported; return a value from an `async` function, or use `Promise.resolve(value)`",
            );
        }
        _ => {
            return fail(
                g,
                loc,
                format!(
                    "`new {}` is not supported; `new` creates a Map or a Set here",
                    String::from_utf8_lossy(name)
                ),
            );
        }
    };
    let reference = ast::TypeRef {
        name: name.clone(),
        loc,
        detail: None,
        args: call.type_args.clone(),
    };
    let mut ty = if call.type_args.is_empty() {
        expected.filter(|ty| kind(g, *ty) == Some(if set { InfoKind::Set } else { InfoKind::Map }))
    } else {
        named(g, &reference)?
    };
    // `new Map([["a", 1], …])`: like tsc's contextual `[K, V]` tuples, the
    // first pair gives the entry types
    if ty.is_none()
        && !set
        && let Some(arg) = call.args.first()
        && let ast::ExprData::Array(items) = &arg.data
        && let Some(first) = items.first()
        && let ast::ExprData::Array(pair) = &first.data
        && pair.len() == 2
    {
        let key = expr::emit(g, c, &pair[0], None, false)?.ty.concrete();
        let value = expr::emit(g, c, &pair[1], None, false)?.ty.concrete();
        ty = Some(intern(g, InfoKind::Map, Some(key), value)?);
    }
    let source = match call.args.first() {
        Some(arg) => {
            let wanted = ty.map(|ty| {
                if set {
                    model::array(g, value_of(g, ty))
                } else {
                    let pair = model::tuple(g, vec![key_of(g, ty), value_of(g, ty)])?;
                    model::array(g, pair)
                }
            });
            let wanted = wanted.transpose()?;
            let value = expr::emit(g, c, arg, wanted, false)?;
            Some(value)
        }
        None => None,
    };
    if ty.is_none()
        && let Some(source) = &source
        && let Some(item) = model::element(g, source.ty)
    {
        // inferred from the initial entries / values
        ty = Some(if set {
            intern(g, InfoKind::Set, None, item.concrete())?
        } else {
            let Some(pair) = model::tuple_items(g, item).filter(|pair| pair.len() == 2) else {
                return fail(g, loc, "a Map starts from `[key, value]` pairs");
            };
            intern(g, InfoKind::Map, Some(pair[0]), pair[1])?
        });
    }
    let Some(ty) = ty else {
        let what = if set { "Set<T>" } else { "Map<K, V>" };
        return fail(
            g,
            loc,
            format!(
                "give the {} its types: `new {what}()`",
                if set { "set" } else { "map" }
            ),
        );
    };
    let code = match source {
        None if set => "tsx::Set::new()".to_owned(),
        None => "tsx::Dict::new()".to_owned(),
        Some(source) => {
            let operand = rust_text::operand(&source, Precedence::Suffix, false);
            if set {
                format!("tsx::Set::from_values({operand}.iter().cloned())")
            } else {
                format!("tsx::Dict::from_entries({operand}.iter().cloned())")
            }
        }
    };
    Ok(Value::new(ty, code))
}

/// `map.set(k, v)`, `map.delete(k)`, `set.add(v)`, `x.clear()` when the
/// receiver is a local map or set (`None` otherwise; nothing is emitted).
pub fn mutating_method(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    member: &ast::Member,
) -> Result<Option<Value>> {
    if !matches!(
        member.name.name.as_slice(),
        b"set" | b"delete" | b"clear" | b"add"
    ) {
        return Ok(None);
    }
    let receiver_ty = match &member.object.data {
        ast::ExprData::Ident(local) => scope::local(c, local).map(|binding| binding.ty),
        _ => None,
    };
    let Some(ty) =
        receiver_ty.filter(|ty| matches!(kind(g, *ty), Some(InfoKind::Map | InfoKind::Set)))
    else {
        return Ok(None);
    };
    mutate(g, c, loc, call, member, ty).map(Some)
}

/// A reading method of a map or set on the evaluated `receiver`; `None`
/// when it is not one.
pub fn read_method(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    member: &ast::Member,
    receiver: &Value,
) -> Result<Option<Value>> {
    let name = member.name.name.as_slice();
    let Some(kind) = kind(g, receiver.ty).filter(|kind| *kind != InfoKind::Record) else {
        return Ok(None);
    };
    let operand = rust_text::operand(receiver, Precedence::Suffix, false);
    let ty = receiver.ty;
    let arity = |g: &mut Generator<'_>, count: usize| {
        if call.args.len() != count {
            return fail(g, loc, format!("this method takes {count} arguments"));
        }
        Ok(())
    };
    let set = kind == InfoKind::Set;
    let key_ty = if set { value_of(g, ty) } else { key_of(g, ty) };
    let value = match name {
        b"get" if !set => {
            arity(g, 1)?;
            let key = key_value(g, c, key_ty, &call.args[0])?;
            let optional = model::optional(g, value_of(g, ty))?;
            Value::new(optional, format!("{operand}.get(&{key})"))
        }
        b"has" => {
            arity(g, 1)?;
            let key = key_value(g, c, key_ty, &call.args[0])?;
            Value::new(Type::Boolean, format!("{operand}.contains(&{key})"))
        }
        b"keys" | b"values" if set => {
            arity(g, 0)?;
            let list = model::array(g, key_ty)?;
            Value::new(list, format!("{operand}.values()"))
        }
        b"keys" => {
            arity(g, 0)?;
            let list = model::array(g, key_ty)?;
            Value::new(list, format!("{operand}.keys()"))
        }
        b"values" => {
            arity(g, 0)?;
            let list = model::array(g, value_of(g, ty))?;
            Value::new(list, format!("{operand}.values()"))
        }
        // `set.entries()`: each value paired with itself
        b"entries" if set => {
            arity(g, 0)?;
            let pair = model::tuple(g, vec![key_ty, key_ty])?;
            let list = model::array(g, pair)?;
            Value::new(
                list,
                format!(
                    "tsx::List::from_vec({operand}.values().iter().map(|v| (v.clone(), v.clone())).collect::<Vec<_>>())"
                ),
            )
        }
        b"entries" if !set => {
            arity(g, 0)?;
            let pair = model::tuple(g, vec![key_ty, value_of(g, ty)])?;
            let list = model::array(g, pair)?;
            Value::new(list, format!("{operand}.entries()"))
        }
        _ => {
            return fail(
                g,
                member.name.loc,
                format!(
                    "unknown {} method `{}`",
                    if set { "Set" } else { "Map" },
                    String::from_utf8_lossy(name)
                ),
            );
        }
    };
    Ok(Some(value))
}

/// `map.set(k, v)`, `map.delete(k)`, `set.add(v)`, `x.clear()` on a local.
fn mutate(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    member: &ast::Member,
    ty: Type,
) -> Result<Value> {
    let set = kind(g, ty) == Some(InfoKind::Set);
    let key_ty = if set { value_of(g, ty) } else { key_of(g, ty) };
    let name = member.name.name.as_slice();
    let expected = match name {
        b"set" if !set => 2,
        b"add" if set => 1,
        b"delete" => 1,
        b"clear" => 0,
        _ => return fail(g, member.name.loc, "unknown method"),
    };
    if call.args.len() != expected {
        return fail(g, loc, format!("this method takes {expected} arguments"));
    }
    let mut setup = String::new();
    if expected >= 1 {
        let key = key_value(g, c, key_ty, &call.args[0])?;
        setup.push_str(&format!("let _key: {} = {key};\n", key_ty.rust(g)));
    }
    if name == b"set" {
        let value_ty = value_of(g, ty);
        let value = expr::emit(g, c, &call.args[1], Some(value_ty), false)?;
        let value = types::coerce(g, value, value_ty, call.args[1].loc)?;
        setup.push_str(&format!(
            "let _value: {} = {};\n",
            value_ty.rust(g),
            value.code
        ));
    }
    let place = place::resolve(g, c, &member.object)?;
    let (result_ty, body) = match name {
        // `set` / `add` return the collection itself
        b"set" => (
            ty,
            "_target.insert(_key, _value);\n_target.clone()".to_owned(),
        ),
        b"add" => (ty, "_target.insert(_key);\n_target.clone()".to_owned()),
        b"delete" => (Type::Boolean, "_target.remove(&_key)".to_owned()),
        _ => (Type::Void, "_target.clear()".to_owned()),
    };
    Ok(Value::new(
        result_ty,
        format!("{{\n{setup}let _target = &mut {};\n{body}\n}}", place.code),
    ))
}

/// `Object.keys / values / entries / fromEntries (…)`.
pub fn object_call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<Value>> {
    let ast::ExprData::Member(member) = &call.callee.data else {
        return Ok(None);
    };
    let ast::ExprData::Ident(object) = &member.object.data else {
        return Ok(None);
    };
    if object.as_slice() != b"Object" || scope::local(c, object).is_some() {
        return Ok(None);
    }
    let name = member.name.name.as_slice();
    if !matches!(name, b"keys" | b"values" | b"entries" | b"fromEntries") {
        let hint = match name {
            b"assign" => " (copy with a spread instead: `{ ...a, ...b }`)",
            b"hasOwn" => " (test a record key with `key in record`)",
            _ => "",
        };
        return fail(
            g,
            loc,
            format!(
                "`Object.{}` is not supported; the Object functions are keys, values, entries and fromEntries{hint}",
                String::from_utf8_lossy(name)
            ),
        );
    }
    if call.args.len() != 1 {
        return fail(g, loc, "this Object function takes 1 argument");
    }
    if name == b"fromEntries" {
        let value = expr::emit(g, c, &call.args[0], None, false)?;
        let pair = model::element(g, value.ty).and_then(|item| model::tuple_items(g, item));
        let Some([key, item]) = pair
            .as_deref()
            .and_then(|pair| <[Type; 2]>::try_from(pair).ok())
        else {
            return fail(
                g,
                call.args[0].loc,
                "Object.fromEntries takes `[key, value]` pairs",
            );
        };
        if key != Type::String {
            return fail(g, call.args[0].loc, "record keys are strings");
        }
        let ty = intern(g, InfoKind::Record, Some(key), item)?;
        let operand = rust_text::operand(&value, Precedence::Suffix, false);
        return Ok(Some(Value::new(
            ty,
            format!("tsx::Dict::from_entries({operand}.iter().cloned())"),
        )));
    }
    place::project(c, &call.args[0]);
    let value = expr::emit(g, c, &call.args[0], None, false)?;
    let operand = rust_text::operand(&value, Precedence::Suffix, false);
    if kind(g, value.ty) == Some(InfoKind::Record) {
        let key = key_of(g, value.ty);
        let item = value_of(g, value.ty);
        // keys are strings, as Object.keys returns them
        let keys = if model::enum_type(g, key) {
            format!(
                "{operand}.keys().iter().map(|_key| zant::Str::Static(_key.as_str())).collect::<tsx::List<zant::Str>>()"
            )
        } else {
            format!("{operand}.keys()")
        };
        let entries_key = if model::enum_type(g, key) {
            Type::String
        } else {
            key
        };
        let value = match name {
            b"keys" => Value::new(model::array(g, Type::String)?, keys),
            b"values" => Value::new(model::array(g, item)?, format!("{operand}.values()")),
            b"entries" => {
                let pair = model::tuple(g, vec![entries_key, item])?;
                let list = model::array(g, pair)?;
                let code = if entries_key == key {
                    format!("{operand}.entries()")
                } else {
                    format!(
                        "{operand}.entries().iter().map(|(_key, _value)| (zant::Str::Static(_key.as_str()), _value.clone())).collect::<tsx::List<_>>()"
                    )
                };
                Value::new(list, code)
            }
            _ => return Ok(None),
        };
        return Ok(Some(value));
    }
    // a plain object: its field names (and values when they share a type)
    let Some(info) = model::info(g, value.ty).filter(|info| info.kind == InfoKind::Object) else {
        return fail(
            g,
            call.args[0].loc,
            "Object.keys / values / entries read records and objects",
        );
    };
    let names: Vec<String> = info
        .fields
        .iter()
        .map(|field| rust_text::quote(&field.name).map(|text| format!("zant::Str::from({text})")))
        .collect::<Option<_>>()
        .unwrap_or_default();
    match name {
        b"keys" => {
            let list = model::array(g, Type::String)?;
            Ok(Some(Value::new(
                list,
                format!("tsx::List::from_vec(vec![{}])", names.join(", ")),
            )))
        }
        b"values" | b"entries" => {
            let Some(first) = info.fields.first().map(|field| field.ty) else {
                let list = model::array(g, Type::String)?;
                return Ok(Some(Value::new(list, "tsx::List::new()".to_owned())));
            };
            if info.fields.iter().any(|field| field.ty != first) {
                return fail(
                    g,
                    call.args[0].loc,
                    "Object.values / entries of an object need fields of one type; use a Record",
                );
            }
            let mut reads = Vec::new();
            for field in &info.fields {
                let rust = match &field.rust {
                    Some(rust) => rust.clone(),
                    None => rust_text::id_at(g, &field.name, loc)?,
                };
                reads.push(format!("_object.{rust}.clone()"));
            }
            let (ty, items) = if name == b"values" {
                (model::array(g, first)?, reads)
            } else {
                let pair = model::tuple(g, vec![Type::String, first])?;
                let items = names
                    .iter()
                    .zip(reads)
                    .map(|(name, read)| format!("({name}, {read})"))
                    .collect();
                (model::array(g, pair)?, items)
            };
            Ok(Some(Value::new(
                ty,
                format!(
                    "{{ let _object = {operand}; tsx::List::from_vec(vec![{}]) }}",
                    items.join(", ")
                ),
            )))
        }
        _ => Ok(None),
    }
}
