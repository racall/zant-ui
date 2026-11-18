//! Mermaid diagram block (Streamdown `mermaid/*`) with copy/download of the
//! source. Unsupported diagram types fall back to a code block.

use std::rc::Rc;

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::vector::Drawing;
use crate::flex;
use crate::markdown::ast;
use crate::markdown::mermaid;
use crate::markdown::render::actions::{Copy, Download};
use crate::markdown::render::code_block;
use crate::markdown::render::context::Ctx;
use crate::markdown::render::controls;
use crate::ui;
use crate::ui::options::{BoxOptions, VectorOptions};

pub fn render(ctx: &Ctx, code: &ast::Code, caret: bool) -> ui::Node {
    let theme = ctx.options.theme;
    // Streamdown shows the source until the fence closes
    if !code.closed {
        return code_block::render(ctx, code, caret);
    }
    let Some(drawing) = mermaid::render(
        &code.text,
        mermaid::Options {
            font_size: ctx.options.font_size * 0.875,
            max_width: ctx.options.content_width - 32.0,
            colors: mermaid::Colors {
                text: theme.foreground,
                node_fill: if theme.dark { 0xff1f2937 } else { 0xfff4f4f5 },
                node_border: if theme.dark { 0xff4b5563 } else { 0xffa1a1aa },
                edge: theme.muted_foreground,
                label_bg: theme.background,
                muted: theme.muted_foreground,
            },
        },
    ) else {
        return code_block::render(ctx, code, caret);
    };
    let drawing: Rc<Drawing> = Rc::new(drawing);

    let mut buttons: Vec<ui::Node> = Vec::with_capacity(2);
    let mc = ctx.options.controls.mermaid;
    if mc.copy {
        buttons.push(controls::button(
            ctx,
            &ctx.sub_key("copy"),
            &ctx.options.labels.copy,
            Some(Copy::action(&code.text)),
            true,
        ));
    }
    if mc.download {
        buttons.push(controls::button(
            ctx,
            &ctx.sub_key("download"),
            &ctx.options.labels.download,
            Some(Download::action(Download {
                filename: "diagram.mmd".to_owned(),
                extensions: "mmd".to_owned(),
                text: code.text.clone(),
                ..Default::default()
            })),
            true,
        ));
    }
    ui::column(BoxOptions {
        key: Some(Str::copy(&ctx.key)),
        width_percent: Some(100.0),
        border_color: Some(theme.border),
        border_width: 1.0,
        radius: 10.0,
        clip: true,
        padding: Edges {
            bottom: 16.0,
            ..Edges::default()
        },
        align_items: flex::Align::Stretch,
        children: vec![
            ui::row(BoxOptions {
                width_percent: Some(100.0),
                justify: flex::Justify::End,
                padding: Edges {
                    top: 4.0,
                    right: 6.0,
                    ..Edges::default()
                },
                gap: controls::GAP,
                children: buttons,
                ..Default::default()
            }),
            ui::row(BoxOptions {
                width_percent: Some(100.0),
                justify: flex::Justify::Center,
                children: vec![ui::vector(VectorOptions {
                    drawing,
                    ..Default::default()
                })],
                ..Default::default()
            }),
        ],
        ..Default::default()
    })
}
