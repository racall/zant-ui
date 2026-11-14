//! Code block (Streamdown `code-block/*`): header with the language and
//! copy/download controls, highlighted body with optional line numbers,
//! scrolling past `code_max_height`.

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::node::RichSpan;
use crate::layout::flex;
use crate::markdown::ast;
use crate::markdown::highlight;
use crate::markdown::render::actions::{Copy, Download};
use crate::markdown::render::context::Ctx;
use crate::markdown::render::controls;
use crate::renderer::text;
use crate::ui;
use crate::ui::options::{BoxOptions, DividerOptions, RichTextOptions, TextOptions};

fn token_color(colors: crate::markdown::theme::CodeColors, kind: highlight::Kind) -> u32 {
    match kind {
        highlight::Kind::Plain => colors.plain,
        highlight::Kind::Keyword => colors.keyword,
        highlight::Kind::String => colors.string,
        highlight::Kind::Comment => colors.comment,
        highlight::Kind::Number => colors.number,
        highlight::Kind::Function => colors.function,
        highlight::Kind::Type => colors.type_,
        highlight::Kind::Constant => colors.constant,
        highlight::Kind::Property => colors.property,
        highlight::Kind::Tag => colors.tag,
        highlight::Kind::Attribute => colors.attribute,
        highlight::Kind::Punctuation => colors.punctuation,
        highlight::Kind::Meta => colors.meta,
        highlight::Kind::Inserted => colors.inserted,
        highlight::Kind::Deleted => colors.deleted,
    }
}

