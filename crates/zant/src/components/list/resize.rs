//! Item size reports: which height changes the layout already places right,
//! and which need the list to build again.
//!
//! Every built item of a list without a fixed `item_height` reports its size
//! (`on_resize`). A rebuild repositions the rows and fills the visible range,
//! but most changes need neither: rows are laid out one after the other, so
//! an item that grows moves only the rows after it, and while the list follows
//! its tail the rows are anchored to the bottom (`bottom_anchored`), so the
//! growing last item (a streaming reply) pushes the others up by itself. Those
//! reports record the height and skip the rebuild (`ActionCx::skip_rebuild`),
//! or leave it to the next frame when only the scroll bar thumb moved
//! (`ActionCx::defer_rebuild`): otherwise each report rebuilds the whole app
//! and lays it out again in the same frame.

use crate::ui::actions::Rebuild;

use super::ListState;

/// How the items were placed by the build that is being laid out.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Placement {
    /// The rows hang from the bottom of the viewport (following the tail).
    pub bottom_anchored: bool,
    /// A short list sits at the bottom (`Alignment::Bottom`).
    pub shifted: bool,
    /// The scroll bar is enabled.
    pub scrollbar: bool,
}

impl ListState {
    /// The scroll bar thumb `(y, height)` in the viewport, when the content
    /// overflows it.
    pub(super) fn thumb(&self) -> Option<(f32, f32)> {
        let viewport_height = self.viewport.height;
        let content_height = self.content_height();
        if content_height <= viewport_height + 0.5 {
            return None;
        }
        let track = viewport_height - 8.0;
        let thumb = (track * viewport_height / content_height).max(28.0);
        let max_top = (content_height - viewport_height).max(1.0);
        let thumb_y = 4.0 + (track - thumb) * (self.scroll_top() / max_top).clamp(0.0, 1.0);
        Some((thumb_y, thumb))
    }

    /// Item `index` was laid out `height` tall. Records the height and says
    /// when the list must build again: now for a first measurement (the
    /// visible range is still being filled), a shrinking item (rows that are
    /// not built may come into view), a change above the scroll anchor of a
    /// top-anchored list, a list that starts to overflow while following or a
    /// short bottom-aligned list; in the next frame when only the scroll bar
    /// moved; otherwise not at all.
    pub(super) fn item_resized(
        &mut self,
        index: usize,
        height: f32,
        placement: Placement,
    ) -> Rebuild {
        if self.uniform.is_some() || index >= self.count() {
            return Rebuild::Now;
        }
        let old = self.heights[index];
        if old < 0.0 || height < old - 0.25 || placement.shifted {
            return Rebuild::Now;
        }
        if height <= old + 0.25 {
            return Rebuild::Skip;
        }
        if !placement.bottom_anchored {
            // a top-anchored list keeps its rows in place from the anchor
            // down; following, it has to start scrolling once it overflows
            if index < self.anchor {
                return Rebuild::Now;
            }
            if self.following && self.content_height() - old + height > self.viewport.height {
                return Rebuild::Now;
            }
        }
        let thumb_before = if placement.scrollbar {
            self.thumb()
        } else {
            None
        };
        self.set_measured(index, height);
        if self.following {
            let max_top = self.max_scroll_top();
            self.set_scroll_top(max_top);
        }
        let thumb_after = if placement.scrollbar {
            self.thumb()
        } else {
            None
        };
        match (thumb_before, thumb_after) {
            (None, None) => Rebuild::Skip,
            (Some(a), Some(b)) if (a.0 - b.0).abs() < 0.5 && (a.1 - b.1).abs() < 0.5 => {
                Rebuild::Skip
            }
            // the thumb is drawn from the build: update it next frame
            (Some(_), Some(_)) => Rebuild::NextFrame,
            _ => Rebuild::Now,
        }
    }
}
