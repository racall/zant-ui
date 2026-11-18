//! Enumeration props: string literals and union-typed values lowered to the Rust enums of the tag table.

use super::*;

/// A value of a string-literal union (`look.variant`: `"primary" | "default"`)
/// for an enum prop: every member must be one of the prop's values; mapped
/// with a `match`. `None` when `e` is not such a value.
pub(super) fn enum_variable(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    owner: &str,
    prop: &Prop,
    e: &ast::Expr,
) -> Result<Option<String>> {
    let value = expr::emit(g, c, e, None, false)?;
    let Some(info) = model::info(g, value.ty) else {
        return Ok(None);
    };
    if info.kind != model::InfoKind::Enumeration {
        return Ok(None);
    }
    let enum_name = value.ty.rust(g).to_owned();
    let Some(path) = enum_type_path(owner, prop.field) else {
        return fail(
            g,
            e.loc,
            format!("`{}` is an enumeration without a Rust enum type", prop.name),
        );
    };
    let mut arms = String::new();
    for member in &info.members {
        let Some(target) = tags::find_value(prop, &String::from_utf8_lossy(member)) else {
            return fail(
                g,
                e.loc,
                format!(
                    "`{}` can be \"{}\", which is not a `{}` value",
                    prop.name,
                    String::from_utf8_lossy(member),
                    prop.name
                ),
            );
        };
        let Some(from) = rust_text::enum_variant(member) else {
            return fail(g, e.loc, "enum member is not valid UTF-8");
        };
        let Some(to) = rust_text::enum_variant(target.as_bytes()) else {
            return fail(g, e.loc, "enum member is not valid UTF-8");
        };
        arms.push_str(&format!("{enum_name}::{from} => {path}::{to}, "));
    }
    Ok(Some(format!("match {} {{ {arms}}}", value.code)))
}

/// A literal / conditional / union value of an enum prop.
pub fn enum_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    owner: &str,
    prop: &Prop,
    e: &ast::Expr,
) -> Result<String> {
    if prop.kind != Kind::Flag
        && let ast::ExprData::Cond(value) = &e.data
    {
        let cond = expr::condition(g, c, &value.cond)?;
        let mut mark = narrowing::push(g, c, &value.cond, true)?;
        let then = enum_value(g, c, owner, prop, &value.then);
        narrowing::restore(c, mark);
        let then = then?;
        mark = narrowing::push(g, c, &value.cond, false)?;
        let otherwise = enum_value(g, c, owner, prop, &value.else_);
        narrowing::restore(c, mark);
        return Ok(format!(
            "if {} {{ {then} }} else {{ {} }}",
            cond.code, otherwise?
        ));
    }
    let spelling: Vec<u8> = match &e.data {
        ast::ExprData::String(s) => s.clone(),
        ast::ExprData::Number(s) => {
            if prop.kind == Kind::Enumeration {
                s.clone()
            } else {
                return fail(
                    g,
                    e.loc,
                    format!("`{}` requires a static string literal", prop.name),
                );
            }
        }
        _ => {
            if prop.kind != Kind::Flag
                && let Some(code) = enum_variable(g, c, owner, prop, e)?
            {
                return Ok(code);
            }
            return fail(
                g,
                e.loc,
                format!(
                    "`{}` requires a literal{}",
                    prop.name,
                    if prop.kind == Kind::Flag {
                        " string"
                    } else {
                        ", a conditional of literals or a string-literal union value"
                    }
                ),
            );
        }
    };
    let Some(value) = tags::find_value(prop, &String::from_utf8_lossy(&spelling)) else {
        let names: Vec<&str> = prop.values.iter().map(|value| value.ts).collect();
        return fail(
            g,
            e.loc,
            format!(
                "invalid `{}` value `{}`; expected: {}",
                prop.name,
                String::from_utf8_lossy(&spelling),
                names.join(", ")
            ),
        );
    };
    if prop.kind == Kind::Flag {
        return Ok(value.to_owned());
    }
    let Some(path) = enum_type_path(owner, prop.field) else {
        return fail(
            g,
            e.loc,
            format!("`{}` is an enumeration without a Rust enum type", prop.name),
        );
    };
    let Some(variant) = rust_text::enum_variant(value.as_bytes()) else {
        return fail(g, e.loc, "enum member is not valid UTF-8");
    };
    Ok(format!("{path}::{variant}"))
}

/// The Rust enum type of an enumeration prop. The generated tag table stores
/// value spellings, not their Rust type; this is the one place that names
/// the enum.
pub(super) fn enum_type_path(owner: &str, field: &str) -> Option<&'static str> {
    if owner.starts_with("ui.") && field == "kind" {
        return Some("ui::MenuItemKind");
    }
    match field {
        "wrap" => Some("zant::flex::Wrap"),
        "align_content" => Some("zant::flex::AlignContent"),
        "align_self" => Some("zant::flex::AlignSelf"),
        "justify" | "justify_content" => Some("zant::flex::Justify"),
        "align_items" => Some("zant::flex::Align"),
        "flex_direction" | "direction" => Some("zant::flex::Direction"),
        "weight" => Some("zant::renderer::text::Weight"),
        "text_align" => Some("zant::renderer::text::Align"),
        "family" => Some("zant::renderer::text::Family"),
        "variant" => Some("zant::core::node::ButtonType"),
        "size" => Some("zant::core::node::ButtonSize"),
        "icon" | "name" => Some("ui::Icon"),
        "fit" => Some("zant::core::node::ImageFit"),
        "cursor" => Some("zant::platform::platform::Cursor"),
        "type" => Some("zant::ui::AlertType"),
        "motion" => Some("zant::motion::motion::Presence"),
        "motion_origin" => Some("zant::motion::motion::Origin"),
        "kind" => Some("zant::ui::MessageKind"),
        "caret" => Some("zant::markdown::render::options::Caret"),
        "alignment" => Some("ui::ListAlignment"),
        _ => None,
    }
}
