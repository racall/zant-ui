//! Port of `src/markdown/render/blocks.zig` — WP-3.3.
//!
//! Block-level components (Streamdown `lib/components.tsx`).
//!
//! Zig allocated every temporary slice in the render arena (`ctx.arena`);
//! the Rust builders own their `Vec`/`String`/`Rc` data, so the functions are
//! infallible and just return the node.

use std::rc::Rc;

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::node::ImageFit;
use crate::image::Image;
use crate::layout::flex;
use crate::markdown::ast;
use crate::markdown::parser::inline;
use crate::markdown::remend;
use crate::markdown::render::code_block;
use crate::markdown::render::context::Ctx;
use crate::markdown::render::data_images;
use crate::markdown::render::diagram;
use crate::markdown::render::spans::{self, Base};
use crate::markdown::render::table;
use crate::markdown::safety;
use crate::math::layout;
use crate::renderer::text;
use crate::ui;
use crate::ui::options::{BoxOptions, DividerOptions, ImageOptions, SpaceOptions, TextOptions};
use crate::ui::styled::Style;

/// True when the block ends with inline text the caret can follow.
fn takes_caret(block: &ast::Block) -> bool {
    match block {
        ast::Block::Paragraph(_) | ast::Block::Heading(_) | ast::Block::Code(_) => true,
        ast::Block::List(list) => list
            .items
            .last()
            .is_some_and(|item| item.blocks.last().is_some_and(takes_caret)),
        ast::Block::Quote(blocks) => blocks.last().is_some_and(takes_caret),
        _ => false,
    }
}

/// Renders a sequence of blocks; the caret goes after the last one.
pub fn sequence(ctx: &Ctx, blocks: &[ast::Block], gap: f32) -> ui::Node {
    let mut children: Vec<ui::Node> = Vec::with_capacity(blocks.len() + 1);
    for (i, block) in blocks.iter().enumerate() {
        let mut c = ctx.clone();
        let last = i + 1 == blocks.len();
        c.caret = ctx.caret && last && takes_caret(block);
        c.key = ctx.sub_key(&format!("{i}"));
        children.push(render(&c, block));
    }
    if ctx.caret && (blocks.is_empty() || !takes_caret(&blocks[blocks.len() - 1])) {
        children.push(spans::body(ctx, &[], true));
    }
    ui::column(BoxOptions {
        key: Some(Str::copy(&ctx.key)),
        width_percent: Some(100.0),
        gap,
        align_items: flex::Align::Stretch,
        children,
        ..Default::default()
    })
}

pub fn render(ctx: &Ctx, block: &ast::Block) -> ui::Node {
    let theme = ctx.options.theme;
    match block {
        ast::Block::Heading(heading) => render_heading(ctx, heading),
        ast::Block::Paragraph(spans) => spans::body(ctx, spans, ctx.caret),
        ast::Block::Code(code) => custom_block(ctx, code).unwrap_or_else(|| {
            if code.lang.eq_ignore_ascii_case("mermaid") {
                diagram::render(ctx, code, ctx.caret)
            } else {
                code_block::render(ctx, code, ctx.caret)
            }
        }),
        ast::Block::Quote(blocks) => render_quote(ctx, blocks),
        ast::Block::Callout(callout) => render_callout(ctx, callout),
        ast::Block::List(list) => render_list(ctx, list, 0),
        ast::Block::Table(table) => table::render(ctx, table),
        ast::Block::Math(math) => render_math_block(ctx, math),
        ast::Block::Image(image) => render_image(ctx, image),
        ast::Block::Html(raw) => render_html(ctx, raw),
        ast::Block::Rule => ui::divider(DividerOptions {
            key: Some(Str::copy(&ctx.key)),
            color: Some(theme.border),
            margin: Edges {
                top: ctx.size * 0.5,
                bottom: ctx.size * 0.5,
                ..Edges::default()
            },
        }),
    }
}

