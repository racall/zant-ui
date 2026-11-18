//! Stateful widgets (`<Markdown>`, `<PdfViewer>`, …): builder and options paths, instance keys and Markdown children.

use super::*;

/// `ui.richText` → `ui::rich_text`, `ui.@"switch"` → `ui::switch`.
pub(super) fn builder_path(tag: &Tag) -> Result<String> {
    let Some(rest) = tag.builder.strip_prefix("ui.") else {
        return Err(crate::codegen::Error::Tsx);
    };
    if let Some(inner) = rest.strip_prefix("@\"") {
        let name = inner.trim_end_matches('"');
        return Ok(format!("ui::{name}"));
    }
    Ok(format!("ui::{}", snake_case(rest)))
}

/// The Rust options struct of a tag builder.
pub(super) fn options_path(tag: &Tag) -> Result<String> {
    let name = match builder_path(tag)?.strip_prefix("ui::") {
        Some(name) => name.to_owned(),
        None => return Err(crate::codegen::Error::Tsx),
    };
    let options = match name.as_str() {
        "column" | "row" => "BoxOptions".to_owned(),
        "checkbox" | "switch" => "ToggleOptions".to_owned(),
        "title" => "HeadingOptions".to_owned(),
        "pdf_preview" => "PdfOptions".to_owned(),
        "pdf_viewer" => "PdfViewerOptions".to_owned(),
        "dropdown_menu" => "MenuOptions".to_owned(),
        _ => format!("{}Options", pascal_case(&name)),
    };
    Ok(format!("ui::{options}"))
}

/// camelCase → snake_case (`richText` → `rich_text`).
pub(crate) fn snake_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            out.push('_');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// snake_case → PascalCase (`rich_text` → `RichText`).
pub(super) fn pascal_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for part in name.split('_') {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

/// Stateful widget tags (`Markdown`, `PdfViewer`): an instance id then options.
pub(super) fn widget_emit(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    jsx: &ast::Jsx,
    tag: &Tag,
) -> Result<Value> {
    let mut fields = Fields::default();
    let mut raw_key: Option<String> = None;
    for attr in attribute_order(g, c, &jsx.attrs)?.iter() {
        attribute(g, c, tag, &mut fields, attr)?;
        if attr.name.name == b"key" {
            raw_key = Some(key_value(g, c, attr.value.as_deref(), attr.name.loc)?);
        }
        if attr.name.name == b"style"
            && let Some(value) = attr.value.as_deref()
            && let ast::ExprData::Object(properties) = &value.data
        {
            for property in properties {
                if property.key.name == b"key" {
                    raw_key = Some(key_value(
                        g,
                        c,
                        Some(property.value.as_ref()),
                        property.key.loc,
                    )?);
                }
            }
        }
    }
    if jsx.name == b"Markdown" {
        markdown_children(g, c, jsx, &mut fields)?;
    } else {
        for child in &jsx.children {
            match child {
                ast::Child::Expr(Some(e)) => {
                    return fail(g, e.loc, format!("<{}> has no children", tag.name));
                }
                ast::Child::Expr(None) => {}
                ast::Child::Text(text) => {
                    return fail(g, text.loc, format!("<{}> has no children", tag.name));
                }
                ast::Child::Element(element) => {
                    return fail(g, element.loc, format!("<{}> has no children", tag.name));
                }
            }
        }
    }
    let owner = format!("<{}>", tag.name);
    jsx_values::required(g, &tag.props, &fields, jsx.loc, &owner, "prop")?;
    if raw_key.is_none() && g.loop_depth > 0 {
        return fail(
            g,
            jsx.loc,
            format!(
                "<{}> keeps its own state; give each one in a list a key (key={{item.id}}) so the state follows the item",
                tag.name
            ),
        );
    }
    let parent = if !g.child_component {
        "0"
    } else if g.handler_state.is_some() {
        "_parent_id"
    } else {
        "self._id"
    };
    let id = match raw_key {
        Some(key) => format!("tsx::child_id({parent}, \"{}\", {key})", tag.name),
        None => {
            g.child_sites += 1;
            format!(
                "tsx::site_id({parent}, \"{}\", {})",
                tag.name, g.child_sites
            )
        }
    };
    let node = format!(
        "{}({id}, {})",
        builder_path(tag)?,
        fields.emit(&options_path(tag)?)
    );
    Ok(Value::new(Type::Node, node))
}

pub(super) fn key_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: Option<&ast::Expr>,
    loc: ast::Loc,
) -> Result<String> {
    let Some(e) = e else {
        return fail(g, loc, "key needs a value");
    };
    let value = expr::emit(g, c, e, None, false)?;
    if value.ty == Type::String {
        return Ok(format!("({}).as_str()", value.code));
    }
    if value.ty.numeric() {
        let value = types::concrete(g, value)?;
        return Ok(format!(
            "zant::Str::copy(&format!(\"{{}}\", {})).as_str()",
            value.code
        ));
    }
    fail(g, e.loc, "key must be a string or a number")
}

/// `<Markdown>` takes its text as `source` or as its children.
pub(super) fn markdown_children(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    jsx: &ast::Jsx,
    fields: &mut Fields,
) -> Result<()> {
    let message = "Markdown takes its text as `source` or as its children";
    let mut format = Format::default();
    let mut has_text = false;
    for child in &jsx.children {
        match child {
            ast::Child::Text(text) => {
                format.text(&text.value);
                has_text = true;
            }
            ast::Child::Expr(Some(e)) => {
                let expression = text_expression(g, e)?;
                let value = expr::emit(g, c, &expression, None, false)?;
                if value.ty != Type::String {
                    return fail(g, e.loc, message);
                }
                format.value(g, value, e.loc)?;
                has_text = true;
            }
            ast::Child::Expr(None) => {}
            ast::Child::Element(element) => return fail(g, element.loc, message),
        }
    }
    if has_text {
        if fields.has("source") {
            return fail(
                g,
                jsx.loc,
                "<Markdown> cannot have both `source` and text children",
            );
        }
        let code = format.finish(g, false)?;
        put_named(
            fields,
            tags::find("Markdown").expect("Markdown tag"),
            "source",
            code,
        );
    }
    Ok(())
}

// ── spread attributes ────────────────────────────────────────────────
