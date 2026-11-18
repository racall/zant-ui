//! The antd text input: measure, editing operations, caret and paint.
//!
//! The input is an antd-style text field: rounded shell with a border that
//! eases to `primary-hover` on hover and `primary` on focus (plus the soft
//! focus ring), the shaped value / placeholder, the selection highlight, the
//! caret and — for multiline inputs — the composition underline and a scroll
//! indicator. `Animated` blur/focus transitions of the shell are sampled by
//! [`animate`], the caret blink by [`update_blink`].
//!
//! The editing operations live here too (`insert_text`, `delete_backward`,
//! `move_*`, `begin_composition`, …). They are the single implementation:
//! `core/tree/input_ops.rs` (the tree-level state changes) and
//! `runtime/ime.rs` (the runtime helpers) delegate to the functions below.
//!
//! `bounds` / `enabled` are passed as values (`Rect`, `bool`) like the
//! neighbouring component modules, and `measure_width` / `measure_height`
//! take `&FlexStyle` like `components::{button,select,toggle}`.
#![forbid(unsafe_code)]

use crate::core::geometry::Rect;
use crate::core::node::{FlexStyle, InputData, NodeData, RetainedNode};
use crate::core::unicode;
use crate::motion::motion;
use crate::renderer::canvas::Canvas;
use crate::renderer::text::{self, TextLayout};
use crate::theme::tokens;
use crate::theme::tokens::Theme;
use zant_sys::skia::ZantRect;

mod caret;
mod editing;
mod painting;
#[cfg(test)]
mod tests;

pub use self::caret::*;
pub use self::editing::*;
pub use self::painting::*;

/// The antd middle control width.
pub const DEFAULT_WIDTH: f32 = 220.0;

/// The antd middle control height.
pub const DEFAULT_HEIGHT: f32 = 32.0;

/// Horizontal padding between the shell and the text column.
pub const PADDING_X: f32 = 11.0;

/// Half period of the caret blink.
pub const BLINK_INTERVAL_MS: i64 = 530;

/// The editing command of `input_move`, defined next to the tree-level
/// editing operations in `core::tree`.
pub use crate::core::tree::Move;

/// The default height of an input.
pub fn control_height() -> f32 {
    DEFAULT_HEIGHT
}

/// Left edge of the text column (centred when
/// `content_max_width` is set).
pub fn text_left(data: &InputData, bounds: Rect) -> f32 {
    let full = (bounds.width - PADDING_X * 2.0).max(0.0);
    if data.content_max_width > 0.0 && full > data.content_max_width {
        return bounds.x + (bounds.width - data.content_max_width) * 0.5;
    }
    bounds.x + PADDING_X
}

/// Width of the text column.
pub fn text_width(data: &InputData, bounds: Rect) -> f32 {
    let full = (bounds.width - PADDING_X * 2.0).max(0.0);
    if data.content_max_width > 0.0 {
        return full.min(data.content_max_width);
    }
    full
}

/// The styled width, or the default control width.
pub fn measure_width(style: &FlexStyle) -> f32 {
    style.width.unwrap_or(DEFAULT_WIDTH)
}

/// The styled height, or the default control height.
pub fn measure_height(style: &FlexStyle) -> f32 {
    style.height.unwrap_or(DEFAULT_HEIGHT)
}

/// The selection as an ordered `(start, end)` byte range.
pub fn selection_range(data: &InputData) -> (usize, usize) {
    (data.cursor.min(data.anchor), data.cursor.max(data.anchor))
}

/// Whether a non-empty range is selected.
pub fn has_selection(data: &InputData) -> bool {
    data.cursor != data.anchor
}

/// Rebuilds the displayed text (value plus composition) from the editing state.
pub fn rebuild_display(data: &mut InputData) {
    data.rebuild_display();
}

/// Shapes `display_text` (cached until the next
/// invalidation). Returns false when shaping fails.
pub fn ensure_layout(data: &mut InputData) -> bool {
    if !data.layout_dirty && data.layout.is_some() {
        return true;
    }
    data.layout = None;
    let style = text::Style {
        font_size: data.font_size,
        weight: text::Weight::Regular,
        color: data.text_color,
        wrap: data.multiline,
        text_align: text::Align::Left,
        line_height: data.line_height,
        ..Default::default()
    };
    let max_width = if data.multiline {
        data.available_width.max(1.0)
    } else {
        1.0e6
    };
    match TextLayout::init(&data.display_text, style, max_width) {
        Ok(layout) => {
            data.layout = Some(layout);
            data.layout_dirty = false;
            true
        }
        Err(_) => false,
    }
}

/// Shapes the placeholder once and returns its cached layout.
fn ensure_placeholder(data: &mut InputData, theme: Theme) -> Option<&TextLayout> {
    if data.placeholder_layout.is_none() {
        let style = text::Style {
            font_size: data.font_size,
            weight: text::Weight::Regular,
            color: theme.color_text_disabled,
            wrap: false,
            line_height: data.line_height,
            ..Default::default()
        };
        match TextLayout::init(&data.placeholder, style, 1.0e6) {
            Ok(layout) => data.placeholder_layout = Some(layout),
            Err(_) => return None,
        }
    }
    data.placeholder_layout.as_ref()
}
