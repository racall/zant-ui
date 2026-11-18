//! Math blocks, images and raw HTML blocks.

use super::*;

pub(super) fn render_math_block(ctx: &Ctx, math: &ast::Math) -> ui::Node {
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

pub(super) fn resolve_image_path(ctx: &Ctx, url: &str) -> String {
    if url.starts_with('/') || ctx.options.base_dir.is_empty() {
        return url.to_owned();
    }
    let rel = url.strip_prefix("./").unwrap_or(url);
    std::path::Path::new(&*ctx.options.base_dir)
        .join(rel)
        .to_string_lossy()
        .into_owned()
}

pub(super) fn render_image(ctx: &Ctx, image: &ast::Image) -> ui::Node {
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
        radius: 8.0,
        align_items: flex::Align::Center,
        children: lines,
        ..Default::default()
    })
}

pub(super) fn render_html(ctx: &Ctx, raw: &str) -> ui::Node {
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
