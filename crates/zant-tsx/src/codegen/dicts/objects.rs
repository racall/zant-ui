//! `Object.keys / values / entries / fromEntries`.

use super::*;

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
