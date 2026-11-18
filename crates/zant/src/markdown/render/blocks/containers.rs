//! Container blocks: block quotes, callouts and lists.

use super::*;

pub(super) fn render_quote(ctx: &Ctx, blocks: &[ast::Block]) -> ui::Node {
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

pub(super) fn render_callout(ctx: &Ctx, callout: &ast::Callout) -> ui::Node {
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

pub(super) fn render_list(ctx: &Ctx, list: &ast::List, depth: usize) -> ui::Node {
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
