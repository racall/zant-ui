//! Type hints: the numeric type an expression asks for before it is generated.

use super::*;

/// Only literal-only numeric trees take peer context; typed integers must
/// still unify.
pub fn numeric_hint(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
) -> Result<Option<Type>> {
    Ok(expression_hint(g, c, e)?.filter(|t| t.numeric() && !t.literal()))
}

/// The type an expression is expected to have, without emitting it.
pub fn expression_hint(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
) -> Result<Option<Type>> {
    let hint: Option<Type> = match &e.data {
        ast::ExprData::Number(n) => Some(if float_literal(n) {
            Type::FloatLiteral
        } else {
            Type::IntLiteral
        }),
        ast::ExprData::Ident(n) => name_hint(g, c, n),
        ast::ExprData::NonNull(inner) => expression_hint(g, c, inner)?,
        ast::ExprData::As(cast) => cast_hint(g, &cast.ty),
        ast::ExprData::Call(call) => call_hint(g, call),
        ast::ExprData::Unary(u) => {
            if u.op == b"!" {
                None
            } else {
                expression_hint(g, c, &u.operand)?
            }
        }
        ast::ExprData::Binary(b) => binary_hint(g, c, b)?,
        ast::ExprData::Cond(v) => branch_hint(g, c, v)?,
        ast::ExprData::Member(_) | ast::ExprData::Index(_) => value_hint(g, c, e)?,
        _ => None,
    };
    Ok(hint.filter(|t| t.numeric()))
}

/// A decimal literal cannot adopt an integer operand's type; it makes the
/// arithmetic floating point, even when the other side is integer.
pub fn numeric_context(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    candidate: Option<Type>,
) -> Result<Option<Type>> {
    let Some(t) = candidate else {
        return Ok(None);
    };
    let Some(hint) = expression_hint(g, c, e)? else {
        return Ok(Some(t));
    };
    if t.integer() && (hint.float() || hint == Type::FloatLiteral) {
        return Ok(None);
    }
    Ok(Some(t))
}

/// Resolve a local before consulting state, matching ordinary identifier reads.
pub(super) fn name_hint(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    name: &[u8],
) -> Option<Type> {
    if let Some(binding) = scope::local(c, name) {
        return Some(binding.ty);
    }
    if let Some(index) = scope::state_index(g, name) {
        return Some(g.states[index].ty);
    }
    g.constants
        .iter()
        .find(|constant| constant.name.as_bytes() == name)
        .map(|constant| constant.value.ty)
}

/// Only a named component function has a supported call result type.
pub(super) fn call_hint(g: &mut Generator<'_>, call: &ast::Call) -> Option<Type> {
    let ast::ExprData::Ident(name) = &call.callee.data else {
        return None;
    };
    let index = scope::source_method(g, name)?;
    Some(g.methods[index].ret)
}

/// Infer numeric peers only for supported arithmetic operators.
pub(super) fn binary_hint(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    b: &ast::Binary,
) -> Result<Option<Type>> {
    if !arithmetic(&b.op) {
        return Ok(None);
    }
    let Some(lhs) = expression_hint(g, c, &b.lhs)? else {
        return Ok(None);
    };
    let Some(rhs) = expression_hint(g, c, &b.rhs)? else {
        return Ok(None);
    };
    Ok(Type::numeric_peer(lhs, rhs))
}

/// Infer a conditional's numeric peer without evaluating its condition.
pub(super) fn branch_hint(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    value: &ast::Cond,
) -> Result<Option<Type>> {
    let Some(then) = expression_hint(g, c, &value.then)? else {
        return Ok(None);
    };
    let Some(otherwise) = expression_hint(g, c, &value.else_)? else {
        return Ok(None);
    };
    Ok(Type::numeric_peer(then, otherwise))
}

/// A numeric cast supplies the peer type even when its operand was untyped.
pub(super) fn cast_hint(g: &mut Generator<'_>, reference: &ast::TypeRef) -> Option<Type> {
    if reference.detail.is_some() {
        return None;
    }
    for alias in &g.type_aliases {
        if alias.name.as_bytes() == reference.name {
            return Some(alias.ty);
        }
    }
    if reference.name == b"number" {
        return Some(Type::F64);
    }
    numeric_name(&reference.name)
}

/// Inspect members and indexes without emitting expressions or registering
/// captures.
pub(super) fn value_hint(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
) -> Result<Option<Type>> {
    match &e.data {
        ast::ExprData::Ident(name) => Ok(name_hint(g, c, name)),
        ast::ExprData::NonNull(inner) => {
            Ok(value_hint(g, c, inner)?.map(|ty| model::optional_child(g, ty).unwrap_or(ty)))
        }
        ast::ExprData::Index(ix) => {
            Ok(value_hint(g, c, &ix.object)?.and_then(|ty| model::element(g, ty)))
        }
        ast::ExprData::Member(member) => member_hint(g, c, member),
        ast::ExprData::Call(call) => Ok(call_hint(g, call)),
        _ => Ok(None),
    }
}

pub(super) fn member_hint(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    member: &ast::Member,
) -> Result<Option<Type>> {
    let Some(ty) = value_hint(g, c, &member.object)? else {
        return Ok(None);
    };
    if member.name.name == b"length" && (ty == Type::String || model::element(g, ty).is_some()) {
        return Ok(Some(Type::Usize));
    }
    let Some(entry) = model::info(g, ty) else {
        return Ok(None);
    };
    if entry.kind == model::InfoKind::Object {
        for field in &entry.fields {
            if field.name == member.name.name {
                return Ok(Some(field.ty));
            }
        }
    }
    Ok(None)
}
