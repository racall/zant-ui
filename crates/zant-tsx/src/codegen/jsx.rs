//! JSX builders, ordered prop merging, text content, fragments, and optional
//! children, together with spread attributes, stateful widgets and the
//! nesting rules. Option fields and edges are in [`super::jsx_values`].
//!
//! Shape of the generated code:
//!
//! - options are concrete structs: `ui::button(ui::ButtonOptions { …,
//!   ..Default::default() })`;
//! - builders use Rust paths (`ui::rich_text`, `ui::switch`), derived from the
//!   spellings of the generated tag table (`ui.richText`);
//! - optional option fields are wrapped in `Some(..)` (the tag table carries
//!   prop kinds, not field optionality — see [`optional_field`]);
//! - children are the `nodes!` macro;
//! - enum props name the Rust enum (`ui::ButtonType::Primary`).
//!
//! Component tags are dispatched by [`super::components`] before the `ui::*`
//! lowering (see [`emit`]); the rest of this module only knows `ui::*` tags,
//! `<List>` and stateful widgets.

use crate::ast;
use crate::tags::{self, Content, Kind, Prop, Tag};

use super::events;
use super::expr;
use super::jsx_children::ChildList;
use super::jsx_values::{self, Fields};
use super::list;
use super::model;
use super::narrowing;
use super::rich_text;
use super::rust_text::{self, Format};
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

