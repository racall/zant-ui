//! Inline spans → rich text runs (links, inline code, inline math embeds,
//! highlights, footnote references, the streaming caret).
//!
//! The `RichSpan`s own `Str`s and `Rc<Drawing>`s, so the result lives as long
//! as the node that holds it (block caches included).

use crate::Str;
use crate::core::node::RichSpan;
use crate::markdown::ast;
use crate::markdown::remend;
use crate::markdown::render::context::Ctx;
use crate::markdown::safety;
use crate::math::{inline_drawing, layout};
use crate::renderer::text;
use crate::ui;
use crate::ui::options::{RichTextOptions, SpaceOptions};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Base {
    pub size: f32,
    pub color: u32,
    pub weight: text::Weight,
    pub family: text::Family,
    pub italic: bool,
}

impl Default for Base {
    /// `size` / `color` have no meaningful default; the neutral values keep
    /// `..Default::default()` usable (the render path always sets both).
    fn default() -> Self {
        Base {
            size: 0.0,
            color: 0,
            weight: text::Weight::Regular,
            family: text::Family::Sans,
            italic: false,
        }
    }
}

pub fn to_rich(ctx: &Ctx, spans: &[ast::Span], base: Base, with_caret: bool) -> Vec<RichSpan> {
    let theme = &ctx.options.theme;
    let mut out: Vec<RichSpan> = Vec::with_capacity(spans.len() + usize::from(with_caret));
    for span in spans {
        let style = span.style;
        let mut run = RichSpan {
            text: Str::copy(&span.text),
            font_size: base.size,
            weight: if style.bold {
                if base.weight as i32 >= text::Weight::Semibold as i32 {
                    text::Weight::Bold
                } else {
                    text::Weight::Semibold
                }
            } else {
                base.weight
            },
            color: base.color,
            italic: base.italic || style.italic,
            family: base.family,
            strike: style.strike,
            underline: style.underline,
            ..RichSpan::default()
        };
        if style.superscript || style.subscript {
            run.font_size = base.size * 0.75;
        }
        if style.highlight {
            run.background = theme.highlight_bg;
        }
        if style.code {
            run.family = text::Family::Mono;
            run.font_size = base.size * 0.875;
            run.background = theme.inline_code_bg;
            run.color = theme.inline_code_fg;
            // pad the code chip with thin spaces
            run.text = format!("\u{2009}{}\u{2009}", span.text).into();
            run.copy = Some(Str::copy(&span.text));
        }
        if style.math {
            run.embed = Some(inline_drawing(
                &span.text,
                layout::Options {
                    size: base.size,
                    color: if base.color == theme.foreground {
                        theme.math
                    } else {
                        base.color
                    },
                    error_color: theme.math_error,
                    ..layout::Options::default()
                },
            ));
            run.text = "\u{FFFC}".into();
            run.copy = Some(format!("${}$", span.text).into());
        }
        if style.footnote_ref {
            run.color = theme.link;
            run.text = format!("[{}]", span.text).into();
        }
        if style.image {
            run.color = theme.muted_foreground;
            run.text = format!("🖼 {}", span.text).into();
        }
        if style.link && !style.image {
            let incomplete = span.url == "#incomplete" || span.url == remend::INCOMPLETE_LINK;
            if !incomplete && safety::check(&ctx.options.policy, safety::Kind::Link, &span.url) {
                run.color = theme.link;
                run.underline = true;
                run.link = Str::copy(&span.url);
            } else {
                run.color = if incomplete {
                    theme.link
                } else {
                    theme.link_blocked
                };
            }
        }
        out.push(run);
    }
    if with_caret {
        out.push(RichSpan {
            text: ctx.options.caret.glyph().into(),
            font_size: base.size * 0.9,
            color: theme.caret,
            family: text::Family::Sans,
            copy: Some(Str::from("")),
            ..RichSpan::default()
        });
    }
    out
}

pub fn rich_node(ctx: &Ctx, spans: &[ast::Span], base: Base, with_caret: bool) -> ui::Node {
    if spans.is_empty() && !with_caret {
        return ui::space(SpaceOptions {
            height: Some(1.0),
            ..Default::default()
        });
    }
    let runs = to_rich(ctx, spans, base, with_caret);
    ui::rich_text(RichTextOptions {
        spans: runs,
        line_height: ctx.options.theme.line_height,
        on_link: ctx.link_action.clone(),
        ..Default::default()
    })
}

