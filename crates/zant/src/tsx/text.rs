//! Runtime of nested TSX text: `<Text>a <Text weight="bold">b</Text>
//! <Link>c</Link></Text>` compiles to `ui::rich_text` with one span per run.
//!
//! A span's unset style fields fall back to the theme the way `ui::text` does;
//! inline links get an index and a shared `on_link` action dispatching to the
//! `onClick` of the link that was clicked.

use crate::Str;
use crate::core::node::RichSpan;
use crate::renderer::text;
use crate::ui::{self, Action, TextAction};

/// The style of one flattened `<Text>` run.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpanStyle {
    pub size: Option<f32>,
    pub weight: Option<text::Weight>,
    pub color: Option<u32>,
    pub family: Option<text::Family>,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub background: u32,
    /// Index into the text's link actions ([`links`]); `None` for plain text.
    pub link: Option<usize>,
}

/// One span of `value` with `style` (`tsx.text.span`).
pub fn span(value: &str, style: SpanStyle) -> RichSpan {
    let theme = ui::current_theme();
    RichSpan {
        text: Str::copy(value),
        font_size: style.size.unwrap_or(14.0),
        weight: style.weight.unwrap_or(text::Weight::Regular),
        color: style.color.unwrap_or(if style.link.is_some() {
            theme.color_primary
        } else {
            theme.color_text
        }),
        family: style.family.unwrap_or(text::Family::Sans),
        italic: style.italic,
        underline: style.underline,
        strike: style.strike,
        background: style.background,
        link: style.link.map(link_name).unwrap_or_default(),
        ..RichSpan::default()
    }
}

/// The link name of a span index: "0" … "9" without an
/// allocation, larger indexes formatted.
pub fn link_name(index: usize) -> Str {
    match index {
        0 => Str::Static("0"),
        1 => Str::Static("1"),
        2 => Str::Static("2"),
        3 => Str::Static("3"),
        4 => Str::Static("4"),
        5 => Str::Static("5"),
        6 => Str::Static("6"),
        7 => Str::Static("7"),
        8 => Str::Static("8"),
        9 => Str::Static("9"),
        other => Str::copy(&other.to_string()),
    }
}

/// The `on_link` of a text with inline `<Link>`s: a click on link span `i`
/// runs `targets[i]`. The targets are owned by the closure, so they live as
/// long as the node's callback.
pub fn links(targets: Vec<Option<Action>>) -> TextAction {
    TextAction::new(move |cx, name| {
        let Ok(index) = name.parse::<usize>() else {
            return;
        };
        let Some(Some(target)) = targets.get(index) else {
            return;
        };
        target.call(cx);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::node::RichSpan;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn spans_take_theme_defaults_and_links_dispatch_by_index() {
        ui::begin_frame();
        let plain = span("a", SpanStyle::default());
        assert_eq!(plain.font_size, 14.0);
        assert_eq!(plain.link.as_str(), "");
        assert_eq!(plain.weight, text::Weight::Regular);
        assert_eq!(plain.family, text::Family::Sans);
        assert_eq!(plain.color, ui::current_theme().color_text);

        let bold = span(
            "b",
            SpanStyle {
                weight: Some(text::Weight::Bold),
                size: Some(20.0),
                link: Some(1),
                ..SpanStyle::default()
            },
        );
        assert_eq!(bold.weight, text::Weight::Bold);
        assert_eq!(bold.font_size, 20.0);
        assert_eq!(bold.link.as_str(), "1");
        assert_eq!(bold.color, ui::current_theme().color_primary);
        assert_eq!(link_name(12).as_str(), "12");

        let hits = Rc::new(Cell::new(0u32));
        let slot = Rc::clone(&hits);
        let on_link = links(vec![
            None,
            Some(Action::new(move |_| slot.set(slot.get() + 1))),
        ]);
        let mut root = ();
        let mut cx = ui::ActionCx::new(&mut root);
        on_link.call(&mut cx, "1");
        on_link.call(&mut cx, "0");
        on_link.call(&mut cx, "7");
        assert_eq!(hits.get(), 1);
        ui::end_frame();
        let _ = RichSpan::default();
    }
}
