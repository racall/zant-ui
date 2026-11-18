//! Spread attributes (`{...props}`): flattening object literals and typed objects into attributes.

use super::*;

/// Whether `attrs` has a spread.
pub(crate) fn spread_any(attrs: &[ast::Attr]) -> bool {
    attrs.iter().any(|attr| attr.spread)
}

/// Attributes in source order. Without spreads the original AST nodes are
/// reused, so handler expressions keep the addresses the scanner registered;
/// spread attributes are cloned by the expansion.
pub(super) fn attribute_order<'a>(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    attrs: &'a [ast::Attr],
) -> Result<std::borrow::Cow<'a, [ast::Attr]>> {
    if spread_any(attrs) {
        return Ok(std::borrow::Cow::Owned(spread_flatten(g, c, attrs)?));
    }
    Ok(std::borrow::Cow::Borrowed(attrs))
}

/// Object-literal spreads expanded, typed ones left in place (handler
/// scanning: arrows inside literals register like attributes).
pub fn spread_literals(_g: &mut Generator<'_>, attrs: &[ast::Attr]) -> Result<Vec<ast::Attr>> {
    if !spread_any(attrs) {
        return Ok(attrs.to_vec());
    }
    let mut out: Vec<ast::Attr> = Vec::new();
    for attr in attrs {
        if attr.spread
            && let Some(value) = attr.value.as_deref()
            && let ast::ExprData::Object(_) = &value.data
        {
            object_literal_literals(value, &mut out);
        } else {
            out.push(attr.clone());
        }
    }
    Ok(out)
}

/// Every spread expanded; a later attribute replaces an earlier one.
pub(super) fn spread_flatten(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    attrs: &[ast::Attr],
) -> Result<Vec<ast::Attr>> {
    if !spread_any(attrs) {
        return Ok(attrs.to_vec());
    }
    let mut out: Vec<ast::Attr> = Vec::new();
    for attr in attrs {
        if !attr.spread {
            spread_put(&mut out, attr.clone());
            continue;
        }
        let Some(value) = attr.value.as_deref() else {
            continue;
        };
        if let ast::ExprData::Object(_) = &value.data {
            object_literal_flatten(g, c, value, &mut out)?;
        } else {
            typed_object(g, c, value, &mut out)?;
        }
    }
    Ok(out)
}

pub(super) fn spread_put(out: &mut Vec<ast::Attr>, attr: ast::Attr) {
    if let Some(index) = out
        .iter()
        .position(|existing| existing.name.name == attr.name.name)
    {
        out.remove(index);
    }
    out.push(attr);
}

pub(super) fn object_literal_literals(e: &ast::Expr, out: &mut Vec<ast::Attr>) {
    let ast::ExprData::Object(properties) = &e.data else {
        return;
    };
    for property in properties {
        if property.spread {
            if let ast::ExprData::Object(_) = &property.value.data {
                object_literal_literals(&property.value, out);
            }
            continue;
        }
        out.push(ast::Attr {
            name: property.key.clone(),
            value: Some(property.value.clone()),
            spread: false,
        });
    }
}

pub(super) fn object_literal_flatten(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    out: &mut Vec<ast::Attr>,
) -> Result<()> {
    let ast::ExprData::Object(properties) = &e.data else {
        return Ok(());
    };
    for property in properties {
        if property.spread {
            if let ast::ExprData::Object(_) = &property.value.data {
                object_literal_flatten(g, c, &property.value, out)?;
            } else {
                typed_object(g, c, &property.value, out)?;
            }
            continue;
        }
        spread_put(
            out,
            ast::Attr {
                name: property.key.clone(),
                value: Some(property.value.clone()),
                spread: false,
            },
        );
    }
    Ok(())
}

pub(super) fn typed_object(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    out: &mut Vec<ast::Attr>,
) -> Result<()> {
    if !is_path(e) {
        return fail(
            g,
            e.loc,
            "spread a name or a member path (`{...style}`, `{...props.common}`); store other values in a const first",
        );
    }
    let value = expr::emit(g, c, e, None, false)?;
    let Some(info) = model::info(g, value.ty) else {
        return fail(g, e.loc, "only objects can be spread");
    };
    if info.kind != model::InfoKind::Object {
        return fail(g, e.loc, "only objects can be spread");
    }
    for field in &info.fields {
        if field.optional {
            return fail(
                g,
                e.loc,
                format!(
                    "optional field `{}` can't be spread (the prop might be absent); pass it explicitly",
                    String::from_utf8_lossy(&field.name)
                ),
            );
        }
        let member = ast::Expr {
            loc: e.loc,
            data: ast::ExprData::Member(ast::Member {
                object: Box::new(e.clone()),
                name: ast::Name {
                    name: field.name.clone(),
                    loc: e.loc,
                },
                optional: false,
            }),
        };
        spread_put(
            out,
            ast::Attr {
                name: ast::Name {
                    name: field.name.clone(),
                    loc: e.loc,
                },
                value: Some(Box::new(member)),
                spread: false,
            },
        );
    }
    Ok(())
}

/// `a`, `a.b.c`: cheap to read once per field.
pub(super) fn is_path(e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::Ident(_) => true,
        ast::ExprData::Member(member) => !member.optional && is_path(&member.object),
        _ => false,
    }
}

// ── nesting rules ────────────────────────────────────────────────────
