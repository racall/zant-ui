//! Text builders of `src/ui.zig`: `heading`, `title`, `paragraph`, `link`,
//! `rich_text` (WP-2.3).
//!
//! The single-line `text` builder is in the parent module (`ui.zig:998-1006`);
//! these wrap it with the antd heading / body styles. Zig's comptime `textFmt`
//! has no Rust counterpart — call sites use `format!` and pass the resulting
//! string, as `pdf_preview` does.
#![forbid(unsafe_code)]

use crate::Str;
use crate::core::node::{ButtonType, FlexStyle, NodeKind};

use super::super::context::current_theme;
use super::super::element::{Element, Node};
use super::super::options::{
    ButtonOptions, HeadingOptions, LinkOptions, ParagraphOptions, RichTextOptions, TextOptions,
};
use super::{button, text};

/// `ui::heading`: a single-line heading at one of the antd level sizes.
pub fn heading(value: impl Into<Str>, options: HeadingOptions) -> Node {
    let size: f32 = match options.level {
        1 => 30.0,
        2 => 24.0,
        3 => 20.0,
        4 => 16.0,
        _ => 14.0,
    };
    text(
        value,
        TextOptions {
            key: options.key,
            size,
            weight: options.weight,
            color: options.color,
            text_align: options.text_align,
            wrap: false,
            ..Default::default()
        },
    )
}

/// `ui::title`: the Zig alias of [`heading`] (a level-1 heading by default).
pub fn title(value: impl Into<Str>, options: HeadingOptions) -> Node {
    heading(value, options)
}

/// `ui::paragraph`: wrapping body text, secondary colour by default.
pub fn paragraph(value: impl Into<Str>, options: ParagraphOptions) -> Node {
    let theme = current_theme();
    text(
        value,
        TextOptions {
            key: options.key,
            size: options.size,
            color: Some(options.color.unwrap_or(theme.color_text_secondary)),
            text_align: options.text_align,
            wrap: true,
            line_height: options.line_height,
            ..Default::default()
        },
    )
}

/// `ui::link`: a `link`-variant button.
pub fn link(options: LinkOptions) -> Node {
    button(ButtonOptions {
        key: options.key,
        text: options.text,
        variant: ButtonType::Link,
        on_click: options.on_click,
        ..Default::default()
    })
}

