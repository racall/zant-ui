//! Element children: node lists, single children, text content, button icons and text expressions.

use super::*;

/// Build a children array and filter null entries only when necessary.
pub(super) fn node_children(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    jsx: &ast::Jsx,
    tag: &Tag,
    fields: &mut Fields,
    child_list: &mut ChildList,
) -> Result<Option<String>> {
    child_list.append(g, c, &jsx.children, None)?;
    if child_list.values.is_empty() {
        return Ok(None);
    }
    let Some(prop) = tags::find_prop(tag, "children") else {
        return fail(
            g,
            jsx.loc,
            format!("tag table is missing <{}> children metadata", tag.name),
        );
    };
    let code = jsx_values::children_code(g, child_list);
    fields.group(prop.parent).put(prop.field, code);
    Ok(None)
}

/// A single-node content field accepts either a prop or exactly one JSX child.
pub(super) fn single_child(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    jsx: &ast::Jsx,
    tag: &Tag,
    fields: &mut Fields,
    child_list: &mut ChildList,
) -> Result<()> {
    let Some(prop) = tags::find_prop(tag, tag.content_field) else {
        return fail(
            g,
            jsx.loc,
            format!("tag table is missing <{}> child metadata", tag.name),
        );
    };
    let has_children = jsx
        .children
        .iter()
        .any(|child| !matches!(child, ast::Child::Expr(None)));
    if fields.has_prop(prop) {
        if has_children {
            return fail(
                g,
                jsx.loc,
                format!(
                    "<{}> cannot have both `{}` and child elements",
                    tag.name, tag.content_field
                ),
            );
        }
        return Ok(());
    }
    child_list.append(g, c, &jsx.children, None)?;
    if child_list.values.len() != 1 {
        return fail(
            g,
            jsx.loc,
            format!(
                "<{}> requires exactly one child element or the `{}` prop",
                tag.name, tag.content_field
            ),
        );
    }
    let child = child_list.values[0].clone();
    if child.ty != Type::Node {
        return fail(
            g,
            jsx.loc,
            format!("<{}> requires a non-optional node child", tag.name),
        );
    }
    fields.group(prop.parent).put(prop.field, child.code);
    Ok(())
}

/// Join scalar text children and reject a competing text prop.
pub(super) fn text_content(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    jsx: &ast::Jsx,
    tag: &Tag,
    fields: &mut Fields,
) -> Result<Option<String>> {
    let mut format = Format::default();
    let mut content = false;
    for (index, child) in jsx.children.iter().enumerate() {
        match child {
            ast::Child::Text(text) => {
                // the button spaces its icon from the label itself
                let mut value = text.value.clone();
                if index > 0 && is_icon(&jsx.children[index - 1]) {
                    value = trim_start_spaces(&value);
                }
                if index + 1 < jsx.children.len() && is_icon(&jsx.children[index + 1]) {
                    value = trim_end_spaces(&value);
                }
                format.text(&value);
                content = true;
            }
            ast::Child::Expr(Some(e)) => {
                let expression = text_expression(g, e)?;
                format.expression(g, c, &expression)?;
                content = true;
            }
            ast::Child::Expr(None) => {}
            ast::Child::Element(element) => {
                // `<Button><Icon name="check" /> Save</Button>`
                if tag.name != "Button" || element.name != b"Icon" {
                    return misplaced(g, tag, element);
                }
                button_icon(g, c, element, fields)?;
            }
        }
    }
    if tag.content == Content::TextArg {
        return Ok(Some(format.finish(g, false)?));
    }
    if content {
        if fields.has(tag.content_field) {
            return fail(
                g,
                jsx.loc,
                format!(
                    "<{}> cannot have both `{}` and text children",
                    tag.name, tag.content_field
                ),
            );
        }
        let code = format.finish(g, false)?;
        put_named(fields, tag, tag.content_field, code);
    }
    Ok(None)
}

pub(super) fn trim_start_spaces(value: &[u8]) -> Vec<u8> {
    let start = value
        .iter()
        .position(|byte| *byte != b' ')
        .unwrap_or(value.len());
    value[start..].to_vec()
}

pub(super) fn trim_end_spaces(value: &[u8]) -> Vec<u8> {
    let end = value
        .iter()
        .rposition(|byte| *byte != b' ')
        .map_or(0, |position| position + 1);
    value[..end].to_vec()
}

pub(super) fn is_icon(child: &ast::Child) -> bool {
    matches!(child, ast::Child::Element(element) if element.name == b"Icon")
}

/// One `<Icon>` inside a `<Button>` sets its `icon` (and `icon_size`).
pub(super) fn button_icon(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    element: &ast::Jsx,
    fields: &mut Fields,
) -> Result<()> {
    if fields.has("icon") {
        return fail(g, element.loc, "a <Button> has one icon");
    }
    let button = tags::find("Button").expect("Button is a tag");
    for attr in &element.attrs {
        let target = if attr.name.name == b"name" {
            "icon"
        } else if attr.name.name == b"size" {
            "iconSize"
        } else {
            return fail(
                g,
                attr.name.loc,
                "an icon in a <Button> takes `name` and `size`",
            );
        };
        let renamed = ast::Attr {
            name: ast::Name {
                name: target.as_bytes().to_vec(),
                loc: attr.name.loc,
            },
            value: attr.value.clone(),
            spread: false,
        };
        attribute(g, c, button, fields, &renamed)?;
    }
    if !fields.has("icon") {
        return fail(g, element.loc, "<Icon> needs `name`");
    }
    Ok(())
}

/// Reject all nonempty children on tags whose content mode is none.
pub(super) fn no_children(g: &mut Generator<'_>, jsx: &ast::Jsx, tag: &Tag) -> Result<()> {
    for child in &jsx.children {
        match child {
            ast::Child::Expr(Some(e)) => {
                return fail(g, e.loc, format!("<{}> has no children", tag.name));
            }
            ast::Child::Expr(None) => {}
            ast::Child::Text(text) => {
                return fail(g, text.loc, format!("<{}> has no children", tag.name));
            }
            ast::Child::Element(element) => return misplaced(g, tag, element),
        }
    }
    Ok(())
}

/// `cond && "text"` in text content: renders "" when false (React renders
/// nothing for `false`). Other expressions are returned unchanged.
pub fn text_expression(g: &mut Generator<'_>, e: &ast::Expr) -> Result<ast::Expr> {
    if let ast::ExprData::Binary(binary) = &e.data
        && binary.op == b"&&"
    {
        let empty = ast::Expr {
            loc: e.loc,
            data: ast::ExprData::String(Vec::new()),
        };
        return Ok(ast::Expr {
            loc: e.loc,
            data: ast::ExprData::Cond(ast::Cond {
                cond: binary.lhs.clone(),
                then: binary.rhs.clone(),
                else_: Box::new(empty),
            }),
        });
    }
    let _ = g;
    Ok(e.clone())
}
