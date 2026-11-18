//! Attributes and props: optional / numeric / dimension values and how each prop kind is lowered into an options struct field.

use super::*;

/// Whether this prop's Rust option field is `Option<..>`: those values are
/// wrapped in `Some(..)`. This is the complement of the frozen design's
/// "prop → field" mapping (the generated table has no optionality flag);
/// `a_field_is_optional_exactly_when_the_design_says_so` keeps it honest for
/// the tags the golden tests use.
pub(super) fn optional_field(owner: &str, prop: &Prop) -> bool {
    if owner == "List" && prop.field == "item_height" {
        return true;
    }
    if owner.starts_with("ui.") {
        return match owner {
            "ui.MenuItem" => matches!(prop.field, "key" | "on_click" | "index_action"),
            "ui.SegmentedOption" => prop.field == "tooltip",
            _ => false,
        };
    }
    match prop.kind {
        Kind::Action
        | Kind::BoolAction
        | Kind::TextAction
        | Kind::IndexAction
        | Kind::ShortcutAction
        | Kind::Image
        | Kind::OptionalNode
        | Kind::Color => true,
        Kind::Enumeration => matches!(prop.field, "flex_direction" | "justify_content"),
        Kind::String => match prop.field {
            "key" => !matches!(
                owner,
                "HotkeyInput" | "Message" | "Modal" | "ShortcutList" | "List"
            ),
            "title" => owner == "Card",
            "extra" | "tooltip" | "value" => true,
            _ => false,
        },
        // `width` / `height` are `Dimension` props in the merged (alias)
        // table and plain numbers elsewhere; both set the same option fields.
        Kind::Number | Kind::Dimension => number_optional(owner, prop.field),
        _ => false,
    }
}

/// Optional `f32` option fields (numbers and dimensions): the tag table has
/// no optionality flag, so this mirrors the frozen option-struct declarations.
pub(super) fn number_optional(owner: &str, field: &str) -> bool {
    match field {
        "width" => !matches!(owner, "DropdownMenu" | "PdfPreview" | "Progress"),
        "height" => matches!(
            owner,
            "Card"
                | "PdfViewer"
                | "Clickable"
                | "Column"
                | "Image"
                | "RichText"
                | "Row"
                | "Space"
                | "View"
                | "List"
        ),
        "top" => owner != "Message",
        "font_size" => owner == "Button",
        "column_gap" => owner != "ShortcutList",
        "grow" => owner == "PdfViewer",
        "basis" | "bottom" | "content_max_width" | "height_percent" | "item_width" | "left"
        | "max_height" | "max_width" | "min_height" | "min_width" | "right" | "row_gap"
        | "stroke" | "width_percent" => true,
        _ => false,
    }
}

pub(super) fn wrap_optional(owner: &str, prop: &Prop, code: String) -> String {
    if optional_field(owner, prop) {
        format!("Some({code})")
    } else {
        code
    }
}

/// Put a field, grouping flattened prop families (`ClickableOptions.box`).
pub(super) fn put(fields: &mut Fields, prop: &Prop, code: String) {
    fields.group(prop.parent).put(prop.field, code);
}

pub(super) fn remove(fields: &mut Fields, prop: &Prop, field: &str) {
    fields.group(prop.parent).remove(field);
}

/// Merge props and literal style objects in source order; later writes replace
/// earlier fields.
pub fn attribute(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    tag: &Tag,
    fields: &mut Fields,
    attr: &ast::Attr,
) -> Result<()> {
    if attr.name.name == b"style" {
        let Some(e) = attr.value.as_deref() else {
            return fail(g, attr.name.loc, "style must be an object literal");
        };
        let ast::ExprData::Object(properties) = &e.data else {
            return fail(g, e.loc, "style must be an object literal");
        };
        for property in properties {
            if property.spread {
                return fail(g, property.value.loc, "object spread is not supported yet");
            }
            if property.key.name == b"style" {
                return fail(
                    g,
                    property.key.loc,
                    "nested style is not supported; put the properties in the outer object",
                );
            }
            let nested = ast::Attr {
                name: property.key.clone(),
                value: Some(property.value.clone()),
                spread: false,
            };
            attribute(g, c, tag, fields, &nested)?;
        }
        return Ok(());
    }
    let Some(prop) = tags::find_prop(tag, &String::from_utf8_lossy(&attr.name.name)) else {
        return fail(
            g,
            attr.name.loc,
            format!(
                "unknown prop `{}` on <{}>; valid props: {}",
                String::from_utf8_lossy(&attr.name.name),
                tag.name,
                valid_props(tag)
            ),
        );
    };
    property(
        g,
        c,
        tag.name,
        prop,
        fields,
        attr.value.as_deref(),
        attr.name.loc,
    )
}

