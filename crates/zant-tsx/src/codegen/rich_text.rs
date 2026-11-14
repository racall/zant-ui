//! `<Text>` with nested `<Text>` spans and inline `<Link>`s, React Native
//! style, compiled to `ui::rich_text`.
//!
//! Each run of text becomes a `tsx::text::span(..)` whose style is its own
//! props over its parents'; a `<Link>` span gets an index into the link
//! actions passed as `on_link` (`tsx::text::links(..)`). The span helpers live
//! in the TSX runtime (`zant/src/tsx/`); they apply the theme defaults.

use crate::ast;
use crate::tags::{self, Kind};

use super::events;
use super::expr;
use super::jsx;
use super::rust_text::{self, Format};
use super::scope;
use super::types::{Type, Value};
use super::{Generator, Result, fail};

/// Whether a `<Text>` has element children (and so is rich text).
pub fn has_elements(jsx: &ast::Jsx) -> bool {
    jsx.children
        .iter()
        .any(|child| matches!(child, ast::Child::Element(_)))
}

/// Span style fields (`tsx::text::SpanStyle`) as Rust field initialisers;
/// a child's entries replace its parent's.
#[derive(Clone, Debug, Default)]
struct Style {
    entries: Vec<(String, String)>,
}

impl Style {
    fn with(mut self, name: &str, code: String) -> Style {
        self.entries.retain(|(field, _)| field != name);
        self.entries.push((name.to_owned(), code));
        self
    }

    fn emit(&self) -> String {
        if self.entries.is_empty() {
            return "tsx::text::SpanStyle { ..Default::default() }".to_owned();
        }
        let mut out = String::from("tsx::text::SpanStyle { ");
        for (name, code) in &self.entries {
            out.push_str(&format!("{name}: {code}, "));
        }
        out.push_str("..Default::default() }");
        out
    }
}

/// The runs of text and their links being collected for one rich-text node.
#[derive(Default)]
struct Builder {
    spans: Vec<String>,
    links: Vec<String>,
    run: Format,
}

impl Builder {
    fn flush(&mut self, g: &mut Generator<'_>, style: &Style) -> Result<()> {
        if self.run.literal.is_empty() && self.run.args.is_empty() {
            return Ok(());
        }
        let text = self.run.finish(g, false)?;
        self.spans.push(format!(
            "tsx::text::span(({text}).as_str(), {})",
            style.emit()
        ));
        self.run = Format::default();
        Ok(())
    }

    fn children(
        &mut self,
        g: &mut Generator<'_>,
        c: &mut scope::Context,
        owner: &tags::Tag,
        children: &[ast::Child],
        style: &Style,
    ) -> Result<()> {
        for child in children {
            match child {
                ast::Child::Text(text) => self.run.text(&text.value),
                ast::Child::Expr(Some(e)) => {
                    let expression = jsx::text_expression(g, e)?;
                    self.run.expression(g, c, &expression)?;
                }
                ast::Child::Expr(None) => {}
                ast::Child::Element(element) => {
                    self.flush(g, style)?;
                    if element.name == b"Text" {
                        let child_style = span_style(g, c, element, style.clone(), false)?;
                        let tag = tags::find("Text").expect("Text tag");
                        self.children(g, c, tag, &element.children, &child_style)?;
                    } else if element.name == b"Link" {
                        let link_style = self.link(g, c, element, style.clone())?;
                        let tag = tags::find("Link").expect("Link tag");
                        self.children(g, c, tag, &element.children, &link_style)?;
                    } else {
                        return jsx::misplaced(g, owner, element);
                    }
                    self.flush(g, style)?;
                }
            }
        }
        self.flush(g, style)
    }

    /// An inline `<Link onClick={…}>`: its index in the link actions.
    fn link(
        &mut self,
        g: &mut Generator<'_>,
        c: &mut scope::Context,
        element: &ast::Jsx,
        style: Style,
    ) -> Result<Style> {
        let mut action = "None".to_owned();
        for attr in &element.attrs {
            if attr.name.name == b"onClick" || attr.name.name == b"onPress" {
                let Some(value) = attr.value.as_deref() else {
                    return fail(g, attr.name.loc, "onClick needs a function");
                };
                action = format!("Some({})", events::event_value(g, c, value, Kind::Action)?);
            } else if attr.name.name != b"key" {
                return fail(
                    g,
                    attr.name.loc,
                    "an inline <Link> takes onClick only; style it with <Text> around it",
                );
            }
        }
        let index = self.links.len();
        self.links.push(action);
        Ok(style.with("link", format!("Some({index})")))
    }
}

