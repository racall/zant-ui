//! Nesting rules: which tags hold which children, scroll containers, unique keys.

use super::*;

/// The phrase used in errors for what `tag` holds.
pub(super) fn holds(tag: &Tag) -> &'static str {
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
pub(super) fn overlay(name: &[u8]) -> bool {
    name == b"Modal" || name == b"Message"
}

/// `scroll`, `scroll={true}`, `overflow="scroll"` or `style={{ overflow: "scroll" }}`.
pub(super) fn scrolls(jsx: &ast::Jsx) -> bool {
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
pub(super) fn unique_keys(g: &mut Generator<'_>, children: &[ast::Child]) -> Result<()> {
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

pub(super) fn literal_key(element: &ast::Jsx) -> Option<(Vec<u8>, ast::Loc)> {
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