/// Reuse scalar, node, and collection conversions for tag props and struct items.
pub fn property(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    owner: &str,
    prop: &Prop,
    fields: &mut Fields,
    value: Option<&ast::Expr>,
    loc: ast::Loc,
) -> Result<()> {
    let Some(e) = value else {
        if prop.kind != Kind::Boolean {
            return fail(
                g,
                loc,
                format!(
                    "bare attribute `{}` is only valid for boolean props",
                    prop.name
                ),
            );
        }
        put(fields, prop, "true".to_owned());
        return Ok(());
    };
    match prop.kind {
        Kind::Number => {
            let code = expr::emit(g, c, e, Some(Type::F32), false)?.code;
            put(fields, prop, wrap_optional(owner, prop, code));
        }
        Kind::Integer => integer_prop(g, c, owner, prop, e, fields)?,
        Kind::Boolean => {
            let code = expr::emit(g, c, e, Some(Type::Boolean), false)?.code;
            put(fields, prop, code);
        }
        Kind::String => {
            let code = string_prop(g, c, prop, e)?;
            put(fields, prop, wrap_optional(owner, prop, code));
        }
        Kind::Enumeration => {
            let code = enum_value(g, c, owner, prop, e)?;
            put(fields, prop, wrap_optional(owner, prop, code));
        }
        Kind::Color => {
            let code = rust_text::color(g, c, e)?;
            put(fields, prop, wrap_optional(owner, prop, code));
        }
        Kind::Dimension => dimension_prop(g, c, owner, prop, e, fields)?,
        Kind::Edges => jsx_values::edges(g, c, prop, e, fields)?,
        Kind::Node | Kind::OptionalNode => {
            let code = jsx_values::node(g, c, prop, e)?;
            put(fields, prop, code);
        }
        Kind::Children => {
            let code = jsx_values::children(g, c, prop, e)?;
            put(fields, prop, code);
        }
        Kind::StringList => {
            let code = jsx_values::strings(g, c, prop, e)?;
            put(fields, prop, code);
        }
        Kind::StructList => {
            let code = jsx_values::structs(g, c, prop, e)?;
            put(fields, prop, code);
        }
        Kind::Struct => {
            let code = jsx_values::object(g, c, prop, e)?;
            put(fields, prop, code);
        }
        Kind::Flag => {
            let field = enum_value(g, c, owner, prop, e)?;
            if !field.is_empty() {
                fields.put(&field, "true".to_owned());
            }
        }
        Kind::Flex => {
            let code = expr::emit(g, c, e, Some(Type::F32), false)?.code;
            put(fields, &prop_named("grow"), code);
            put(fields, &prop_named("shrink"), "1.0f32".to_owned());
            put(fields, &prop_named("basis"), "Some(0.0f32)".to_owned());
        }
        Kind::Action | Kind::BoolAction | Kind::TextAction | Kind::IndexAction => {
            let code = events::optional_event_value(g, c, e, prop.kind)?;
            put(fields, prop, code);
        }
        Kind::ShortcutAction => {
            let code = events::event_value(g, c, e, Kind::TextAction)?;
            put(
                fields,
                prop,
                format!("Some(tsx::desktop::shortcut_action({code}))"),
            );
        }
        Kind::Shortcut => {
            let value = expr::emit(g, c, e, None, false)?;
            let code = if value.ty == Type::Null {
                "None".to_owned()
            } else {
                let text = types::coerce(g, value, Type::String, e.loc)?;
                format!("tsx::desktop::parse_shortcut(({}).as_str())", text.code)
            };
            put(fields, prop, code);
        }
        Kind::Image => {
            let image_type = image_type(g)?;
            let image = model::optional(g, image_type)?;
            let code = expr::emit(g, c, e, Some(image), false)?.code;
            put(fields, prop, code);
        }
    }
    Ok(())
}

/// A synthetic prop for the `flex` shorthand fields.
pub(super) fn prop_named(field: &'static str) -> Prop {
    Prop {
        name: "",
        field,
        kind: Kind::Number,
        ..Prop::default()
    }
}

