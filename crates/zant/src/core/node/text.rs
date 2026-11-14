//! Port of `src/core/node.zig` — WP-1.3: `TextData` and its layout caches.
//!
//! A text node keeps **both** the layout at the current width and one at the
//! previous width (`alt_layout`), plus a separate single-width natural-size
//! cache (`measure_width`). Flex layout measures a node at a trial width before
//! its final one; keeping both layouts stops them from evicting each other
//! (that re-shaped every paragraph every frame — AGENTS.md performance rules).
//! Never re-shape in the measure path: `measure_width` builds a throwaway
//! layout only when neither cache matches.
#![forbid(unsafe_code)]

use crate::Str;
use crate::renderer::text::{self, TextLayout};

/// Tolerance under which two widths count as the same cached width (Zig uses
/// 0.25 in all three caches).
const WIDTH_EPSILON: f32 = 0.25;

/// Zig `TextData`.
pub struct TextData {
    pub content: Str,
    pub style: text::Style,
    pub layout: Option<TextLayout>,
    pub layout_width: f32,
    pub layout_dirty: bool,
    /// Intrinsic width measured at `measure_avail` (layout's min/max-content
    /// pass), kept apart from `layout` so measuring never discards the
    /// laid-out paragraph.
    pub measure_avail: f32,
    pub measure_width: f32,
    /// The layout at the previous width (see the module docs).
    pub alt_layout: Option<TextLayout>,
    pub alt_width: f32,
}

impl Default for TextData {
    fn default() -> Self {
        TextData {
            content: Str::default(),
            style: text::Style::default(),
            layout: None,
            layout_width: -1.0,
            layout_dirty: true,
            measure_avail: -1.0,
            measure_width: 0.0,
            alt_layout: None,
            alt_width: -1.0,
        }
    }
}

impl TextData {
    /// Width to lay the text out at inside a box `box_width` wide.
    ///
    /// Wrapping text uses the box; single-line text with `max_lines` too, so it
    /// ends in "…" when squeezed (1px slack avoids rounding into an ellipsis);
    /// other single-line text keeps its natural width.
    pub fn layout_width(&self, box_width: f32) -> f32 {
        if self.style.wrap {
            return box_width;
        }
        if self.style.max_lines > 0 {
            return box_width + 1.0;
        }
        1.0e6
    }

    pub fn invalidate(&mut self) {
        self.layout_dirty = true;
        self.measure_avail = -1.0;
        self.layout = None;
        self.layout_width = -1.0;
        self.alt_layout = None;
        self.alt_width = -1.0;
    }

    /// The paragraph laid out `max_width` wide, reusing / re-wrapping the
    /// cached layouts when possible.
    pub fn ensure_layout(&mut self, max_width: f32) -> Option<&TextLayout> {
        if !self.layout_dirty
            && self.layout.is_some()
            && (self.layout_width - max_width).abs() < WIDTH_EPSILON
        {
            return self.layout.as_ref();
        }
        if !self.layout_dirty && self.alt_layout.is_some() {
            // the other cached width, or re-wrap it to this one (no shaping)
            if (self.alt_width - max_width).abs() >= WIDTH_EPSILON {
                if let Some(alt) = self.alt_layout.as_mut() {
                    alt.relayout(max_width);
                }
                self.alt_width = max_width;
            }
            std::mem::swap(&mut self.layout, &mut self.alt_layout);
            std::mem::swap(&mut self.layout_width, &mut self.alt_width);
            return self.layout.as_ref();
        }
        let fresh = TextLayout::init(&self.content, self.style, max_width).ok();
        let fresh = fresh?;
        self.alt_layout = None;
        if self.layout_dirty {
            self.layout = None;
            self.alt_width = -1.0;
        } else {
            self.alt_layout = self.layout.take();
            self.alt_width = self.layout_width;
        }
        self.layout = Some(fresh);
        self.layout_width = max_width;
        self.layout_dirty = false;
        self.layout.as_ref()
    }

