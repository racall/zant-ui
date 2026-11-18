//! Reading and calling regexp and match values: members, indexes, methods, `lastIndex`.

use super::*;

/// `re.source`, `m.index`, `m.groups`, …; `None` for other types.
pub fn member_read(
    g: &mut Generator<'_>,
    ty: Type,
    operand: &str,
    name: &ast::Name,
) -> Result<Option<Value>> {
    let field = String::from_utf8_lossy(&name.name).into_owned();
    if is_regexp(g, ty) {
        let (ty, method) = match field.as_str() {
            "source" => (Type::String, "source"),
            "flags" => (Type::String, "flags"),
            "global" => (Type::Boolean, "global"),
            "ignoreCase" => (Type::Boolean, "ignore_case"),
            "multiline" => (Type::Boolean, "multiline"),
            "sticky" => (Type::Boolean, "sticky"),
            "unicode" => (Type::Boolean, "unicode"),
            "dotAll" => (Type::Boolean, "dot_all"),
            "hasIndices" => (Type::Boolean, "has_indices"),
            "lastIndex" => (Type::F64, "last_index"),
            _ => {
                return fail(
                    g,
                    name.loc,
                    format!(
                        "RegExp has no `{field}` (source, flags, global, ignoreCase, multiline, sticky, unicode, dotAll, hasIndices, lastIndex)"
                    ),
                );
            }
        };
        return Ok(Some(Value::new(ty, format!("{operand}.{method}()"))));
    }
    if is_match(g, ty) {
        return match field.as_str() {
            "index" => Ok(Some(Value::new(Type::F64, format!("{operand}.index()")))),
            "length" => Ok(Some(Value::new(Type::F64, format!("{operand}.len()")))),
            "input" => Ok(Some(Value::new(Type::String, format!("{operand}.input()")))),
            "groups" => {
                let record = crate::codegen::dicts::record_of(g, Type::String, Type::String)?;
                let ty = model::optional(g, record)?;
                Ok(Some(Value::new(ty, format!("{operand}.groups()"))))
            }
            _ => fail(
                g,
                name.loc,
                format!("a match has no `{field}` (index, input, groups, length, m[i])"),
            ),
        };
    }
    Ok(None)
}

/// `m[i]` on a match: a `string` (an unmatched capture reads as "").
pub fn index(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    object: &Value,
    position: &ast::Expr,
) -> Result<Option<Value>> {
    if !is_match(g, object.ty) {
        return Ok(None);
    }
    let at = expr::emit(g, c, position, Some(Type::F64), false)?;
    let at = types::coerce(g, at, Type::F64, position.loc)?;
    Ok(Some(Value::new(
        Type::String,
        format!(
            "{}.get({}).unwrap_or_default()",
            rust_text::operand(object, Precedence::Suffix, false),
            at.code
        ),
    )))
}

/// `re.test(s)` / `re.exec(s)`, and array methods on a match
/// (`m.map(…)`, `m.slice(1)`); `None` for other receivers.
pub fn method(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    receiver: &Value,
) -> Result<Option<Value>> {
    let ast::ExprData::Member(member) = &call.callee.data else {
        return Ok(None);
    };
    let name = member.name.name.as_slice();
    if is_match(g, receiver.ty) {
        // a match is an array of strings
        let list = model::array(g, Type::String)?;
        let as_list = Value::new(
            list,
            format!(
                "{}.to_list()",
                rust_text::operand(receiver, Precedence::Suffix, false)
            ),
        );
        return crate::codegen::array_methods::emit_on(g, c, loc, call, None, as_list).map(Some);
    }
    if !is_regexp(g, receiver.ty) {
        return Ok(None);
    }
    if call.args.len() != 1 || !matches!(name, b"test" | b"exec") {
        return fail(
            g,
            loc,
            "RegExp methods: `re.test(text)`, `re.exec(text)`; strings have match / matchAll / replace / split / search",
        );
    }
    let text = expr::emit(g, c, &call.args[0], Some(Type::String), false)?;
    let text = types::coerce(g, text, Type::String, call.args[0].loc)?;
    let re = rust_text::operand(receiver, Precedence::Suffix, false);
    let text = rust_text::operand(&text, Precedence::Suffix, false);
    if name == b"test" {
        return Ok(Some(Value::new(
            Type::Boolean,
            format!("{re}.test(&{text})"),
        )));
    }
    let found = match_type(g)?;
    let ty = model::optional(g, found)?;
    Ok(Some(Value::new(
        ty,
        format!("zant::tsx::regexp::exec(&{re}, &{text})"),
    )))
}

/// `re.lastIndex = n`: the one writable property; `None` for other targets.
pub fn assign(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    target: &ast::Expr,
    value: &ast::Expr,
) -> Result<Option<Value>> {
    let ast::ExprData::Member(member) = &target.data else {
        return Ok(None);
    };
    if member.name.name != b"lastIndex" {
        return Ok(None);
    }
    crate::codegen::place::project(c, &member.object);
    let object = expr::emit(g, c, &member.object, None, false)?;
    if !is_regexp(g, object.ty) {
        return Ok(None);
    }
    let index = expr::emit(g, c, value, Some(Type::F64), false)?;
    let index = types::coerce(g, index, Type::F64, value.loc)?;
    Ok(Some(Value::new(
        Type::Void,
        format!(
            "{}.set_last_index({})",
            rust_text::operand(&object, Precedence::Suffix, false),
            index.code
        ),
    )))
}
