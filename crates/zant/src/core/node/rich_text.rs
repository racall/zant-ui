//! `RichTextData` and its layout caches.
//!
//! Same current + alternate width caching as [`TextData`](super::text::TextData),
//! with a separate unwrapped-width cache (`natural_width`) used by the layout's
//! max-content pass. Inline embeds (inline math) are placeholder boxes in the
//! paragraph; they are owned through `Rc<Drawing>`, so spans can simply be
//! cloned without a deep copy.
#![forbid(unsafe_code)]

use crate::core::node::data::RichSpan;
use crate::core::vector;
use crate::renderer::canvas::Canvas;
use crate::renderer::rich_text;
use crate::ui::actions::TextAction;

const WIDTH_EPSILON: f32 = 0.25;

/// A rich text node: styled spans plus their paragraph layout caches.
pub struct RichTextData {
    pub spans: Vec<RichSpan>,
    pub line_height: f32,
    pub link_action: Option<TextAction>,
    pub layout: Option<rich_text::Layout>,
    pub layout_width: f32,
    pub layout_dirty: bool,
    /// Unwrapped width (layout's max-content pass), cached apart from
    /// `layout` (see `TextData::measure_width`).
    pub natural_width: Option<f32>,
    /// Layout at the previous width (see `TextData::alt_layout`).
    pub alt_layout: Option<rich_text::Layout>,
    pub alt_width: f32,
}

impl Default for RichTextData {
    fn default() -> Self {
        RichTextData {
            spans: Vec::new(),
            line_height: 2.0,
            link_action: None,
            layout: None,
            layout_width: -1.0,
            layout_dirty: true,
            natural_width: None,
            alt_layout: None,
            alt_width: -1.0,
        }
    }
}

impl RichTextData {
    pub fn invalidate_layout(&mut self) {
        self.layout_dirty = true;
        self.natural_width = None;
        self.layout = None;
        self.layout_width = -1.0;
        self.alt_layout = None;
        self.alt_width = -1.0;
    }