/// The style of a nested (or the root) `<Text>`: its props over `parent`.
fn span_style(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    element: &ast::Jsx,
    parent: Style,
    root: bool,
) -> Result<Style> {
    let tag = tags::find("Text").expect("Text tag");
    let mut style = parent;
    for attr in &element.attrs {
        if attr.spread {
            return fail(
                g,
                attr.name.loc,
                "text with spans takes its props one by one (no spread)",
            );
        }
        let name = attr.name.name.as_slice();
        if name == b"key" || (root && name == b"lineHeight") {
            continue;
        }
        if let Some((field, code)) = extra(g, c, attr)? {
            style = style.with(&field, code);
            continue;
        }
        let Some(prop) = tags::find_prop(tag, &String::from_utf8_lossy(name)) else {
            return fail(
                g,
                attr.name.loc,
                format!(
                    "unknown prop `{}` on a <Text> span; spans take size, weight, color, family, italic, underline, strike and background",
                    String::from_utf8_lossy(name)
                ),
            );
        };
        let Some(value) = attr.value.as_deref() else {
            return fail(
                g,
                attr.name.loc,
                format!("`{}` needs a value", String::from_utf8_lossy(name)),
            );
        };
        let code = if prop.field == "size" {
            format!(
                "Some({})",
                expr::emit(g, c, value, Some(Type::F32), false)?.code
            )
        } else if prop.field == "color" {
            format!("Some({})", rust_text::color(g, c, value)?)
        } else if prop.field == "weight" || prop.field == "family" {
            let value = jsx::enum_value(g, c, "Text", prop, value)?;
            format!("Some({value})")
        } else {
            return fail(
                g,
                attr.name.loc,
                format!(
                    "`{}` can't be used on text with spans",
                    String::from_utf8_lossy(name)
                ),
            );
        };
        style = style.with(prop.field, code);
    }
    Ok(style)
}

/// Span-only props: `italic`, `underline`, `strike` (booleans), `background`.
fn extra(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    attr: &ast::Attr,
) -> Result<Option<(String, String)>> {
    let name = attr.name.name.as_slice();
    if name == b"background" {
        let Some(value) = attr.value.as_deref() else {
            return fail(g, attr.name.loc, "background needs a color");
        };
        return Ok(Some((
            "background".to_owned(),
            rust_text::color(g, c, value)?,
        )));
    }
    if !(name == b"italic" || name == b"underline" || name == b"strike") {
        return Ok(None);
    }
    let code = match attr.value.as_deref() {
        Some(value) => expr::emit(g, c, value, Some(Type::Boolean), false)?.code,
        None => "true".to_owned(),
    };
    Ok(Some((String::from_utf8_lossy(name).into_owned(), code)))
}

/// The rich text node of a `<Text>` with element children.
pub fn emit(g: &mut Generator<'_>, c: &mut scope::Context, jsx: &ast::Jsx) -> Result<Value> {
    let mut builder = Builder::default();
    let base = span_style(g, c, jsx, Style::default(), true)?;
    let tag = tags::find("Text").expect("Text tag");
    builder.children(g, c, tag, &jsx.children, &base)?;
    // natural line height, like `ui::text` (rich_text's own default is for Markdown)
    let mut line_height = "0.0".to_owned();
    let mut key = String::new();
    for attr in &jsx.attrs {
        let Some(value) = attr.value.as_deref() else {
            continue;
        };
        if attr.name.name == b"key" {
            let value = expr::emit(g, c, value, Some(Type::String), false)?;
            key = format!("key: Some({}), ", value.code);
        } else if attr.name.name == b"lineHeight" {
            line_height = expr::emit(g, c, value, Some(Type::F32), false)?.code;
        }
    }
    let mut fields = key;
    fields.push_str(&format!("line_height: {line_height}, "));
    fields.push_str(&format!("spans: vec![{}], ", builder.spans.join(", ")));
    if !builder.links.is_empty() {
        fields.push_str(&format!(
            "on_link: Some(tsx::text::links(vec![{}])), ",
            builder.links.join(", ")
        ));
    }
    let node = format!("ui::rich_text(ui::RichTextOptions {{ {fields}..Default::default() }})");
    Ok(Value::new(Type::Node, node))
}