/// `Image` (`zant::image::Image`): an opaque in-memory image reference.
pub fn image_type(g: &mut Generator<'_>) -> Result<Type> {
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.rust == "std::rc::Rc<zant::image::Image>" {
            return Ok(Type::Compound(index as u32));
        }
    }
    model::add(
        g,
        model::Info {
            kind: model::InfoKind::Object,
            rust: "std::rc::Rc<zant::image::Image>".to_owned(),
            ..model::Info::default()
        },
    )
}

/// Require an integer expression and coerce it to the field integer type.
pub(super) fn integer_prop(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    owner: &str,
    prop: &Prop,
    e: &ast::Expr,
    fields: &mut Fields,
) -> Result<()> {
    let Some(ty) = types::numeric_name(prop.int_type.as_bytes()) else {
        return fail(
            g,
            e.loc,
            format!("`{}` has an unsupported integer type", prop.name),
        );
    };
    types::check_literal(g, e, ty)?;
    let expected = if types::numeric_literals(e) && !matches!(e.data, ast::ExprData::Number(_)) {
        Some(ty)
    } else {
        None
    };
    let value = expr::emit(g, c, e, expected, false)?;
    if value.ty == Type::F64 || value.ty == Type::F32 {
        // TS numbers: truncated and clamped to the field's range
        let code = format!("tsx::to_int::<{}>(({}) as f64)", ty.rust(g), value.code);
        put(fields, prop, wrap_optional(owner, prop, code));
        return Ok(());
    }
    if !value.ty.integer() && value.ty != Type::IntLiteral {
        return fail(
            g,
            e.loc,
            format!("`{}` requires an integer expression", prop.name),
        );
    }
    let code = types::coerce(g, value, ty, e.loc)?.code;
    put(fields, prop, wrap_optional(owner, prop, code));
    Ok(())
}

/// Keep the last fixed or percent dimension and remove its opposite field.
pub(super) fn dimension_prop(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    owner: &str,
    prop: &Prop,
    e: &ast::Expr,
    fields: &mut Fields,
) -> Result<()> {
    if let ast::ExprData::String(s) = &e.data {
        if s.len() < 2 || s.last() != Some(&b'%') || prop.percent_field.is_empty() {
            return fail(
                g,
                e.loc,
                "dimension must be a number or a percentage literal such as \"50%\"",
            );
        }
        let text = String::from_utf8_lossy(&s[..s.len() - 1]);
        let Ok(n) = text.parse::<f32>() else {
            return fail(g, e.loc, "invalid percentage dimension");
        };
        if !n.is_finite() {
            return fail(g, e.loc, "percentage dimension must be finite");
        }
        remove(fields, prop, prop.field);
        put(
            fields,
            &Prop {
                field: prop.percent_field,
                kind: Kind::Number,
                ..prop.clone()
            },
            format!("Some({})", float_literal(n)),
        );
    } else {
        remove(fields, prop, prop.percent_field);
        let code = expr::emit(g, c, e, Some(Type::F32), false)?.code;
        put(fields, prop, wrap_optional(owner, prop, code));
    }
    Ok(())
}

/// A float literal with a decimal point (`50.0`), for `f32` option fields.
pub(super) fn float_literal(n: f32) -> String {
    let mut text = format!("{n}");
    if !text.contains(['.', 'e', 'E']) {
        text.push_str(".0");
    }
    text
}

/// Put a field chosen by name, grouped under its flattened prop family.
pub(super) fn put_named(fields: &mut Fields, tag: &Tag, field: &str, code: String) {
    match tags::find_prop(tag, field) {
        Some(prop) => put(fields, prop, code),
        None => fields.put(field, code),
    }
}

/// Numeric keys use the same formatting rules as text substitutions.
pub(super) fn string_prop(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    prop: &Prop,
    e: &ast::Expr,
) -> Result<String> {
    if prop.name != "key" {
        return Ok(expr::emit(g, c, e, Some(Type::String), false)?.code);
    }
    let value = expr::emit(g, c, e, None, false)?;
    if value.ty == Type::String {
        return Ok(value.code);
    }
    if model::enum_type(g, value.ty) {
        return Ok(types::coerce(g, value, Type::String, e.loc)?.code);
    }
    if model::optional_child(g, value.ty).is_some() {
        return fail(g, e.loc, "optional key needs ?? or !");
    }
    if !value.ty.numeric() {
        return fail(g, e.loc, "key must be a string or number");
    }
    let mut format = Format::default();
    format.value(g, value, e.loc)?;
    format.finish(g, false)
}