/// A fenced block handed to an app renderer (`Options.block_renderers`).
fn custom_block(ctx: &Ctx, code: &ast::Code) -> Option<ui::Node> {
    for renderer in &ctx.options.block_renderers {
        if !renderer.lang.eq_ignore_ascii_case(&code.lang) {
            continue;
        }
        let node = (renderer.func)(&crate::markdown::render::options::BlockInfo {
            lang: Str::copy(&code.lang),
            meta: Str::copy(&code.meta),
            code: Str::copy(&code.text),
            complete: code.closed,
            key: Str::copy(&ctx.key),
        })?;
        return Some(ui::column(BoxOptions {
            key: Some(Str::copy(&ctx.key)),
            width_percent: Some(100.0),
            align_items: flex::Align::Stretch,
            children: vec![node],
            ..Default::default()
        }));
    }
    None
}

/// Decoded image for a URL: `data:` URIs, or the app's loader for remote
/// and custom URLs.
pub fn load_image(ctx: &Ctx, url: &str) -> Option<Image> {
    if url.starts_with("data:") {
        return data_images::get(url);
    }
    let loader = ctx.options.image_loader.as_ref()?;
    loader(url)
}

fn render_heading(ctx: &Ctx, heading: &ast::Heading) -> ui::Node {
    let theme = ctx.options.theme;
    let scale = theme.heading_scale[heading.level.min(6) as usize - 1];
    let node = spans::rich_node(
        ctx,
        &heading.spans,
        Base {
            size: ctx.size * scale,
            color: theme.heading,
            weight: theme.heading_weight,
            family: theme.heading_family,
            ..Base::default()
        },
        ctx.caret,
    );
    // headings get extra space above (Streamdown: mt-6 mb-2)
    ui::column(BoxOptions {
        key: Some(Str::copy(&ctx.key)),
        width_percent: Some(100.0),
        padding: Edges {
            top: if heading.level <= 2 {
                ctx.size * 0.6
            } else {
                ctx.size * 0.3
            },
            ..Edges::default()
        },
        children: vec![node],
        ..Default::default()
    })
}

fn render_quote(ctx: &Ctx, blocks: &[ast::Block]) -> ui::Node {
    let theme = ctx.options.theme;
    let inner = ctx.with(theme.quote_text, ctx.size);
    ui::row(BoxOptions {
        key: Some(Str::copy(&ctx.key)),
        width_percent: Some(100.0),
        align_items: flex::Align::Stretch,
        children: vec![
            ui::styled(
                ui::space(SpaceOptions {
                    width: Some(3.0),
                    ..Default::default()
                }),
                Style {
                    background: Some(theme.quote_bar),
                    radius: Some(1.5),
                    ..Default::default()
                },
            ),
            ui::column(BoxOptions {
                grow: 1.0,
                shrink: 1.0,
                basis: Some(0.0),
                padding: Edges {
                    left: 16.0,
                    ..Edges::default()
                },
                align_items: flex::Align::Stretch,
                children: vec![sequence(&inner, blocks, ctx.size * 0.75)],
                ..Default::default()
            }),
        ],
        ..Default::default()
    })
}

fn render_callout(ctx: &Ctx, callout: &ast::Callout) -> ui::Node {
    let theme = ctx.options.theme;
    let color = match callout.kind {
        ast::CalloutKind::Note => theme.note,
        ast::CalloutKind::Tip => theme.tip,
        ast::CalloutKind::Important => theme.important,
        ast::CalloutKind::Warning => theme.warning,
        ast::CalloutKind::Caution => theme.caution,
    };
    let icon = match callout.kind {
        ast::CalloutKind::Note => "ⓘ",
        ast::CalloutKind::Tip => "💡",
        ast::CalloutKind::Important => "❢",
        ast::CalloutKind::Warning => "⚠",
        ast::CalloutKind::Caution => "⛔",
    };
    let title = ui::row(BoxOptions {
        gap: 6.0,
        align_items: flex::Align::Center,
        children: vec![
            ui::text(
                icon,
                TextOptions {
                    size: ctx.size * 0.9,
                    color: Some(color),
                    ..Default::default()
                },
            ),
            ui::text(
                callout.kind.label(),
                TextOptions {
                    size: ctx.size * 0.9,
                    weight: text::Weight::Semibold,
                    color: Some(color),
                    ..Default::default()
                },
            ),
        ],
        ..Default::default()
    });
    ui::row(BoxOptions {
        key: Some(Str::copy(&ctx.key)),
        width_percent: Some(100.0),
        align_items: flex::Align::Stretch,
        children: vec![
            ui::styled(
                ui::space(SpaceOptions {
                    width: Some(3.0),
                    ..Default::default()
                }),
                Style {
                    background: Some(color),
                    radius: Some(1.5),
                    ..Default::default()
                },
            ),
            ui::column(BoxOptions {
                grow: 1.0,
                shrink: 1.0,
                basis: Some(0.0),
                padding: Edges {
                    left: 16.0,
                    top: 2.0,
                    bottom: 2.0,
                    ..Edges::default()
                },
                gap: ctx.size * 0.5,
                align_items: flex::Align::Stretch,
                children: vec![title, sequence(ctx, &callout.blocks, ctx.size * 0.75)],
                ..Default::default()
            }),
        ],
        ..Default::default()
    })
}

