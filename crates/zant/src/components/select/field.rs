//! Closed field measurements, cached label and option layouts, and field painting.

use crate::core::geometry::Rect;
use crate::core::node::{FlexStyle, NodeData, RetainedNode, SelectData};
use crate::renderer::canvas::Canvas;
use crate::renderer::text::{self, TextLayout};
use crate::theme::tokens;
use crate::theme::tokens::Theme;

/// Height of the closed field (the antd middle control height).
pub const FIELD_HEIGHT: f32 = 32.0;
/// Horizontal padding of the field.
pub const PADDING_X: f32 = 11.0;

/// The antd control height of the closed field.
pub fn control_height() -> f32 {
    FIELD_HEIGHT
}

/// Horizontal padding between the field edge and the selected label.
pub fn horizontal_padding() -> f32 {
    PADDING_X
}

/// Label font size (`SelectData::font_size`).
pub fn font_size(data: &SelectData) -> f32 {
    data.font_size
}

/// The styled width, or the default 220px.
pub fn measure_width(style: &FlexStyle) -> f32 {
    style.width.unwrap_or(220.0)
}

/// The styled height, or the field height.
pub fn measure_height(style: &FlexStyle) -> f32 {
    style.height.unwrap_or(FIELD_HEIGHT)
}

/// The option at `selected`, or the "Select…" placeholder.
pub fn selected_label(data: &SelectData) -> &str {
    if data.selected < data.options.len() {
        data.options[data.selected].label.as_str()
    } else {
        "Select…"
    }
}

/// Ensures the selected label's layout and one layout per option (the popup
/// paints without re-shaping). Rebuilds per colour; the label and the option
/// set are keyed by colour / count.
pub fn ensure_layout(data: &mut SelectData, color: u32, option_color: u32) -> Option<&TextLayout> {
    let label_ok = !data.layout_dirty && data.layout.is_some() && data.layout_color == color;
    let options_ok =
        data.option_layouts.len() == data.options.len() && data.option_color == option_color;
    if label_ok && options_ok {
        return data.layout.as_ref();
    }
    if !label_ok {
        data.layout = None;
        let style = text::Style {
            font_size: data.font_size,
            color,
            wrap: false,
            ..Default::default()
        };
        match TextLayout::init(selected_label(data), style, 1.0e6) {
            Ok(layout) => data.layout = Some(layout),
            Err(_) => return None,
        }
        data.layout_color = color;
    }
    if !options_ok {
        data.option_color = option_color;
        data.option_layouts.clear();
        for index in 0..data.options.len() {
            let label = data.options[index].label.clone();
            let style = text::Style {
                font_size: data.font_size,
                color: option_color,
                wrap: false,
                ..Default::default()
            };
            match TextLayout::init(&label, style, 1.0e6) {
                Ok(layout) => data.option_layouts.push(layout),
                Err(_) => return None,
            }
        }
    }
    data.layout_dirty = false;
    data.layout.as_ref()
}

pub fn paint(node: &mut RetainedNode, canvas: Canvas, theme: Theme) {
    let rect = node.bounds;
    let enabled = node.enabled;
    let visual = node.visual;
    let data = match &mut node.data {
        NodeData::Select(data) => data,
        _ => return,
    };
    let hover_t = if enabled { data.hover_t } else { 0.0 };
    let focus_t = if enabled { data.focus_t } else { 0.0 };
    let default_border = if !enabled {
        theme.color_border
    } else {
        tokens::lerp(
            tokens::lerp(theme.color_border, theme.color_primary_hover, hover_t),
            theme.color_primary,
            focus_t,
        )
    };
    let border = visual.border_color.unwrap_or(default_border);
    let bg = visual.background.unwrap_or(if enabled {
        theme.color_bg_container
    } else {
        theme.color_fill_secondary
    });
    if focus_t > 0.01 {
        canvas.fill_round_rect(
            Rect::init(
                rect.x - 3.0,
                rect.y - 3.0,
                rect.width + 6.0,
                rect.height + 6.0,
            ),
            visual.radius + 3.0,
            tokens::with_alpha(theme.color_primary, (0x18 as f32 * focus_t) as u8),
        );
    }
    canvas.fill_round_rect(rect, visual.radius, bg);
    canvas.stroke_round_rect(
        Rect::init(
            rect.x + 0.5,
            rect.y + 0.5,
            (rect.width - 1.0).max(0.0),
            (rect.height - 1.0).max(0.0),
        ),
        visual.radius,
        border,
        1.0 + 0.5 * focus_t,
    );
    let color = if enabled {
        theme.color_text
    } else {
        theme.color_text_disabled
    };
    if let Some(layout) = ensure_layout(data, color, theme.color_text) {
        layout.paint(
            canvas,
            rect.x + PADDING_X,
            rect.y + (rect.height - layout.metrics().height) * 0.5,
        );
    }
    let cx = rect.right() - 14.0;
    let cy = rect.y + rect.height * 0.5;
    let dir: f32 = if data.open { -1.0 } else { 1.0 };
    canvas.draw_line(
        cx - 4.0,
        cy - 2.0 * dir,
        cx,
        cy + 2.0 * dir,
        theme.color_text_secondary,
        1.2,
    );
    canvas.draw_line(
        cx,
        cy + 2.0 * dir,
        cx + 4.0,
        cy - 2.0 * dir,
        theme.color_text_secondary,
        1.2,
    );
}
