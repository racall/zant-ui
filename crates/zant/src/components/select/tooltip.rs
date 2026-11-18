//! Component-side helpers for the runtime's tooltip overlay.
//!
//! The layout style, the centred-below / flipped-above rectangle, the enter /
//! exit transitions and the paint. The real layer — the tooltip target
//! `NodeId`, the 450ms delay timer, keeping the layout alive through the
//! zoom-out, and calling this after the tree paint — lives in the runtime,
//! not here.

use crate::core::geometry::{Rect, Size};
use crate::motion::motion;
use crate::renderer::canvas::Canvas;
use crate::renderer::text::{self, TextLayout};

/// Hover delay before the tooltip appears.
pub const DELAY_MS: i64 = 450;
pub const FONT_SIZE: f32 = 12.0;
pub const PADDING_X: f32 = 8.0;
pub const PADDING_Y: f32 = 4.0;
/// Gap between the target and the tooltip box.
pub const GAP: f32 = 6.0;
/// Minimum distance to the window edges.
pub const MARGIN: f32 = 8.0;
pub const RADIUS: f32 = 6.0;
/// antd `colorBgSpotlight`.
pub const BACKGROUND: u32 = 0xd900_0000;
pub const TEXT_COLOR: u32 = 0xffff_ffff;
/// antd `zoom-big-fast`.
pub const PRESENCE: motion::Presence = motion::Presence::ZoomBigFast;

/// The text style `showTooltip` lays the content out with.
pub fn text_style() -> text::Style {
    text::Style {
        font_size: FONT_SIZE,
        color: TEXT_COLOR,
        wrap: false,
        ..Default::default()
    }
}

/// The tooltip box size for a laid-out content (`longest_line` + padding).
pub fn measure(layout: &TextLayout) -> (f32, f32) {
    (
        layout.metrics().longest_line + PADDING_X * 2.0,
        layout.metrics().height + PADDING_Y * 2.0,
    )
}

/// `showTooltip`'s geometry: centred below `bounds`, flipped above when it
/// would overflow the window bottom, clamped 8px from the left / right.
/// Returns `(rect, above)`, the runtime's `tooltip_rect` / `tooltip_above`.
pub fn rect(bounds: Rect, viewport: Size, layout: &TextLayout) -> (Rect, bool) {
    let (width, height) = measure(layout);
    let mut x = bounds.x + (bounds.width - width) * 0.5;
    let mut y = bounds.bottom() + GAP;
    x = x.clamp(MARGIN, (viewport.width - width - MARGIN).max(MARGIN));
    let mut above = false;
    if y + height > viewport.height - MARGIN {
        y = (bounds.y - height - GAP).max(MARGIN);
        above = true;
    }
    (Rect::init(x, y, width, height), above)
}

/// The enter transition `showTooltip` starts (`Animated::start(0, 1, …)`).
pub fn enter() -> motion::Transition {
    motion::presence_enter(PRESENCE)
}

/// The exit transition `clearTooltip` retargets to.
pub fn exit() -> motion::Transition {
    motion::presence_exit(PRESENCE)
}

/// `paintTooltip`: zoom / fade around the edge next to the target, then a
/// round spotlight box with the laid-out content.
pub fn paint(canvas: Canvas, layout: &TextLayout, rect: Rect, above: bool, anim_value: f32) {
    let fx = motion::presence_fx(PRESENCE, anim_value);
    if fx.opacity <= 0.003 {
        return;
    }
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
                rect.x - 4.0,
                rect.y - 4.0,
                rect.width + 8.0,
                rect.height + 8.0,
            ),
            fx.opacity,
        );
    }
    canvas.fill_round_rect(rect, RADIUS, BACKGROUND);
    layout.paint(canvas, rect.x + PADDING_X, rect.y + PADDING_Y);
    if layer {
        canvas.restore();
    }
    canvas.restore();
}
