//! Small toolbar buttons used by code blocks, tables and diagrams.

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::node::{ButtonSize, ButtonType};
use crate::markdown::render::context::Ctx;
use crate::ui;
use crate::ui::actions::Action;
use crate::ui::options::ButtonOptions;
use crate::ui::styled::Style;

/// Space between action buttons: labels mix CJK and Latin words, so a
/// word-sized gap would read as one sentence.
pub const GAP: f32 = 10.0;

pub fn button(
    ctx: &Ctx,
    key: &str,
    label: &str,
    action: Option<Action>,
    enabled: bool,
) -> ui::Node {
    ui::styled(
        ui::button(ButtonOptions {
            key: Some(Str::copy(key)),
            text: Str::copy(label),
            variant: ButtonType::Text,
            size: ButtonSize::Small,
            font_size: Some(12.0),
            disabled: !enabled,
            text_color: Some(ctx.options.theme.button_fg),
            on_click: action,
            ..Default::default()
        }),
        Style {
            height: Some(24.0),
            radius: Some(5.0),
            padding: Some(Edges {
                left: 8.0,
                right: 8.0,
                ..Edges::default()
            }),
            ..Default::default()
        },
    )
}
