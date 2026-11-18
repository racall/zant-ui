//! Reading and writing entries: index, member, assignment, `delete` and `in`.

use super::*;

/// A read `record[key]` / `record.key`.
pub(super) fn read(
    g: &mut Generator<'_>,
    operand: &str,
    ty: Type,
    key_code: String,
) -> Result<Value> {
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
pub(super) fn place_type(
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
