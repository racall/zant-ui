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

use super::app_state;
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

mod children;
mod enums;
mod nesting;
mod props;
mod spread;
#[cfg(test)]
mod tests;
mod widgets;

pub use self::children::*;
pub use self::enums::*;
pub use self::nesting::*;
pub use self::props::*;
pub use self::spread::*;
pub(crate) use self::widgets::*;

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

/// Lower container children or joined text according to the tag table content contract.
///
/// Child components are dispatched first: container children
/// (`jsx_children`) and rich-text/list item builders call this function
/// directly instead of the `emit_jsx` extension wrapper, so the component
/// check must live here too.
pub fn emit(g: &mut Generator<'_>, c: &mut scope::Context, jsx: &ast::Jsx) -> Result<Value> {
    if let Err(diag) = super::generics::jsx_type_args(jsx) {
        return fail(g, diag.loc, diag.msg);
    }

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
    let feature = match name {
        b"Markdown" => app_state::missing_feature("markdown"),
        b"PdfViewer" | b"PdfPreview" => app_state::missing_feature("pdf"),
        _ => None,
    };
    if let Some(feature) = feature {
        let what = format!("<{}>", String::from_utf8_lossy(name));
        return fail(g, jsx.loc, app_state::feature_message(&what, feature));
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