    /// Longest line when laid out `max_width` wide, cached per width.
    pub fn measure_width(&mut self, max_width: f32) -> f32 {
        if (self.measure_avail - max_width).abs() < WIDTH_EPSILON {
            return self.measure_width;
        }
        let w = if let (Some(layout), false) = (self.layout.as_ref(), self.layout_dirty) {
            if (self.layout_width - max_width).abs() < WIDTH_EPSILON {
                layout.metrics().longest_line
            } else {
                self.measure_fresh(max_width)
            }
        } else {
            self.measure_fresh(max_width)
        };
        // Round up: a paragraph laid out exactly as wide as its longest line
        // can wrap that line's last word (float error between measuring and
        // breaking), so "Terms" became "Term / s". Flutter rounds intrinsic
        // widths up for the same reason; the small margin covers a width that
        // is already a whole number.
        let rounded = (w + 0.01).ceil();
        self.measure_avail = max_width;
        self.measure_width = rounded;
        rounded
    }

    fn measure_fresh(&self, max_width: f32) -> f32 {
        match TextLayout::init(&self.content, self.style, max_width) {
            Ok(layout) => layout.metrics().longest_line,
            Err(_) => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::text;

    fn text_data(content: &'static str, style: text::Style) -> TextData {
        TextData {
            content: Str::from(content),
            style,
            ..Default::default()
        }
    }

    #[test]
    fn layout_width_depends_on_wrap_and_max_lines() {
        let mut data = text_data(
            "x",
            text::Style {
                wrap: true,
                ..Default::default()
            },
        );
        assert_eq!(data.layout_width(123.0), 123.0);
        data.style.wrap = false;
        data.style.max_lines = 0;
        assert_eq!(data.layout_width(123.0), 1.0e6);
        data.style.max_lines = 1;
        assert_eq!(data.layout_width(123.0), 124.0, "1px slack");
    }

    #[test]
    fn layouts_are_cached_per_width_and_survive_a_second_width() {
        let mut data = text_data("one two three four five six seven", text::Style::default());
        let first_width = data
            .ensure_layout(200.0)
            .map(|layout| layout.metrics().height)
            .expect("layout");
        assert_eq!(data.layout_width, 200.0);
        assert!(!data.layout_dirty);
        // same width: the same cached layout is returned
        assert_eq!(
            data.ensure_layout(200.0)
                .map(|layout| layout.metrics().height),
            Some(first_width)
        );
        // another width is wrapped into the alternate slot, not re-shaped
        let narrow = data
            .ensure_layout(60.0)
            .map(|layout| layout.metrics().height)
            .expect("layout");
        assert!(narrow > first_width);
        assert_eq!(data.layout_width, 60.0);
        assert_eq!(data.alt_width, 200.0);
        // back to the first width: swapped, still no shaping
        assert_eq!(
            data.ensure_layout(200.0)
                .map(|layout| layout.metrics().height),
            Some(first_width)
        );
        assert_eq!(data.layout_width, 200.0);
    }

    #[test]
    fn invalidate_drops_the_caches() {
        let mut data = text_data("hello", text::Style::default());
        assert!(data.ensure_layout(100.0).is_some());
        data.invalidate();
        assert!(data.layout.is_none());
        assert!(data.alt_layout.is_none());
        assert!(data.layout_dirty);
        assert_eq!(data.layout_width, -1.0);
        assert_eq!(data.alt_width, -1.0);
        assert_eq!(data.measure_avail, -1.0);
    }

    #[test]
    fn measure_width_is_cached_and_rounded_up() {
        let mut data = text_data(
            "Terms",
            text::Style {
                wrap: false,
                ..Default::default()
            },
        );
        let measured = data.measure_width(1.0e6);
        assert!(measured >= 1.0);
        // a whole number: ceil(w + 0.01) still rounds up
        assert_eq!(measured, measured.ceil());
        let (avail, width) = (data.measure_avail, data.measure_width);
        assert_eq!(data.measure_width(1.0e6), width);
        assert_eq!((data.measure_avail, data.measure_width), (avail, width));
    }
}