/// `ui::rich_text`: several styled runs laid out as one wrapping block.
pub fn rich_text(options: RichTextOptions) -> Node {
    Node::new(Element {
        kind: NodeKind::RichText,
        key: options.key,
        rich_spans: options.spans,
        rich_line_height: options.line_height,
        on_link: options.on_link,
        layout: FlexStyle {
            width: options.width,
            height: options.height,
            shrink: 0.0,
            // Zig: a rich text block spans the full parent width unless an
            // explicit width is given.
            width_percent: if options.width.is_none() {
                Some(100.0)
            } else {
                None
            },
            ..FlexStyle::default()
        },
        ..Element::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::node::{ButtonSize, RichSpan};
    use crate::renderer::text as text_mod;
    use crate::theme::tokens;
    use crate::ui::actions::{ActionCx, action};
    use crate::ui::context::set_build_theme;

    fn theme() -> tokens::Theme {
        current_theme()
    }

    #[test]
    fn heading_levels_map_to_the_zig_sizes() {
        let expected: [(u8, f32); 6] = [
            (1, 30.0),
            (2, 24.0),
            (3, 20.0),
            (4, 16.0),
            (5, 14.0),
            (0, 14.0),
        ];
        for (level, size) in expected {
            let node = heading(
                "Title",
                HeadingOptions {
                    level,
                    ..Default::default()
                },
            );
            let element = node.element();
            assert_eq!(element.kind, NodeKind::Text);
            assert_eq!(element.content, "Title");
            assert_eq!(element.text_style.font_size, size, "level {level}");
            assert_eq!(element.text_style.weight, text_mod::Weight::Semibold);
            assert_eq!(element.text_style.color, theme().color_text);
            assert!(!element.text_style.wrap, "headings are single line");
            assert_eq!(element.layout.shrink, 0.0);
        }
    }

    #[test]
    fn heading_keeps_key_colour_weight_and_alignment() {
        let node = heading(
            "H",
            HeadingOptions {
                key: Some("h".into()),
                level: 2,
                color: Some(0xff12_3456),
                weight: text_mod::Weight::Bold,
                text_align: text_mod::Align::Center,
            },
        );
        let element = node.element();
        assert_eq!(element.key.as_deref(), Some("h"));
        assert_eq!(element.text_style.font_size, 24.0);
        assert_eq!(element.text_style.color, 0xff12_3456);
        assert_eq!(element.text_style.weight, text_mod::Weight::Bold);
        assert_eq!(element.text_style.text_align, text_mod::Align::Center);
    }

    #[test]
    fn title_is_a_level_one_heading() {
        let node = title("T", HeadingOptions::default());
        assert_eq!(node.element().kind, NodeKind::Text);
        assert_eq!(node.element().text_style.font_size, 30.0);
    }

    #[test]
    fn paragraph_wraps_with_the_secondary_colour() {
        let node = paragraph(
            "Body",
            ParagraphOptions {
                key: Some("body".into()),
                ..Default::default()
            },
        );
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Text);
        assert_eq!(element.key.as_deref(), Some("body"));
        assert_eq!(element.content, "Body");
        assert_eq!(element.text_style.font_size, 14.0);
        assert_eq!(element.text_style.color, theme().color_text_secondary);
        assert_eq!(element.text_style.line_height, 1.5);
        assert!(element.text_style.wrap);
        assert_eq!(element.layout.shrink, 0.0);

        let custom = paragraph(
            "Custom",
            ParagraphOptions {
                size: 18.0,
                color: Some(0xff00_ff00),
                text_align: text_mod::Align::Right,
                line_height: 1.2,
                ..Default::default()
            },
        );
        let custom = custom.element();
        assert_eq!(custom.text_style.font_size, 18.0);
        assert_eq!(custom.text_style.color, 0xff00_ff00);
        assert_eq!(custom.text_style.text_align, text_mod::Align::Right);
        assert_eq!(custom.text_style.line_height, 1.2);
    }

    #[test]
    fn paragraph_reads_the_build_theme() {
        set_build_theme(tokens::ant_dark());
        let dark = paragraph("dark", ParagraphOptions::default());
        assert_eq!(
            dark.element().text_style.color,
            tokens::ant_dark().color_text_secondary
        );
        set_build_theme(tokens::ant_light());
    }

    #[test]
    fn link_is_a_link_variant_button() {
        let mut state = 0u32;
        let node = link(LinkOptions {
            key: Some("more".into()),
            text: "More".into(),
            on_click: Some(action(|s: &mut u32| *s += 1)),
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Button);
        assert_eq!(element.key.as_deref(), Some("more"));
        assert_eq!(element.content, "More");
        assert_eq!(element.button_type, ButtonType::Link);
        assert_eq!(element.button_size, ButtonSize::Middle);
        assert_eq!(element.layout.height, Some(32.0));
        assert!(element.enabled);
        assert!(element.on_click.is_some());
        {
            let mut cx = ActionCx::new(&mut state);
            if let Some(callback) = element.on_click.clone() {
                callback.call(&mut cx);
            }
        }
        assert_eq!(state, 1);
    }

    #[test]
    fn rich_text_carries_spans_line_height_and_link_handler() {
        let spans = vec![
            RichSpan {
                text: "plain ".into(),
                ..Default::default()
            },
            RichSpan {
                text: "link".into(),
                link: "https://example.com".into(),
                ..Default::default()
            },
        ];
        let node = rich_text(RichTextOptions {
            key: Some("rich".into()),
            spans,
            line_height: 1.6,
            on_link: Some(crate::ui::actions::text_action(|s: &mut u32, value| {
                *s = value.len() as u32
            })),
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::RichText);
        assert_eq!(element.key.as_deref(), Some("rich"));
        assert_eq!(element.rich_spans.len(), 2);
        assert_eq!(element.rich_spans[1].link, "https://example.com");
        assert_eq!(element.rich_line_height, 1.6);
        assert!(element.on_link.is_some());
        assert_eq!(element.layout.shrink, 0.0);
        assert_eq!(element.layout.width, None);
        assert_eq!(element.layout.height, None);
        assert_eq!(
            element.layout.width_percent,
            Some(100.0),
            "no explicit width spans the parent"
        );
    }

    #[test]
    fn rich_text_with_a_width_does_not_span_the_parent() {
        let node = rich_text(RichTextOptions {
            width: Some(200.0),
            height: Some(40.0),
            line_height: 0.0,
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.layout.width, Some(200.0));
        assert_eq!(element.layout.height, Some(40.0));
        assert_eq!(element.layout.width_percent, None);
        assert_eq!(element.rich_line_height, 0.0, "0 uses the font's height");
    }
}
