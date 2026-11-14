//! Port of `src/ui_runtime.zig:2545-2582`: IME composition events and the IME
//! candidate-window anchor.
//!
//! The input-editing helpers the runtime needs (`hasSelection`,
//! `selectionRange`, `maxScrollY`, `updateBlink`, `textLeft`, `ensureLayout`)
//! live in [`crate::components::input`] (WP-2.1); this module re-exports the
//! ones other runtime modules call under their runtime-side names, so the
//! three temporary copies that existed while the component was a stub are gone.
#![forbid(unsafe_code)]

use crate::core::events::Composition;
use crate::core::geometry::Rect;
use crate::core::node::NodeData;

use super::Runtime;

pub(crate) use crate::components::input::{
    has_selection, max_scroll_y as input_max_scroll_y, selection_range, update_blink,
};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::input;
    use crate::core::node::InputData;

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
        assert_eq!(data.next_blink_ms, 1000 + input::BLINK_INTERVAL_MS);
        assert!(update_blink(
            &mut data,
            true,
            1000 + input::BLINK_INTERVAL_MS
        ));
        assert!(!data.blink_on);
        assert_eq!(data.next_blink_ms, 1000 + 2 * input::BLINK_INTERVAL_MS);
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
        assert_eq!(input::text_left(&data, bounds), 21.0);
        data.content_max_width = 100.0;
        assert_eq!(input::text_left(&data, bounds), 60.0);
        // the column is narrower than content_max_width: keep the padding
        assert_eq!(
            input::text_left(&data, Rect::init(0.0, 0.0, 50.0, 10.0)),
            11.0,
        );
        // the layout is cached and survives until invalidated
        data.text = String::from("hello");
        data.rebuild_display();
        assert!(input::ensure_layout(&mut data));
        let width = data
            .layout
            .as_ref()
            .map(|layout| layout.metrics().longest_line)
            .expect("layout");
        assert!(width > 0.0);
        assert!(!data.layout_dirty);
    }
}
