//! Member and index reads, optional chaining included.

use super::*;

/// Optional access evaluates its receiver once and preserves null through the
/// member.
pub fn member(g: &mut Generator<'_>, c: &mut scope::Context, m: &ast::Member) -> Result<Value> {
    if let Some(value) = crate::codegen::enums::member(g, c, m)? {
        return Ok(value);
    }
    if !c.narrowed.is_empty() && !m.optional {
        // `s.prompt` narrowed by `s.prompt !== null && …`
        let expression = ast::Expr {
            loc: m.name.loc,
            data: ast::ExprData::Member(m.clone()),
        };
        if let Some(path) = crate::codegen::narrowing::path(&expression)
            && let Some(value) = crate::codegen::narrowing::find_path(
                c,
                &path,
                &crate::codegen::narrowing::base(&expression),
            )
        {
            return Ok(value);
        }
    }
    if let Some(value) = builtins::member(g, c, m)? {
        return Ok(value);
    }
    if let Some(hook) = g.extensions.hooks.ref_read
        && let Some(value) = hook(g, c, m)?
    {
        return Ok(value);
    }
    crate::codegen::place::project(c, &m.object);
    let value = expr::emit(g, c, &m.object, None, false)?;
    if (m.optional || in_optional_chain(&m.object))
        && let Some(child) = model::optional_child(g, value.ty)
    {
        let name = temporary(g, c, b"_value")?;
        // `groups?.year`: a record read inside the chain
        let result =
            match crate::codegen::dicts::member(g, &Value::new(child, name.clone()), &m.name)? {
                Some(read) => read,
                None => member_read(g, child, &name, &m.name)?,
            };
        // an optional result flattens (`a?.b` where `b` may be absent)
        let (ty, adapter) = if model::optional_child(g, result.ty).is_some() {
            (result.ty, "and_then")
        } else {
            (model::optional(g, result.ty)?, "map")
        };
        return Ok(Value {
            ty,
            code: format!(
                "{}.{adapter}(|{name}| {})",
                rust_text::operand(&value, Precedence::Suffix, false),
                result.code
            ),
            precedence: Precedence::Suffix,
            ..Value::default()
        });
    }
    if model::optional_child(g, value.ty).is_some() {
        return fail(
            g,
            m.name.loc,
            "optional value needs ?? or ! before member access",
        );
    }
    if let Some(read) = crate::codegen::dicts::member(g, &value, &m.name)? {
        return Ok(read);
    }
    if let Some(read) = crate::codegen::unions::member(g, &value, &m.name)? {
        return Ok(read);
    }
    let operand = rust_text::operand(&value, Precedence::Suffix, false);
    member_read(g, value.ty, &operand, &m.name)
}

/// A member read of a value whose expression is already parenthesized to
/// suffix precedence (`operand`).
pub(super) fn member_read(
    g: &mut Generator<'_>,
    object_ty: Type,
    operand: &str,
    name: &ast::Name,
) -> Result<Value> {
    if let Some(read) = crate::codegen::regexps::member_read(g, object_ty, operand, name)? {
        return Ok(read);
    }
    if let Some(read) = crate::codegen::promises::member_read(g, object_ty, operand, name)? {
        return Ok(read);
    }
    let length = name.name == b"length"
        && (object_ty == Type::String || model::element(g, object_ty).is_some());
    if length && object_ty == Type::String {
        // strings count UTF-16 code units, as in TypeScript
        return Ok(Value {
            ty: Type::Usize,
            code: format!("tsx::utf16_length(({operand}).as_str())"),
            precedence: Precedence::Suffix,
            ..Value::default()
        });
    }
    if name.name == b"length"
        && let Some(items) = model::tuple_items(g, object_ty)
    {
        return Ok(Value::new(Type::Usize, format!("{}usize", items.len())));
    }
    if length {
        return Ok(Value {
            ty: Type::Usize,
            code: format!("{operand}.len()"),
            precedence: Precedence::Suffix,
            ..Value::default()
        });
    }
    let ty = model::field_type(g, object_ty, &name.name, name.loc)?;
    let field = match model::field_rust(g, object_ty, &name.name) {
        Some(field) => field,
        None => rust_text::id_at(g, &name.name, name.loc)?,
    };
    let raw = format!("{operand}.{field}");
    if ty.copy(g) {
        Ok(Value::new(ty, raw))
    } else {
        Ok(Value {
            ty,
            code: format!("{raw}.clone()"),
            precedence: Precedence::Suffix,
            ..Value::default()
        })
    }
}