pub fn render(ctx: &Ctx, code: &ast::Code, caret: bool) -> ui::Node {
    let theme = ctx.options.theme;
    let size = ctx.options.font_size * 0.8125;
    let line_height: f32 = 1.65;

    // header: language + controls
    let incomplete = !code.closed && ctx.options.is_animating;
    let lang_label = ui::text(
        if !code.lang.is_empty() {
            code.lang.clone()
        } else {
            "text".to_owned()
        },
        TextOptions {
            size: 12.0,
            family: text::Family::Mono,
            color: Some(theme.muted_foreground),
            ..Default::default()
        },
    );
    let mut buttons: Vec<ui::Node> = Vec::with_capacity(2);
    if ctx.options.controls.code.copy {
        buttons.push(controls::button(
            ctx,
            &ctx.sub_key("copy"),
            &ctx.options.labels.copy,
            Some(Copy::action(&code.text)),
            !incomplete,
        ));
    }
    if ctx.options.controls.code.download {
        let ext = highlight::extension(&code.lang);
        buttons.push(controls::button(
            ctx,
            &ctx.sub_key("download"),
            &ctx.options.labels.download,
            Some(Download::action(Download {
                filename: format!("file.{ext}"),
                extensions: String::new(),
                text: code.text.clone(),
                ..Default::default()
            })),
            !incomplete,
        ));
    }
    let header = ui::row(BoxOptions {
        key: Some(Str::copy(&ctx.sub_key("header"))),
        width_percent: Some(100.0),
        height: Some(34.0),
        padding: Edges {
            left: 14.0,
            right: 6.0,
            ..Edges::default()
        },
        align_items: flex::Align::Center,
        justify: flex::Justify::SpaceBetween,
        background: Some(theme.code_header_bg),
        unselectable: true,
        children: vec![
            lang_label,
            ui::row(BoxOptions {
                gap: controls::GAP,
                align_items: flex::Align::Center,
                children: buttons,
                ..Default::default()
            }),
        ],
        ..Default::default()
    });

    // body: highlighted lines
    let mut state = highlight::State::default();
    let total_lines = code.text.split('\n').count();
    let digits = format!("{total_lines}").len();
    let gutter_w = digits as f32 * size * 0.62 + 18.0;
    let mut line_nodes: Vec<ui::Node> = Vec::with_capacity(total_lines);
    let mut all_runs: Vec<RichSpan> = Vec::new();
    for (count, line) in code.text.split('\n').enumerate() {
        let tokens = highlight::line(&code.lang, line, &mut state);
        let mut runs: Vec<RichSpan> = Vec::with_capacity(tokens.len() + 2);
        for token in tokens {
            let mut run = RichSpan {
                text: Str::copy(token.text),
                font_size: size,
                family: text::Family::Mono,
                color: token_color(theme.code, token.kind),
                ..RichSpan::default()
            };
            if token.kind == highlight::Kind::Inserted {
                run.background = theme.code.inserted_bg;
            }
            if token.kind == highlight::Kind::Deleted {
                run.background = theme.code.deleted_bg;
            }
            runs.push(run);
        }
        let last = count + 1 == total_lines;
        if caret && last {
            runs.push(RichSpan {
                text: ctx.options.caret.glyph().into(),
                font_size: size,
                color: theme.caret,
                copy: Some(Str::from("")),
                ..RichSpan::default()
            });
        }
        if runs.is_empty() {
            runs.push(RichSpan {
                text: " ".into(),
                font_size: size,
                family: text::Family::Mono,
                color: theme.code.plain,
                ..RichSpan::default()
            });
        }
        if ctx.options.line_numbers {
            line_nodes.push(ui::row(BoxOptions {
                width_percent: Some(100.0),
                align_items: flex::Align::Start,
                children: vec![
                    ui::row(BoxOptions {
                        width: Some(gutter_w),
                        justify: flex::Justify::End,
                        unselectable: true,
                        children: vec![ui::text(
                            format!("{}", count + 1),
                            TextOptions {
                                size,
                                family: text::Family::Mono,
                                color: Some(theme.line_number),
                                line_height,
                                ..Default::default()
                            },
                        )],
                        ..Default::default()
                    }),
                    ui::column(BoxOptions {
                        grow: 1.0,
                        shrink: 1.0,
                        basis: Some(0.0),
                        margin: Edges {
                            left: 12.0,
                            ..Edges::default()
                        },
                        children: vec![ui::rich_text(RichTextOptions {
                            spans: runs,
                            line_height,
                            ..Default::default()
                        })],
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            }));
        } else {
            if count > 0 {
                all_runs.push(RichSpan {
                    text: "\n".into(),
                    font_size: size,
                    family: text::Family::Mono,
                    color: theme.code.plain,
                    ..RichSpan::default()
                });
            }
            all_runs.extend(runs);
        }
    }
    let body_children: Vec<ui::Node> = if ctx.options.line_numbers {
        line_nodes
    } else {
        vec![ui::rich_text(RichTextOptions {
            spans: all_runs,
            line_height,
            ..Default::default()
        })]
    };
    let est_height = total_lines as f32 * size * line_height + 28.0;
    let max_h = ctx.options.code_max_height;
    let scrolls = max_h > 0.0 && est_height > max_h;
    let body = ui::column(BoxOptions {
        key: Some(Str::copy(&ctx.sub_key("body"))),
        width_percent: Some(100.0),
        padding: Edges {
            left: if ctx.options.line_numbers { 2.0 } else { 16.0 },
            right: 16.0,
            top: 14.0,
            bottom: 14.0,
        },
        height: if scrolls { Some(max_h) } else { None },
        scroll: scrolls,
        background: Some(theme.code_bg),
        align_items: flex::Align::Stretch,
        children: body_children,
        ..Default::default()
    });
    ui::column(BoxOptions {
        key: Some(Str::copy(&ctx.key)),
        width_percent: Some(100.0),
        border_color: Some(theme.code_border),
        border_width: 1.0,
        radius: 10.0,
        clip: true,
        align_items: flex::Align::Stretch,
        children: vec![
            header,
            ui::divider(DividerOptions {
                color: Some(theme.code_border),
                ..Default::default()
            }),
            body,
        ],
        ..Default::default()
    })
}
