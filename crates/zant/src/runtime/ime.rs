//! Port of `src/ui_runtime.zig:2508-2582` + the IME helpers: composition
//! events, the IME candidate-window anchor and the small input-editing helpers
//! the pointer dispatch needs (click into a text field, caret blink).
//!
//! Zig called `components/input.zig` (`hasSelection`, `selectionRange`,
//! `textLeft`, `ensureLayout`, `updateBlink`); those component functions are
//! not ported yet (WP-2.1) so the few the runtime needs live here, marked with
//! the Zig source line. They should move to `components::input` when WP-2.1
//! lands.
#![forbid(unsafe_code)]

use crate::core::events::Composition;
use crate::core::geometry::Rect;
use crate::core::node::{InputData, NodeData};
use crate::renderer::text::{self, TextLayout};

use super::Runtime;

/// Zig `components/input.zig:padding_x`.
const INPUT_PADDING_X: f32 = 11.0;

/// Zig `components/input.zig:blink_interval_ms`.
pub(crate) const BLINK_INTERVAL_MS: i64 = 530;

impl Runtime {
    /// Zig `handleComposition`: feeds an IME composition update to the focused
    /// input; an empty text cancels it.
    pub(crate) fn handle_composition(&mut self, composition: &Composition) -> i32 {
        if let Some(id) = self.tree.focused {
            let is_input = matches!(&self.tree.get(id).data, NodeData::Input(_));
            if is_input {
                if composition.text.is_empty() {
                    self.tree.input_cancel_composition(id);
                } else {
                    self.tree.input_composition(
                        id,
                        &composition.text,
                        composition.selection_start,
                        composition.selection_end,
                    );
                }
                return 1;
            }
        }
        0
    }

    /// Zig `imeRect`: where the IME candidate window should point (the focused
    /// input's caret box).
    pub(crate) fn ime_rect(&mut self) -> Option<Rect> {
        let id = self.tree.focused?;
        let is_input = matches!(&self.tree.get(id).data, NodeData::Input(_));
        if !is_input {
            return None;
        }
        self.tree.input_caret_rect(id)
    }
}

/// `components/input.zig:hasSelection`.
pub(crate) fn has_selection(data: &InputData) -> bool {
    data.cursor != data.anchor
}

/// `components/input.zig:selectionRange`.
pub(crate) fn selection_range(data: &InputData) -> (usize, usize) {
    (data.cursor.min(data.anchor), data.cursor.max(data.anchor))
}

/// `components/input.zig:textLeft`: left edge of the text column.
pub(crate) fn input_text_left(data: &InputData, bounds: Rect) -> f32 {
    let full = (bounds.width - INPUT_PADDING_X * 2.0).max(0.0);
    if data.content_max_width > 0.0 && full > data.content_max_width {
        return bounds.x + (bounds.width - data.content_max_width) * 0.5;
    }
    bounds.x + INPUT_PADDING_X
}

/// `components/input.zig:ensureLayout`: the shaped text layout of the input
/// (invalidated by edits).
pub(crate) fn ensure_input_layout(data: &mut InputData) -> Option<&TextLayout> {
    if !data.layout_dirty && data.layout.is_some() {
        return data.layout.as_ref();
    }
    let style = text::Style {
        font_size: data.font_size,
        weight: text::Weight::Regular,
        color: data.text_color,
        wrap: data.multiline,
        text_align: text::Align::Left,
        line_height: data.line_height,
        ..text::Style::default()
    };
    let max_width = if data.multiline {
        data.available_width.max(1.0)
    } else {
        1.0e6
    };
    let layout = TextLayout::init(&data.display_text, style, max_width).ok()?;
    data.layout = Some(layout);
    data.layout_dirty = false;
    data.layout.as_ref()
}

/// `components/input.zig:maxScrollY`: the largest vertical scroll offset of a
/// multiline input.
pub(crate) fn input_max_scroll_y(data: &mut InputData, view_height: f32) -> f32 {
    let Some(layout) = ensure_input_layout(data) else {
        return 0.0;
    };
    (layout.metrics().height - view_height.max(1.0)).max(0.0)
}

/// `components/input.zig:updateBlink`: the caret blink; true when the blink
/// state flipped and the input must repaint.
pub(crate) fn update_blink(data: &mut InputData, enabled: bool, now_ms: i64) -> bool {
    if !data.focused || !enabled {
        data.blink_on = true;
        data.next_blink_ms = 0;
        return false;
    }
    if data.next_blink_ms == 0 {
        data.blink_on = true;
        data.next_blink_ms = now_ms + BLINK_INTERVAL_MS;
        return false;
    }
    if now_ms >= data.next_blink_ms {
        data.blink_on = !data.blink_on;
        data.next_blink_ms = now_ms + BLINK_INTERVAL_MS;
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::node::NodeData;

    #[test]
    fn caret_blink_flips_every_interval_only_when_focused() {
        let mut data = InputData::default();
        assert!(!update_blink(&mut data, true, 1000), "not focused");
        assert!(data.blink_on);
        data.focused = true;
        assert!(
            !update_blink(&mut data, true, 1000),
            "the first call arms it"
        );
        assert_eq!(data.next_blink_ms, 1000 + BLINK_INTERVAL_MS);
        assert!(update_blink(&mut data, true, 1000 + BLINK_INTERVAL_MS));
        assert!(!data.blink_on);
        assert_eq!(data.next_blink_ms, 1000 + 2 * BLINK_INTERVAL_MS);
        // unfocusing resets the caret to visible
        data.focused = false;
        assert!(!update_blink(&mut data, true, 5000));
        assert!(data.blink_on);
        assert_eq!(data.next_blink_ms, 0);
    }

    #[test]
    fn selection_helpers_match_the_zig_component() {
        let mut data = InputData::default();
        assert!(!has_selection(&data));
        assert_eq!(selection_range(&data), (0, 0));
        data.anchor = 5;
        data.cursor = 2;
        assert!(has_selection(&data));
        assert_eq!(selection_range(&data), (2, 5));
        let bounds = Rect::init(10.0, 20.0, 200.0, 32.0);
        assert_eq!(input_text_left(&data, bounds), 21.0);
        data.content_max_width = 100.0;
        assert_eq!(input_text_left(&data, bounds), 60.0);
        // the column is narrower than content_max_width: keep the padding
        assert_eq!(
            input_text_left(&data, Rect::init(0.0, 0.0, 50.0, 10.0)),
            11.0,
        );
        // the layout is cached and survives until invalidated
        data.text = String::from("hello");
        data.rebuild_display();
        let width = ensure_input_layout(&mut data)
            .map(|layout| layout.metrics().longest_line)
            .expect("layout");
        assert!(width > 0.0);
        assert!(!data.layout_dirty);
        let _ = NodeData::Input(InputData::default());
    }
}
