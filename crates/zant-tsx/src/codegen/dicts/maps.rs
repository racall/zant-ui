//! `new Map` / `new Set` and their methods (reading and mutating).

use super::*;

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
pub(super) fn mutate(
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
