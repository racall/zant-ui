//! Block-level components (Streamdown `lib/components.tsx`).
//!
//! The builders own their `Vec`/`String`/`Rc` data, so the functions are
//! infallible and just return the node.

use std::rc::Rc;

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::node::ImageFit;
use crate::flex;
use crate::image::Image;
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

mod containers;
mod media;

use self::containers::*;
use self::media::*;

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
