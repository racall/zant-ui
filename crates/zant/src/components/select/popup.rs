//! Dropdown geometry, option hit testing, and overlay painting.

use crate::core::geometry::{Rect, Size};
use crate::core::node::{NodeData, RetainedNode, SelectData};
use crate::motion::motion;
use crate::renderer::canvas::Canvas;
use crate::theme::tokens::Theme;

/// Height of one option row in the popup.
pub const ROW_HEIGHT: f32 = 30.0;
/// Padding between the popup edge and its rows.
pub const POPUP_PADDING: f32 = 4.0;

fn select_data(node: &RetainedNode) -> Option<&SelectData> {
    match &node.data {
        NodeData::Select(data) => Some(data),
        _ => None,
    }
}

/// Dropdown size / position below the field, flipped above
/// near the window bottom, clamped to 8px margins.
pub fn popup_rect(node: &RetainedNode, viewport: Size) -> Rect {
    let Some(data) = select_data(node) else {
        return Rect::default();
    };
    let height = data.options.len() as f32 * ROW_HEIGHT + POPUP_PADDING * 2.0;
    let mut rect = Rect::init(
        node.bounds.x,
        node.bounds.bottom() + 4.0,
        node.bounds.width,
        height,
    );
    if rect.bottom() > viewport.height - 8.0 {
        rect.y = node.bounds.y - rect.height - 4.0;
    }
    if rect.y < 8.0 {
        rect.y = 8.0;
    }
    rect
}

/// The option index under a window-space point, if any.
pub fn option_at(node: &RetainedNode, x: f32, y: f32, viewport: Size) -> Option<usize> {
    let data = select_data(node)?;
    let rect = popup_rect(node, viewport);
    if !rect.contains(x, y) {
        return None;
    }
    let local = y - rect.y - POPUP_PADDING;
    if local < 0.0 {
        return None;
    }
    let index = (local / ROW_HEIGHT).floor() as usize;
    if index >= data.options.len() {
        return None;
    }
    Some(index)
}

/// Paints the antd slide-up dropdown, painted by the runtime above
/// the tree (it is not a tree node). Kept here so the transition and the row
/// layout stay next to the closed control's paint.
pub fn paint_popup(node: &RetainedNode, canvas: Canvas, theme: Theme, viewport: Size) {
    let Some(data) = select_data(node) else {
        return;
    };
    if data.options.is_empty() {
        return;
    }
    // antd slide-up: grows from the edge next to the field while fading in
    let fx = motion::presence_fx(motion::Presence::SlideUp, data.open_t);
    if fx.opacity <= 0.003 {
        return;
    }
    let rect = popup_rect(node, viewport);
    let above = rect.y < node.bounds.y;
    let origin = motion::origin_point(
        if above {
            motion::Origin::Bottom
        } else {
            motion::Origin::Top
        },
        rect.x,
        rect.y,
        rect.width,
        rect.height,
    );
    canvas.save();
    canvas.translate(origin[0], origin[1]);
    canvas.scale(fx.scale_x, fx.scale_y);
    canvas.translate(-origin[0], -origin[1]);
    let layer = fx.opacity < 0.997;
    if layer {
        canvas.save_layer_alpha(
            Rect::init(
                rect.x - 24.0,
                rect.y - 24.0,
                rect.width + 48.0,
                rect.height + 48.0,
            ),
            fx.opacity,
        );
    }
    let mut shadow = rect;
    shadow.y += 6.0;
    canvas.shadow_round_rect(shadow, 8.0, 0x1f00_0000, 16.0);
    canvas.fill_round_rect(rect, 8.0, theme.color_bg_elevated);
    canvas.stroke_round_rect(rect, 8.0, theme.color_border_secondary, 1.0);
    canvas.save();
    canvas.clip_rect(rect);
    for i in 0..data.options.len() {
        let row = Rect::init(
            rect.x + POPUP_PADDING,
            rect.y + POPUP_PADDING + i as f32 * ROW_HEIGHT,
            rect.width - POPUP_PADDING * 2.0,
            ROW_HEIGHT,
        );
        if data.hovered_option == Some(i) {
            canvas.fill_round_rect(row, 4.0, theme.color_fill_secondary);
        }
        if i == data.selected {
            canvas.fill_round_rect(
                Rect::init(row.x, row.y + 7.0, 2.0, row.height - 14.0),
                1.0,
                theme.color_primary,
            );
        }
        if i < data.option_layouts.len() {
            let layout = &data.option_layouts[i];
            layout.paint(
                canvas,
                row.x + 8.0,
                row.y + (row.height - layout.metrics().height) * 0.5,
            );
        }
    }
    canvas.restore();
    if layer {
        canvas.restore();
    }
    canvas.restore();
}