fn render_list(ctx: &Ctx, list: &ast::List, depth: usize) -> ui::Node {
    let theme = ctx.options.theme;
    let mut items: Vec<ui::Node> = Vec::with_capacity(list.items.len());
    // third level: a small square. "▪" has an emoji form that font fallback
    // picks (and that ignores the colour), so a half-size "■" is used.
    const BULLETS: [&str; 3] = ["•", "◦", "■"];
    let mut marker_width: f32 = ctx.size * 1.1;
    if list.ordered {
        let last_number = list.start + list.items.len();
        marker_width = (format!("{last_number}").len() + 1) as f32 * ctx.size * 0.6 + 6.0;
    }
    for (i, item) in list.items.iter().enumerate() {
        let mut c = ctx.clone();
        c.caret = ctx.caret && i + 1 == list.items.len();
        c.key = ctx.sub_key(&format!("i{i}"));
        let marker_text: String = match item.checked {
            Some(checked) => {
                if checked {
                    "☑".to_owned()
                } else {
                    "☐".to_owned()
                }
            }
            None => {
                if list.ordered {
                    format!("{}.", list.start + i)
                } else {
                    BULLETS[depth % BULLETS.len()].to_owned()
                }
            }
        };
        let square = item.checked.is_none() && !list.ordered && depth % BULLETS.len() == 2;
        let checked_color = |checked: bool| {
            if checked { theme.link } else { theme.marker }
        };
        let marker_label = ui::text(
            marker_text,
            TextOptions {
                size: if square { ctx.size * 0.5 } else { ctx.size },
                color: Some(match item.checked {
                    Some(checked) => checked_color(checked),
                    None => theme.marker,
                }),
                // same line box as the item text, so the marker stays centred
                line_height: if square {
                    theme.line_height * 2.0
                } else {
                    theme.line_height
                },
                ..Default::default()
            },
        );
        // markers are not copied (like list markers in a browser)
        let marker = ui::row(BoxOptions {
            justify: if list.ordered {
                flex::Justify::End
            } else {
                flex::Justify::Start
            },
            unselectable: true,
            children: vec![marker_label],
            ..Default::default()
        });
        // nested lists render one level deeper
        let mut children: Vec<ui::Node> = Vec::with_capacity(item.blocks.len());
        for (b, block) in item.blocks.iter().enumerate() {
            let mut bc = c.clone();
            bc.caret = c.caret && b + 1 == item.blocks.len();
            bc.key = c.sub_key(&format!("{b}"));
            let node = match block {
                ast::Block::List(nested) => render_list(&bc, nested, depth + 1),
                other => render(&bc, other),
            };
            children.push(node);
        }
        if children.is_empty() {
            children.push(spans::body(&c, &[], c.caret));
        }
        items.push(ui::row(BoxOptions {
            key: Some(Str::copy(&c.key)),
            width_percent: Some(100.0),
            align_items: flex::Align::Start,
            children: vec![
                ui::styled(
                    marker,
                    Style {
                        width: Some(marker_width),
                        ..Default::default()
                    },
                ),
                ui::column(BoxOptions {
                    grow: 1.0,
                    shrink: 1.0,
                    basis: Some(0.0),
                    padding: Edges {
                        left: 6.0,
                        ..Edges::default()
                    },
                    gap: if list.tight {
                        ctx.size * 0.25
                    } else {
                        ctx.size * 0.75
                    },
                    align_items: flex::Align::Stretch,
                    children,
                    ..Default::default()
                }),
            ],
            ..Default::default()
        }));
    }
    ui::column(BoxOptions {
        key: Some(Str::copy(&ctx.key)),
        width_percent: Some(100.0),
        gap: if list.tight {
            ctx.size * 0.25
        } else {
            ctx.size * 0.6
        },
        align_items: flex::Align::Stretch,
        children: items,
        ..Default::default()
    })
}

