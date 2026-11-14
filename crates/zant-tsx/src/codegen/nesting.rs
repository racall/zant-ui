//! Which elements may contain which (`docs/tsx.md`, "嵌套规则"). Every tag
//! has a content model, like HTML's:
//!
//! - containers (`View`, `Row`, `Card`, `Clickable`, …): any elements;
//!   text must be inside `<Text>`;
//! - `<Text>`: text, `{values}`, nested `<Text>` spans and inline `<Link>`s;
//! - `Heading` / `Title` / `Paragraph`: text and `{values}`;
//! - labels (`Button`, `Link`, `Checkbox`, `Switch`, `Tag`): text and
//!   `{values}`; a `Button` also takes one `<Icon>`;
//! - one-child slots (`Tooltip`, `Modal`): exactly one element;
//! - leaves (`Input`, `Image`, `Divider`, …): no children.

use crate::ast;
use crate::tags;

use super::{Generator, fail};

/// The phrase used in errors for what `tag` holds.
fn holds(tag: &tags::Tag) -> &'static str {
    if tag.name == "Text" {
        return "text, {values}, <Text> spans and <Link>";
    }
    if tag.name == "Button" {
        return "its label (text and {values}) and one <Icon>";
    }
    match tag.content {
        tags::Content::Children => "elements",
        tags::Content::TextArg => "text and {values}; use <Text> for spans",
        tags::Content::TextField => "its label as text and {values}, written directly inside it",
        tags::Content::Child => "exactly one element",
        tags::Content::None => "no children",
    }
}

/// `<child>` written inside `tag` where its content model doesn't allow it.
pub fn misplaced(g: &mut Generator<'_>, tag: &tags::Tag, child: &ast::Jsx) -> super::Result<()> {
    let name = if child.name.is_empty() {
        "a fragment".to_owned()
    } else {
        String::from_utf8_lossy(&child.name).into_owned()
    };
    if tag.content == tags::Content::None {
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
pub fn loose_text(g: &mut Generator<'_>, owner: &str, loc: ast::Loc) -> super::Result<()> {
    fail(
        g,
        loc,
        format!("text inside <{owner}> must be wrapped in <Text>"),
    )
}

/// Overlays are moved to the window root (`ui::portal`).
pub fn overlay(name: &[u8]) -> bool {
    name == b"Modal" || name == b"Message"
}

/// `scroll`, `scroll={true}`, `overflow="scroll"` or
/// `style={{ overflow: "scroll" }}`.
pub fn scrolls(j: &ast::Jsx) -> bool {
    for attr in &j.attrs {
        if attr.name.name == b"scroll" {
            let Some(value) = attr.value.as_deref() else {
                return true;
            };
            return matches!(&value.data, ast::ExprData::Boolean(true));
        }
        if attr.name.name == b"overflow"
            && let Some(value) = attr.value.as_deref()
        {
            return matches!(&value.data, ast::ExprData::String(s) if s == b"scroll");
        }
        if attr.name.name == b"style"
            && let Some(value) = attr.value.as_deref()
            && let ast::ExprData::Object(properties) = &value.data
        {
            for property in properties {
                if !property.spread && property.key.name == b"overflow" {
                    return matches!(
                        &property.value.data,
                        ast::ExprData::String(s) if s == b"scroll"
                    );
                }
            }
        }
    }
    false
}

/// An element that scrolls itself (`<List>`) written inside a scrolling
/// container: it would get unbounded height and never scroll.
pub fn not_in_scroll(g: &mut Generator<'_>, j: &ast::Jsx) -> super::Result<()> {
    if g.scroll_depth == 0 {
        return Ok(());
    }
    fail(
        g,
        j.loc,
        format!(
            "<{}> scrolls by itself; put it in a container with a bounded height instead of a scrolling one",
            String::from_utf8_lossy(&j.name)
        ),
    )
}

/// Two literal sibling keys (`key="a"`) that are equal.
pub fn unique_keys(g: &mut Generator<'_>, children: &[ast::Child]) -> super::Result<()> {
    let mut seen: Vec<Vec<u8>> = Vec::new();
    for child in children {
        let ast::Child::Element(element) = child else {
            continue;
        };
        if element.name.is_empty() {
            unique_keys(g, &element.children)?;
            continue;
        }
        let Some(key) = literal_key(element) else {
            continue;
        };
        if seen.iter().any(|previous| previous == &key.0) {
            return fail(
                g,
                key.1,
                format!(
                    "two siblings have the key \"{}\"; keys must differ",
                    String::from_utf8_lossy(&key.0)
                ),
            );
        }
        seen.push(key.0);
    }
    Ok(())
}

/// The literal key of an element and its location.
fn literal_key(element: &ast::Jsx) -> Option<(Vec<u8>, ast::Loc)> {
    for attr in &element.attrs {
        if attr.name.name == b"key" {
            let value = attr.value.as_deref()?;
            return match &value.data {
                ast::ExprData::String(s) => Some((s.clone(), value.loc)),
                ast::ExprData::Number(n) => Some((n.clone(), value.loc)),
                _ => None,
            };
        }
    }
    None
}

/// `cond && "text"` in text content: renders "" when false (React renders
/// nothing for `false`). Other expressions are returned unchanged.
pub fn text_expression(_g: &mut Generator<'_>, e: &ast::Expr) -> super::Result<Box<ast::Expr>> {
    let ast::ExprData::Binary(binary) = &e.data else {
        return Ok(Box::new(e.clone()));
    };
    if binary.op != b"&&" {
        return Ok(Box::new(e.clone()));
    }
    let empty = ast::Expr {
        loc: e.loc,
        data: ast::ExprData::String(Vec::new()),
    };
    Ok(Box::new(ast::Expr {
        loc: e.loc,
        data: ast::ExprData::Cond(ast::Cond {
            cond: binary.lhs.clone(),
            then: binary.rhs.clone(),
            else_: Box::new(empty),
        }),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    fn parse_module(src: &str) -> ast::Module {
        let mut parser = Parser::init(src);
        parser.parse_module().expect("test source parses")
    }

    /// The first returned expression of the first function.
    fn returned(module: &ast::Module) -> ast::Expr {
        let function = &module.functions[0].func;
        let value = match &function.body {
            ast::Body::Block(statements) => match &statements[0].data {
                ast::StmtData::Ret(Some(value)) => value,
                _ => panic!("a return statement"),
            },
            ast::Body::Expr(value) => value,
        };
        (**value).clone()
    }

    #[test]
    fn scroll_attributes_are_recognized() {
        let module =
            parse_module("function App() { return <View scroll><Text>a</Text></View>; }\n");
        let value = returned(&module);
        let ast::ExprData::Jsx(jsx) = &value.data else {
            panic!("a JSX element");
        };
        assert!(scrolls(jsx));
    }

    #[test]
    fn literal_sibling_keys_must_differ() {
        let module = parse_module(
            "function App() { return <View><Text key=\"a\">1</Text><Text key=\"a\">2</Text></View>; }\n",
        );
        let value = returned(&module);
        let ast::ExprData::Jsx(jsx) = &value.data else {
            panic!("a JSX element");
        };
        let mut diag = None;
        let mut g = Generator::new(&mut diag);
        assert!(unique_keys(&mut g, &jsx.children).is_err());
        assert!(diag.expect("a diagnostic").msg.contains("keys must differ"));
    }
}
