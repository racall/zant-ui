//! Streamdown's link-safety dialog: shows the URL with Open / Copy / Cancel.
//! Render it as the last child of the window root while a link is pending.

use crate::core::geometry::Edges;
use crate::core::node::{ButtonSize, ButtonType};
use crate::layout::flex;
use crate::markdown::render::options::Labels;
use crate::markdown::theme;
use crate::motion::motion;
use crate::ui;
use crate::ui::actions::Action;
use crate::ui::options::{BoxOptions, ButtonOptions, ParagraphOptions, TextOptions};
use crate::ui::styled::Style;

#[derive(Clone, Default)]
pub struct Props {
    pub url: crate::Str,
    pub theme: theme::Theme,
    pub labels: Labels,
    pub on_open: Option<Action>,
    pub on_copy: Option<Action>,
    pub on_close: Option<Action>,
}

pub fn link_safety_modal(p: Props) -> ui::Node {
    let t = p.theme;
    let card = ui::column(BoxOptions {
        key: Some("md-link-modal-card".into()),
        width: Some(420.0),
        padding: Edges::all(22.0),
        gap: 12.0,
        radius: 12.0,
        background: Some(t.surface),
        border_color: Some(t.border),
        border_width: 1.0,
        align_items: flex::Align::Stretch,
        children: vec![
            ui::text(
                p.labels.link_title.clone(),
                TextOptions {
                    size: 16.0,
                    weight: crate::renderer::text::Weight::Semibold,
                    color: Some(t.foreground),
                    ..Default::default()
                },
            ),
            ui::paragraph(
                p.labels.link_message.clone(),
                ParagraphOptions {
                    size: 13.0,
                    color: Some(t.muted_foreground),
                    ..Default::default()
                },
            ),
            ui::styled(
                ui::paragraph(
                    p.url.clone(),
                    ParagraphOptions {
                        size: 12.0,
                        color: Some(t.foreground),
                        ..Default::default()
                    },
                ),
                Style {
                    background: Some(t.muted),
                    radius: Some(6.0),
                    padding: Some(Edges {
                        left: 10.0,
                        right: 10.0,
                        top: 8.0,
                        bottom: 8.0,
                    }),
                    ..Default::default()
                },
            ),
            ui::row(BoxOptions {
                gap: 8.0,
                justify: flex::Justify::End,
                width_percent: Some(100.0),
                children: vec![
                    ui::button(ButtonOptions {
                        key: Some("md-link-cancel".into()),
                        text: p.labels.cancel.clone(),
                        variant: ButtonType::Text,
                        size: ButtonSize::Small,
                        on_click: p.on_close,
                        ..Default::default()
                    }),
                    ui::button(ButtonOptions {
                        key: Some("md-link-copy".into()),
                        text: p.labels.copy_link.clone(),
                        size: ButtonSize::Small,
                        on_click: p.on_copy,
                        ..Default::default()
                    }),
                    ui::button(ButtonOptions {
                        key: Some("md-link-open".into()),
                        text: p.labels.open_link.clone(),
                        variant: ButtonType::Primary,
                        size: ButtonSize::Small,
                        on_click: p.on_open,
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            }),
        ],
        ..Default::default()
    });
    let card = ui::styled(
        card,
        Style {
            shadow_color: Some(0x33000000),
            shadow_blur: Some(18.0),
            shadow_offset_y: Some(8.0),
            motion: Some(motion::Presence::ZoomBig),
            ..Default::default()
        },
    );
    let overlay = ui::column(BoxOptions {
        key: Some("md-link-modal".into()),
        justify: flex::Justify::Center,
        align_items: flex::Align::Center,
        background: Some(t.overlay),
        block_pointer: true,
        children: vec![card],
        ..Default::default()
    });
    ui::styled(
        overlay,
        Style {
            absolute: Some(true),
            left: Some(0.0),
            top: Some(0.0),
            width_percent: Some(100.0),
            height_percent: Some(100.0),
            motion: Some(motion::Presence::Fade),
            ..Default::default()
        },
    )
}