/// Numeric indexes are checked by `tsx::index` before Rust's own slice bounds
/// check.
pub fn index(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    ix: &ast::Index,
) -> Result<Value> {
    crate::codegen::place::project(c, &ix.object);
    let value = expr::emit(g, c, &ix.object, None, false)?;
    if let Some(read) = crate::codegen::regexps::index(g, c, &value, &ix.index)? {
        return Ok(read);
    }
    if crate::codegen::dicts::kind(g, value.ty).is_some() {
        return crate::codegen::dicts::index(g, c, value, &ix.index);
    }
    if let Some(read) = crate::codegen::type_ops::field_by_key(g, c, &value, &ix.index)? {
        return Ok(read);
    }
    if let Some(items) = model::tuple_items(g, value.ty) {
        // `pair[1]`: a literal position
        let position = match &ix.index.data {
            ast::ExprData::Number(n) => std::str::from_utf8(n)
                .ok()
                .and_then(|n| n.parse::<usize>().ok()),
            _ => None,
        };
        let Some(position) = position.filter(|position| *position < items.len()) else {
            return fail(
                g,
                ix.index.loc,
                format!(
                    "index a tuple with a literal position 0..{}",
                    items.len() - 1
                ),
            );
        };
        let item = items[position];
        let operand = rust_text::operand(&value, Precedence::Suffix, false);
        let code = if item.copy(g) {
            format!("{operand}.{position}")
        } else {
            format!("{operand}.{position}.clone()")
        };
        return Ok(Value {
            ty: item,
            code,
            precedence: Precedence::Suffix,
            ..Value::default()
        });
    }
    let ty = unwrap(g, value.ty);
    if !ix.optional && ty != value.ty {
        return fail(g, loc, "optional array needs ?? or !");
    }
    let Some(child) = model::element(g, ty) else {
        return fail(g, loc, "indexing needs an array");
    };
    let position = expr::emit(g, c, &ix.index, None, false)?;
    if model::optional_child(g, position.ty).is_some() {
        return fail(g, ix.index.loc, "optional array index needs ?? or !");
    }
    if !position.ty.numeric() {
        return fail(g, ix.index.loc, "array index must be numeric");
    }
    let at = format!("tsx::index({})", types::concrete(g, position)?.code);
    let child_copy = child.copy(g);
    let access = |code: &str| {
        if child_copy {
            format!("{code}[{at}]")
        } else {
            format!("{code}[{at}].clone()")
        }
    };
    if ty != value.ty {
        let name = temporary(g, c, b"_array")?;
        let body = access(&name);
        return Ok(Value {
            ty: model::optional(g, child)?,
            code: format!(
                "{}.map(|{name}| {body})",
                rust_text::operand(&value, Precedence::Suffix, false)
            ),
            precedence: Precedence::Suffix,
            ..Value::default()
        });
    }
    Ok(Value {
        ty: child,
        code: access(&rust_text::operand(&value, Precedence::Suffix, false)),
        precedence: Precedence::Suffix,
        ..Value::default()
    })
}

/// `e` ends an optional chain (`a?.b`, `a?.b.c`, `f?.()`): by JavaScript's
/// short-circuiting, the member or call that follows it applies only when
/// the chain did not stop, so it continues inside the optional
/// (`text?.trim().toUpperCase()` is `text.map(|t| t.trim().to_upper())`).
pub(in crate::codegen) fn in_optional_chain(e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::Member(m) => m.optional || in_optional_chain(&m.object),
        ast::ExprData::Call(call) => call.optional || in_optional_chain(&call.callee),
        ast::ExprData::Index(i) => i.optional || in_optional_chain(&i.object),
        ast::ExprData::NonNull(inner) => in_optional_chain(inner),
        _ => false,
    }
}
