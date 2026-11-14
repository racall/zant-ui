//! JSX spread attributes: `<Button {...common} onClick={save} />`. Before a
//! tag or component reads its attributes, every spread becomes plain
//! attributes in source order, so a later attribute wins as in React:
//!
//! - an object literal `{...{ size: "small" }}` spreads its properties;
//! - a typed object (`common: ButtonStyle`) spreads `common.field` for each
//!   field of its type; it must be a name or a member path (it is read once
//!   per field), and optional fields can't be spread (the prop would be
//!   "maybe absent").

use crate::ast;

use super::expr;
use super::model;
use super::scope;
use super::types::Type;
use super::{Generator, Result, fail};

/// Whether `attrs` has a spread.
pub fn any(attrs: &[ast::Attr]) -> bool {
    attrs.iter().any(|attr| attr.spread)
}

/// Object-literal spreads expanded, typed ones left in place (handler
/// scanning: arrows inside literals register like attributes).
pub fn literals(g: &mut Generator<'_>, attrs: &[ast::Attr]) -> Result<Vec<ast::Attr>> {
    if !any(attrs) {
        return Ok(attrs.to_vec());
    }
    let mut out: Vec<ast::Attr> = Vec::new();
    for attr in attrs {
        if attr.spread
            && let Some(value) = attr.value.as_deref()
            && matches!(value.data, ast::ExprData::Object(_))
        {
            object_literal(g, value, &mut out, false, None)?;
        } else {
            out.push(attr.clone());
        }
    }
    Ok(out)
}

/// Every spread expanded; a later attribute replaces an earlier one.
pub fn flatten(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    attrs: &[ast::Attr],
) -> Result<Vec<ast::Attr>> {
    if !any(attrs) {
        return Ok(attrs.to_vec());
    }
    let mut out: Vec<ast::Attr> = Vec::new();
    for attr in attrs {
        if !attr.spread {
            put(g, &mut out, attr.clone());
            continue;
        }
        let Some(value) = attr.value.as_deref() else {
            continue;
        };
        if matches!(value.data, ast::ExprData::Object(_)) {
            object_literal(g, value, &mut out, true, Some(c))?;
            continue;
        }
        typed_object(g, c, value, &mut out)?;
    }
    Ok(out)
}

fn put(g: &Generator<'_>, out: &mut Vec<ast::Attr>, attr: ast::Attr) {
    if let Some(index) = out
        .iter()
        .position(|existing| existing.name.name == attr.name.name)
    {
        out.remove(index);
    }
    let _ = g;
    out.push(attr);
}

fn object_literal(
    g: &mut Generator<'_>,
    e: &ast::Expr,
    out: &mut Vec<ast::Attr>,
    replace: bool,
    c: Option<&mut scope::Context>,
) -> Result<()> {
    let ast::ExprData::Object(properties) = &e.data else {
        return Ok(());
    };
    let mut context = c;
    for property in properties {
        // borrow checker: reborrow the optional context per property
        let context_ref = context.as_deref_mut();
        if property.spread {
            let value = Some(&*property.value);
            if let Some(value) = value {
                if matches!(value.data, ast::ExprData::Object(_)) {
                    object_literal(g, value, out, replace, context_ref)?;
                } else if let Some(context) = context_ref {
                    typed_object(g, context, value, out)?;
                }
            }
            continue;
        }
        let attr = ast::Attr {
            name: property.key.clone(),
            value: Some(property.value.clone()),
            spread: false,
        };
        if replace {
            put(g, out, attr);
        } else {
            out.push(attr);
        }
    }
    Ok(())
}

fn typed_object(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    out: &mut Vec<ast::Attr>,
) -> Result<()> {
    if !path(e) {
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
    let _ = Type::Void;
    for field in info.fields {
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
        put(
            g,
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
    let _ = info.rust;
    Ok(())
}

/// `a`, `a.b.c`: cheap to read once per field.
fn path(e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::Ident(_) => true,
        ast::ExprData::Member(m) => !m.optional && path(&m.object),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_paths_are_spreadable() {
        let mut diag = None;
        let mut g = Generator::new(&mut diag);
        let expr = |data: ast::ExprData| ast::Expr { loc: 0, data };
        assert!(path(&expr(ast::ExprData::Ident(b"common".to_vec()))));
        assert!(path(&expr(ast::ExprData::Member(ast::Member {
            object: Box::new(expr(ast::ExprData::Ident(b"props".to_vec()))),
            name: ast::Name {
                name: b"common".to_vec(),
                loc: 0,
            },
            optional: false,
        }))));
        assert!(!path(&expr(ast::ExprData::Number(b"1".to_vec()))));
        assert!(!any(&[]));
        let _ = &mut g;
    }
}