/// Body text with the context's colour and size.
pub fn body(ctx: &Ctx, spans: &[ast::Span], with_caret: bool) -> ui::Node {
    rich_node(
        ctx,
        spans,
        Base {
            size: ctx.size,
            color: ctx.fg,
            family: ctx.family(),
            ..Base::default()
        },
        with_caret,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::ast::Style;
    use std::rc::Rc;

    fn base() -> Base {
        Base {
            size: 16.0,
            color: 0xff0a0a0a,
            ..Base::default()
        }
    }

    fn options() -> Rc<crate::markdown::render::options::Options> {
        Rc::new(crate::markdown::render::options::Options::default())
    }

    #[test]
    fn inline_styles_become_runs() {
        let ctx = Ctx::init(&options());
        let spans = vec![
            ast::Span {
                text: "bold".into(),
                style: Style {
                    bold: true,
                    ..Style::default()
                },
                url: String::new(),
            },
            ast::Span {
                text: "code".into(),
                style: Style {
                    code: true,
                    ..Style::default()
                },
                url: String::new(),
            },
            ast::Span {
                text: "x^2".into(),
                style: Style {
                    math: true,
                    ..Style::default()
                },
                url: String::new(),
            },
            ast::Span {
                text: "link".into(),
                style: Style {
                    link: true,
                    ..Style::default()
                },
                url: "https://example.com".into(),
            },
        ];
        let runs = to_rich(&ctx, &spans, base(), true);
        assert_eq!(runs.len(), 5, "4 spans + the caret");
        assert_eq!(runs[0].weight, text::Weight::Semibold);
        assert_eq!(runs[1].family, text::Family::Mono);
        assert_eq!(runs[1].text.as_str(), "\u{2009}code\u{2009}");
        assert_eq!(
            runs[1].copy.as_deref().map(str::to_string),
            Some("code".into())
        );
        assert!(runs[2].embed.is_some());
        assert_eq!(runs[2].text.as_str(), "\u{FFFC}");
        assert_eq!(
            runs[2].copy.as_deref().map(str::to_string),
            Some("$x^2$".into())
        );
        assert_eq!(runs[3].color, ctx.options.theme.link);
        assert!(runs[3].underline);
        assert_eq!(runs[3].link.as_str(), "https://example.com");
        assert_eq!(runs[4].color, ctx.options.theme.caret);
        assert_eq!(
            runs[4].copy.as_deref().map(str::to_string),
            Some(String::new())
        );
    }

    #[test]
    fn bold_inside_bold_stays_bold() {
        let ctx = Ctx::init(&options());
        let spans = vec![ast::Span {
            text: "x".into(),
            style: Style {
                bold: true,
                ..Style::default()
            },
            url: String::new(),
        }];
        let runs = to_rich(
            &ctx,
            &spans,
            Base {
                weight: text::Weight::Bold,
                ..base()
            },
            false,
        );
        assert_eq!(runs[0].weight, text::Weight::Bold);
    }

    #[test]
    fn empty_spans_without_a_caret_are_a_spacer() {
        crate::ui::begin_frame();
        let ctx = Ctx::init(&options());
        let node = rich_node(&ctx, &[], base(), false);
        crate::ui::end_frame();
        assert_eq!(node.element().kind, crate::core::node::NodeKind::Container);
        assert_eq!(node.element().layout.height, Some(1.0));
    }

    #[test]
    fn blocked_links_are_not_clickable() {
        let mut options = crate::markdown::render::options::Options::default();
        options.policy.allowed_link_prefixes = &["https://zant.dev/"];
        let ctx = Ctx::init(&Rc::new(options));
        let spans = vec![ast::Span {
            text: "evil".into(),
            style: Style {
                link: true,
                ..Style::default()
            },
            url: "https://example.com".into(),
        }];
        let runs = to_rich(&ctx, &spans, base(), false);
        assert_eq!(runs[0].color, ctx.options.theme.link_blocked);
        assert!(runs[0].link.is_empty());
    }
}