fn render_math_block(ctx: &Ctx, math: &ast::Math) -> ui::Node {
    let theme = ctx.options.theme;
    let drawing = Rc::new(layout::layout(
        &math.source,
        layout::Options {
            size: ctx.size,
            color: theme.math,
            error_color: theme.math_error,
            display: true,
            max_width: Some(ctx.options.content_width),
            ..Default::default()
        },
    ));
    let mut children: Vec<ui::Node> = Vec::with_capacity(2);
    children.push(ui::vector(crate::ui::options::VectorOptions {
        drawing,
        copy: format!("$$\n{}\n$$", math.source).into(),
        ..Default::default()
    }));
    if ctx.caret {
        children.push(ui::text(
            ctx.options.caret.glyph(),
            TextOptions {
                size: ctx.size * 0.9,
                color: Some(theme.caret),
                ..Default::default()
            },
        ));
    }
    ui::row(BoxOptions {
        key: Some(Str::copy(&ctx.key)),
        width_percent: Some(100.0),
        justify: flex::Justify::Center,
        align_items: flex::Align::Center,
        padding: Edges {
            top: ctx.size * 0.25,
            bottom: ctx.size * 0.25,
            ..Edges::default()
        },
        children,
        ..Default::default()
    })
}

fn resolve_image_path(ctx: &Ctx, url: &str) -> String {
    if url.starts_with('/') || ctx.options.base_dir.is_empty() {
        return url.to_owned();
    }
    let rel = url.strip_prefix("./").unwrap_or(url);
    std::path::Path::new(&*ctx.options.base_dir)
        .join(rel)
        .to_string_lossy()
        .into_owned()
}

fn render_image(ctx: &Ctx, image: &ast::Image) -> ui::Node {
    let theme = ctx.options.theme;
    let placeholder = image.url == remend::INCOMPLETE_IMAGE;
    let has_scheme = image.url.contains("://") || image.url.starts_with("data:");
    let allowed =
        !placeholder && safety::check(&ctx.options.policy, safety::Kind::Image, &image.url);
    let loaded: Option<Image> = if allowed && has_scheme {
        load_image(ctx, &image.url)
    } else {
        None
    };
    if allowed && (!has_scheme || loaded.is_some()) {
        let mut children: Vec<ui::Node> = Vec::with_capacity(2);
        let key = ctx.sub_key("img");
        let node = match loaded {
            Some(decoded) => ui::image(ImageOptions {
                key: Some(Str::copy(&key)),
                image: Some(Rc::new(decoded)),
                fit: ImageFit::Contain,
                radius: 8.0,
                ..Default::default()
            }),
            None => ui::image(ImageOptions {
                key: Some(Str::copy(&key)),
                src: resolve_image_path(ctx, &image.url).into(),
                fit: ImageFit::Contain,
                radius: 8.0,
                ..Default::default()
            }),
        };
        children.push(ui::styled(
            node,
            Style {
                max_width: Some(ctx.options.content_width),
                ..Default::default()
            },
        ));
        if !image.alt.is_empty() {
            children.push(ui::text(
                image.alt.clone(),
                TextOptions {
                    size: ctx.size * 0.8,
                    color: Some(theme.muted_foreground),
                    ..Default::default()
                },
            ));
        }
        return ui::column(BoxOptions {
            key: Some(Str::copy(&ctx.key)),
            width_percent: Some(100.0),
            gap: 6.0,
            align_items: flex::Align::Center,
            children,
            ..Default::default()
        });
    }
    // remote / blocked / still streaming: a labelled placeholder card
    let label: String = if placeholder {
        ctx.options.labels.loading_image.to_string()
    } else if !image.alt.is_empty() {
        image.alt.clone()
    } else {
        ctx.options.labels.image.to_string()
    };
    let mut lines: Vec<ui::Node> = Vec::with_capacity(2);
    lines.push(ui::text(
        format!("🖼  {label}"),
        TextOptions {
            size: ctx.size * 0.875,
            color: Some(theme.muted_foreground),
            ..Default::default()
        },
    ));
    if !placeholder && !image.url.is_empty() {
        lines.push(ui::text(
            image.url.clone(),
            TextOptions {
                size: ctx.size * 0.75,
                color: Some(theme.muted_foreground),
                max_lines: 1,
                ..Default::default()
            },
        ));
    }
    ui::column(BoxOptions {
        key: Some(Str::copy(&ctx.key)),
        width_percent: Some(100.0),
        padding: Edges::all(16.0),
        gap: 4.0,
        background: Some(theme.muted),
        align_items: flex::Align::Center,
        children: lines,
        ..Default::default()
    })
}