    pub fn ensure_layout(&mut self, max_width: f32) -> Option<&rich_text::Layout> {
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
        let fresh = self.create_layout(max_width).ok()?;
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

    /// Width of the paragraph without wrapping (cached).
    pub fn natural_width(&mut self) -> f32 {
        if let Some(width) = self.natural_width {
            return width;
        }
        let w = if let Some(layout) = self.layout.as_ref() {
            if !self.layout_dirty && self.layout_width >= 1.0e5 {
                layout.width
            } else {
                self.temporary_width()
            }
        } else {
            self.temporary_width()
        };
        self.natural_width = Some(w);
        w
    }

    fn temporary_width(&self) -> f32 {
        match self.create_layout(1.0e6) {
            Ok(layout) => layout.width,
            Err(_) => 0.0,
        }
    }

    fn create_layout(&self, max_width: f32) -> Result<rich_text::Layout, rich_text::RichTextError> {
        let native: Vec<rich_text::Span<'_>> = self
            .spans
            .iter()
            .map(|span| rich_text::Span {
                text: &span.text,
                font_size: span.font_size,
                weight: span.weight as i32,
                color: span.color,
                italic: span.italic,
                family: span.family as i32,
                background: span.background,
                decoration: i32::from(span.underline) | (i32::from(span.strike) << 1),
                placeholder_width: span.embed.as_ref().map_or(0.0, |e| e.width.max(0.01)),
                placeholder_ascent: span.embed.as_ref().map_or(0.0, |e| e.ascent),
                placeholder_descent: span.embed.as_ref().map_or(0.0, |e| e.descent),
            })
            .collect();
        rich_text::Layout::create(&native, max_width, self.line_height)
    }

    pub fn has_embeds(&self) -> bool {
        self.spans.iter().any(|span| span.embed.is_some())
    }

    /// Paints the paragraph and its inline embeds at (x, y), laid out
    /// `max_width` wide.
    pub fn paint_at(&mut self, canvas: Canvas, x: f32, y: f32, max_width: f32) {
        if self.ensure_layout(max_width).is_none() {
            return;
        }
        let mut rects_buf = [crate::core::geometry::Rect::default(); 128];
        let count = {
            let Some(layout) = self.layout.as_ref() else {
                return;
            };
            layout.paint(canvas, x, y);
            if !self.has_embeds() {
                return;
            }
            layout.placeholder_rects(&mut rects_buf).len()
        };
        // Collect first: the embed drawings are `Rc` and the layout borrow of
        // `self` has to end before `vector::paint` (which may read any span).
        let mut placements: Vec<(f32, f32, crate::core::vector::DrawingRef)> = Vec::new();
        let mut index = 0usize;
        for span in &self.spans {
            let Some(embed) = span.embed.as_ref() else {
                continue;
            };
            if index >= count {
                break;
            }
            let r = rects_buf[index];
            index += 1;
            // the placeholder box's top is `ascent` above its baseline
            placements.push((x + r.x, y + r.y + embed.ascent, embed.clone()));
        }
        for (embed_x, embed_y, embed) in placements {
            vector::paint(canvas, &embed, embed_x, embed_y);
        }
    }

    pub fn has_links(&self) -> bool {
        self.spans.iter().any(|span| !span.link.is_empty())
    }

    /// Returns the link of the span under a byte offset of the joined text.
    pub fn link_at_byte(&self, byte: usize) -> Option<&str> {
        let mut start = 0usize;
        for span in &self.spans {
            let end = start + span.text.len();
            if byte >= start && byte < end {
                return if !span.link.is_empty() {
                    Some(&span.link)
                } else {
                    None
                };
            }
            start = end;
        }
        None
    }

    /// The joined text of all spans (as laid out).
    pub fn joined_text(&self) -> String {
        let mut out = String::new();
        for span in &self.spans {
            out.push_str(&span.text);
        }
        out
    }

    pub fn text_len(&self) -> usize {
        self.spans.iter().map(|span| span.text.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Str;
    use crate::core::node::data::RichSpan;
    use crate::renderer::text;

    fn spans() -> Vec<RichSpan> {
        vec![
            RichSpan {
                text: Str::from("Hello "),
                font_size: 15.0,
                color: 0xff29_2d2a,
                ..Default::default()
            },
            RichSpan {
                text: Str::from("world this is a long richer text run"),
                font_size: 15.0,
                weight: text::Weight::Semibold,
                color: 0xff60_7d6b,
                ..Default::default()
            },
        ]
    }

    #[test]
    fn natural_width_and_layout_caches_are_consistent() {
        let mut data = RichTextData {
            spans: spans(),
            ..Default::default()
        };
        let natural = data.natural_width();
        assert!(natural > 0.0);
        assert_eq!(data.natural_width(), natural, "cached");
        let narrow = data
            .ensure_layout(60.0)
            .map(|layout| layout.height)
            .expect("layout");
        assert!(narrow > 20.0);
        // back at a wide width: the cached natural layout is reused or re-wrapped
        let wide = data
            .ensure_layout(1000.0)
            .map(|layout| layout.height)
            .expect("layout");
        assert!(wide <= narrow);
    }

    #[test]
    fn links_and_text_helpers_match_the_spans() {
        let mut data = RichTextData {
            spans: spans(),
            ..Default::default()
        };
        data.spans[1].link = Str::from("https://example.com");
        assert!(data.has_links());
        assert_eq!(data.link_at_byte(0), None);
        assert_eq!(data.link_at_byte(6), Some("https://example.com"));
        assert_eq!(data.text_len(), data.joined_text().len());
        assert_eq!(
            data.joined_text(),
            "Hello world this is a long richer text run"
        );
        assert!(!data.has_embeds());
    }

    #[test]
    fn invalidate_layout_clears_both_caches() {
        let mut data = RichTextData {
            spans: spans(),
            ..Default::default()
        };
        assert!(data.ensure_layout(100.0).is_some());
        let _ = data.natural_width();
        data.invalidate_layout();
        assert!(data.layout.is_none());
        assert!(data.alt_layout.is_none());
        assert!(data.natural_width.is_none());
        assert_eq!(data.layout_width, -1.0);
        assert_eq!(data.alt_width, -1.0);
    }
}