/// The comma separated tag list of an unknown-tag diagnostic.
pub fn valid_tags() -> String {
    tags::all()
        .iter()
        .map(|tag| tag.name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The comma separated prop list of an unknown-prop diagnostic.
pub fn valid_props(tag: &Tag) -> String {
    tag.props
        .iter()
        .map(|prop| prop.name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Whether `e` contains a JSX fragment (`<>…</>`), possibly behind
/// conditionals and boolean operators.
pub fn has_fragment(e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::Jsx(element) => element.name.is_empty(),
        ast::ExprData::NonNull(inner) => has_fragment(inner),
        ast::ExprData::Cond(value) => has_fragment(&value.then) || has_fragment(&value.else_),
        ast::ExprData::Binary(binary) => has_fragment(&binary.lhs) || has_fragment(&binary.rhs),
        _ => false,
    }
}

/// A value of a string-literal union (`look.variant`: `"primary" | "default"`)
/// for an enum prop: every member must be one of the prop's values; mapped
/// with a `match`. `None` when `e` is not such a value.
fn enum_variable(
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
fn enum_type_path(owner: &str, field: &str) -> Option<&'static str> {
    if owner.starts_with("ui.") && field == "kind" {
        return Some("ui::MenuItemKind");
    }
    match field {
        "wrap" => Some("zant::layout::flex::Wrap"),
        "align_content" => Some("zant::layout::flex::AlignContent"),
        "align_self" => Some("zant::layout::flex::AlignSelf"),
        "justify" | "justify_content" => Some("zant::layout::flex::Justify"),
        "align_items" => Some("zant::layout::flex::Align"),
        "flex_direction" | "direction" => Some("zant::layout::flex::Direction"),
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

/// Whether this prop's Rust option field is `Option<..>`: those values are
/// wrapped in `Some(..)`. This is the complement of the frozen design's
/// "prop → field" mapping (the generated table has no optionality flag);
/// `a_field_is_optional_exactly_when_the_design_says_so` keeps it honest for
/// the tags the golden tests use.
fn optional_field(owner: &str, prop: &Prop) -> bool {
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
fn number_optional(owner: &str, field: &str) -> bool {
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

fn wrap_optional(owner: &str, prop: &Prop, code: String) -> String {
    if optional_field(owner, prop) {
        format!("Some({code})")
    } else {
        code
    }
}

/// Put a field, grouping flattened prop families (`ClickableOptions.box`).
fn put(fields: &mut Fields, prop: &Prop, code: String) {
    fields.group(prop.parent).put(prop.field, code);
}

fn remove(fields: &mut Fields, prop: &Prop, field: &str) {
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
fn prop_named(field: &'static str) -> Prop {
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
fn integer_prop(
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
fn dimension_prop(
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
fn float_literal(n: f32) -> String {
    let mut text = format!("{n}");
    if !text.contains(['.', 'e', 'E']) {
        text.push_str(".0");
    }
    text
}

/// Lower container children or joined text according to the tag table content contract.
///
/// Child components are dispatched first: container children
/// (`jsx_children`) and rich-text/list item builders call this function
/// directly instead of the `emit_jsx` extension wrapper, so the component
/// check must live here too.
pub fn emit(g: &mut Generator<'_>, c: &mut scope::Context, jsx: &ast::Jsx) -> Result<Value> {
    let name = String::from_utf8_lossy(&jsx.name);
    if !name.is_empty()
        && let Some(component) = super::components::find(g, &name)
    {
        let component = component.clone();
        return super::components::emit_call(g, c, jsx, &component);
    }
    emit_named(g, c, jsx, &jsx.name)
}

/// Emission with a tag name override: a fragment keeps its AST (so handler
/// expressions stay the same nodes the scanner saw) but becomes `View`.
fn emit_named(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    jsx: &ast::Jsx,
    name: &[u8],
) -> Result<Value> {
    if name.is_empty() {
        // A node-valued fragment needs a native root; container children are
        // flattened by ChildList.
        return emit_named(g, c, jsx, b"View");
    }
    if name == b"List" {
        not_in_scroll(g, jsx)?;
        return list::emit(g, c, jsx);
    }
    let Some(tag) = tags::find(&String::from_utf8_lossy(name)) else {
        return fail(
            g,
            jsx.loc,
            format!(
                "unknown tag <{}>; valid tags: {}",
                String::from_utf8_lossy(name),
                valid_tags()
            ),
        );
    };
    if tag.instance_id {
        // widgets that keep state per instance (and scroll by themselves)
        if name == b"PdfViewer" {
            not_in_scroll(g, jsx)?;
        }
        return widget_emit(g, c, jsx, tag);
    }
    if name == b"Text" && rich_text::has_elements(jsx) {
        return rich_text::emit(g, c, jsx);
    }
    unique_keys(g, &jsx.children)?;
    let mut fields = Fields::default();
    let mut child_list = ChildList::new();
    for attr in attribute_order(g, c, &jsx.attrs)?.iter() {
        attribute(g, c, tag, &mut fields, attr)?;
    }
    let scrolls = scrolls(jsx);
    if scrolls {
        g.scroll_depth += 1;
    }
    let content = match tag.content {
        Content::Children => node_children(g, c, jsx, tag, &mut fields, &mut child_list),
        Content::TextArg | Content::TextField => text_content(g, c, jsx, tag, &mut fields),
        Content::None => no_children(g, jsx, tag).map(|()| None),
        Content::Child => single_child(g, c, jsx, tag, &mut fields, &mut child_list).map(|()| None),
    };
    if scrolls {
        g.scroll_depth -= 1;
    }
    let text = content?;
    let owner = format!("<{}>", tag.name);
    jsx_values::required(g, &tag.props, &fields, jsx.loc, &owner, "prop")?;
    if tag.content == Content::TextField {
        fields.content_first(tag.content_field);
    }
    let options = fields.emit(&options_path(tag)?);
    let builder = builder_path(tag)?;
    let mut node = match text {
        Some(text) => format!("{builder}({text}, {options})"),
        None => format!("{builder}({options})"),
    };
    // overlays cover the window wherever they are written
    if overlay(name) {
        node = format!("ui::portal({node})");
    }
    let code = child_list.wrap(g, c, &node)?;
    Ok(Value::new(Type::Node, code))
}

/// Build a children array and filter null entries only when necessary.
fn node_children(
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
fn single_child(
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
fn text_content(
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

fn trim_start_spaces(value: &[u8]) -> Vec<u8> {
    let start = value
        .iter()
        .position(|byte| *byte != b' ')
        .unwrap_or(value.len());
    value[start..].to_vec()
}

fn trim_end_spaces(value: &[u8]) -> Vec<u8> {
    let end = value
        .iter()
        .rposition(|byte| *byte != b' ')
        .map_or(0, |position| position + 1);
    value[..end].to_vec()
}

fn is_icon(child: &ast::Child) -> bool {
    matches!(child, ast::Child::Element(element) if element.name == b"Icon")
}

/// One `<Icon>` inside a `<Button>` sets its `icon` (and `icon_size`).
fn button_icon(
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

/// Put a field chosen by name, grouped under its flattened prop family.
fn put_named(fields: &mut Fields, tag: &Tag, field: &str, code: String) {
    match tags::find_prop(tag, field) {
        Some(prop) => put(fields, prop, code),
        None => fields.put(field, code),
    }
}

/// Reject all nonempty children on tags whose content mode is none.
fn no_children(g: &mut Generator<'_>, jsx: &ast::Jsx, tag: &Tag) -> Result<()> {
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

/// Numeric keys use the same formatting rules as text substitutions.
fn string_prop(
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

/// `ui.richText` → `ui::rich_text`, `ui.@"switch"` → `ui::switch`.
fn builder_path(tag: &Tag) -> Result<String> {
    let Some(rest) = tag.builder.strip_prefix("ui.") else {
        return Err(super::Error::Tsx);
    };
    if let Some(inner) = rest.strip_prefix("@\"") {
        let name = inner.trim_end_matches('"');
        return Ok(format!("ui::{name}"));
    }
    Ok(format!("ui::{}", snake_case(rest)))
}

/// The Rust options struct of a tag builder.
fn options_path(tag: &Tag) -> Result<String> {
    let name = match builder_path(tag)?.strip_prefix("ui::") {
        Some(name) => name.to_owned(),
        None => return Err(super::Error::Tsx),
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
fn pascal_case(name: &str) -> String {
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
fn widget_emit(
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

fn key_value(
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
fn markdown_children(
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

/// Whether `attrs` has a spread.
pub(crate) fn spread_any(attrs: &[ast::Attr]) -> bool {
    attrs.iter().any(|attr| attr.spread)
}

/// Attributes in source order. Without spreads the original AST nodes are
/// reused, so handler expressions keep the addresses the scanner registered;
/// spread attributes are cloned by the expansion.
fn attribute_order<'a>(
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
fn spread_flatten(
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

fn spread_put(out: &mut Vec<ast::Attr>, attr: ast::Attr) {
    if let Some(index) = out
        .iter()
        .position(|existing| existing.name.name == attr.name.name)
    {
        out.remove(index);
    }
    out.push(attr);
}

fn object_literal_literals(e: &ast::Expr, out: &mut Vec<ast::Attr>) {
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

fn object_literal_flatten(
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

fn typed_object(
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
fn is_path(e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::Ident(_) => true,
        ast::ExprData::Member(member) => !member.optional && is_path(&member.object),
        _ => false,
    }
}

// ── nesting rules ────────────────────────────────────────────────────

/// The phrase used in errors for what `tag` holds.
fn holds(tag: &Tag) -> &'static str {
    if tag.name == "Text" {
        return "text, {values}, <Text> spans and <Link>";
    }
    if tag.name == "Button" {
        return "its label (text and {values}) and one <Icon>";
    }
    match tag.content {
        Content::Children => "elements",
        Content::TextArg => "text and {values}; use <Text> for spans",
        Content::TextField => "its label as text and {values}, written directly inside it",
        Content::Child => "exactly one element",
        Content::None => "no children",
    }
}

/// `<child>` written inside `tag` where its content model doesn't allow it.
pub fn misplaced<T>(g: &mut Generator<'_>, tag: &Tag, child: &ast::Jsx) -> Result<T> {
    let name = if child.name.is_empty() {
        "a fragment".to_owned()
    } else {
        String::from_utf8_lossy(&child.name).into_owned()
    };
    if tag.content == Content::None {
        return fail(g, child.loc, format!("<{}> has no children", tag.name));
    }
    fail(
        g,
        child.loc,
        format!(
            "<{name}> can't be inside <{}>; it holds {}",
            tag.name,
            holds(tag)
        ),
    )
}

/// Text written directly inside an element that holds elements.
pub fn loose_text<T>(g: &mut Generator<'_>, owner: &str, loc: ast::Loc) -> Result<T> {
    fail(
        g,
        loc,
        format!("text inside <{owner}> must be wrapped in <Text>"),
    )
}

/// Overlays are moved to the window root (`ui::portal`).
fn overlay(name: &[u8]) -> bool {
    name == b"Modal" || name == b"Message"
}

/// `scroll`, `scroll={true}`, `overflow="scroll"` or `style={{ overflow: "scroll" }}`.
fn scrolls(jsx: &ast::Jsx) -> bool {
    for attr in &jsx.attrs {
        if attr.name.name == b"scroll" {
            let Some(value) = attr.value.as_deref() else {
                return true;
            };
            return matches!(value.data, ast::ExprData::Boolean(true));
        }
        if attr.name.name == b"overflow"
            && let Some(value) = attr.value.as_deref()
            && let ast::ExprData::String(text) = &value.data
        {
            return text == b"scroll";
        }
        if attr.name.name == b"style"
            && let Some(value) = attr.value.as_deref()
            && let ast::ExprData::Object(properties) = &value.data
        {
            for property in properties {
                if !property.spread
                    && property.key.name == b"overflow"
                    && let ast::ExprData::String(text) = &property.value.data
                {
                    return text == b"scroll";
                }
            }
        }
    }
    false
}

/// An element that scrolls itself (`<List>`) written inside a scrolling
/// container: it would get unbounded height and never scroll.
pub fn not_in_scroll(g: &mut Generator<'_>, jsx: &ast::Jsx) -> Result<()> {
    if g.scroll_depth == 0 {
        return Ok(());
    }
    fail(
        g,
        jsx.loc,
        format!(
            "<{}> scrolls by itself; put it in a container with a bounded height instead of a scrolling one",
            String::from_utf8_lossy(&jsx.name)
        ),
    )
}

/// Two literal sibling keys (`key="a"`) that are equal.
fn unique_keys(g: &mut Generator<'_>, children: &[ast::Child]) -> Result<()> {
    let mut seen: Vec<Vec<u8>> = Vec::new();
    for child in children {
        let ast::Child::Element(element) = child else {
            continue;
        };
        if element.name.is_empty() {
            unique_keys(g, &element.children)?;
            continue;
        }
        let Some((value, loc)) = literal_key(element) else {
            continue;
        };
        if seen.contains(&value) {
            return fail(
                g,
                loc,
                format!(
                    "two siblings have the key \"{}\"; keys must differ",
                    String::from_utf8_lossy(&value)
                ),
            );
        }
        seen.push(value);
    }
    Ok(())
}

fn literal_key(element: &ast::Jsx) -> Option<(Vec<u8>, ast::Loc)> {
    for attr in &element.attrs {
        if attr.name.name != b"key" {
            continue;
        }
        let value = attr.value.as_deref()?;
        return match &value.data {
            ast::ExprData::String(s) => Some((s.clone(), value.loc)),
            ast::ExprData::Number(n) => Some((n.clone(), value.loc)),
            _ => None,
        };
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tag_maps_to_a_rust_builder_and_options_struct() {
        for tag in tags::all() {
            let builder = builder_path(tag).expect("builder path");
            assert!(
                builder.starts_with("ui::"),
                "tag {} builder {builder}",
                tag.name
            );
            assert!(
                !builder.contains('.') && !builder.contains('@'),
                "tag {} builder {builder} is not a Rust builder path",
                tag.name
            );
            let options = options_path(tag).expect("options path");
            assert!(
                options.starts_with("ui::") && options.ends_with("Options"),
                "tag {} options {options}",
                tag.name
            );
        }
    }

    #[test]
    fn option_structs_with_different_names_are_mapped() {
        let cases = [
            ("Column", "ui::BoxOptions"),
            ("Row", "ui::BoxOptions"),
            ("Switch", "ui::ToggleOptions"),
            ("Checkbox", "ui::ToggleOptions"),
            ("Title", "ui::HeadingOptions"),
            ("PdfPreview", "ui::PdfOptions"),
            ("DropdownMenu", "ui::MenuOptions"),
            ("RichText", "ui::RichTextOptions"),
            ("IconButton", "ui::IconButtonOptions"),
            ("View", "ui::ViewOptions"),
            ("Markdown", "ui::MarkdownOptions"),
        ];
        for (name, expected) in cases {
            let tag = tags::find(name).expect("tag");
            assert_eq!(expected, options_path(tag).expect("options"), "tag {name}");
        }
        assert_eq!(
            "ui::switch",
            builder_path(tags::find("Switch").unwrap()).unwrap()
        );
        assert_eq!(
            "ui::rich_text",
            builder_path(tags::find("RichText").unwrap()).unwrap()
        );
    }

    #[test]
    fn enum_paths_are_known_for_every_enumeration_prop() {
        for tag in tags::all() {
            for prop in &tag.props {
                if prop.kind != Kind::Enumeration {
                    continue;
                }
                assert!(
                    enum_type_path(tag.name, prop.field).is_some(),
                    "tag {} prop {} has no Rust enum path",
                    tag.name,
                    prop.name
                );
            }
        }
    }

    #[test]
    fn optional_fields_match_the_frozen_option_struct_rules() {
        let button = tags::find("Button").unwrap();
        let key = tags::find_prop(button, "key").unwrap();
        assert!(optional_field("Button", key));
        let variant = tags::find_prop(button, "variant").unwrap();
        assert!(!optional_field("Button", variant));
        // key is a plain `Str` on these option structs
        for name in ["Modal", "Message", "ShortcutList", "HotkeyInput"] {
            let tag = tags::find(name).unwrap();
            let prop = tags::find_prop(tag, "key").unwrap();
            assert!(!optional_field(name, prop), "tag {name}");
        }
        let text = tags::find("Text").unwrap();
        assert!(!optional_field(
            "Text",
            tags::find_prop(text, "size").unwrap()
        ));
        let button_size = tags::find_prop(button, "size").unwrap();
        assert!(!optional_field("Button", button_size));
        let column = tags::find("Column").unwrap();
        for field in ["width", "background", "rowGap", "justifyContent"] {
            let prop = tags::find_prop(column, field);
            let Some(prop) = prop else { continue };
            assert!(
                optional_field("Column", prop),
                "Column {field} kind {:?} field {}",
                prop.kind,
                prop.field
            );
        }
    }
}