fn render_html(ctx: &Ctx, raw: &str) -> ui::Node {
    // Streamdown sanitises HTML; natively we keep the text and inline
    // formatting (<b>, <em>, <code>, <a>, <br>) and drop block tags.
    let refs = inline::RefMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut ictx = inline::Context {
        refs: &refs,
        footnote_order: &mut order,
        single_dollar_math: true,
        latex_delimiters: false,
    };
    let mut joined = String::with_capacity(raw.len());
    for line in raw.split('\n') {
        let t = line.trim_matches([' ', '\t']);
        if t.is_empty() {
            continue;
        }
        if !joined.is_empty() {
            joined.push('\n');
        }
        joined.push_str(t);
    }
    let parsed = inline::parse(&mut ictx, &joined);
    // drop the whitespace-only runs left by stripped block tags
    let mut kept: Vec<ast::Span> = Vec::with_capacity(parsed.len());
    for span in parsed {
        if span.text.trim_matches([' ', '\n', '\t']).is_empty() && kept.is_empty() {
            continue;
        }
        kept.push(span);
    }
    if kept.is_empty() && !ctx.caret {
        return ui::space(SpaceOptions {
            key: Some(Str::copy(&ctx.key)),
            height: Some(0.0),
            ..Default::default()
        });
    }
    spans::body(ctx, &kept, ctx.caret)
}

pub fn footnotes(ctx: &Ctx, notes: &[ast::Footnote]) -> ui::Node {
    let theme = ctx.options.theme;
    let small = ctx.with(theme.muted_foreground, ctx.size * 0.875);
    let mut rows: Vec<ui::Node> = Vec::with_capacity(notes.len() + 2);
    rows.push(ui::divider(DividerOptions {
        color: Some(theme.border),
        margin: Edges {
            top: ctx.size * 0.5,
            bottom: ctx.size * 0.25,
            ..Edges::default()
        },
        ..Default::default()
    }));
    rows.push(ui::text(
        ctx.options.labels.footnotes.clone(),
        TextOptions {
            size: small.size,
            weight: text::Weight::Semibold,
            color: Some(theme.muted_foreground),
            ..Default::default()
        },
    ));
    for (i, note) in notes.iter().enumerate() {
        let mut c = small.clone();
        c.key = ctx.sub_key(&format!("fn{i}"));
        rows.push(ui::row(BoxOptions {
            key: Some(Str::copy(&c.key)),
            width_percent: Some(100.0),
            align_items: flex::Align::Start,
            children: vec![
                ui::styled(
                    ui::text(
                        format!("{}.", note.number),
                        TextOptions {
                            size: small.size,
                            color: Some(theme.muted_foreground),
                            line_height: theme.line_height,
                            ..Default::default()
                        },
                    ),
                    Style {
                        width: Some(small.size * 2.0),
                        ..Default::default()
                    },
                ),
                ui::column(BoxOptions {
                    grow: 1.0,
                    shrink: 1.0,
                    basis: Some(0.0),
                    align_items: flex::Align::Stretch,
                    children: vec![sequence(&c, &note.blocks, small.size * 0.5)],
                    ..Default::default()
                }),
            ],
            ..Default::default()
        }));
    }
    ui::column(BoxOptions {
        key: Some(Str::copy(&ctx.sub_key("footnotes"))),
        width_percent: Some(100.0),
        gap: small.size * 0.4,
        align_items: flex::Align::Stretch,
        children: rows,
        ..Default::default()
    })
}
