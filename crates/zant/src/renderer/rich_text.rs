//! Paragraphs made of styled spans (inline markdown, syntax highlighting,
//! inline math placeholders). The native layout owns a copy of the span text.

use crate::core::geometry::Rect;
use zant_sys::skia as ffi;

/// Errors of rich text layout creation.
#[derive(Debug, thiserror::Error)]
pub enum RichTextError {
    #[error("rich text layout creation failed")]
    Failed,
}

/// One styled run of text. Fields mirror the native `ZantRichSpan`; `italic`
/// is a C `int` used as a boolean.
#[derive(Clone, Copy, Debug)]
pub struct Span<'a> {
    pub text: &'a str,
    pub font_size: f32,
    pub weight: i32,
    pub color: u32,
    pub italic: bool,
    pub family: i32,
    pub background: u32,
    /// Bit 0: underline, bit 1: line-through.
    pub decoration: i32,
    pub placeholder_width: f32,
    pub placeholder_ascent: f32,
    pub placeholder_descent: f32,
}

impl<'a> Default for Span<'a> {
    fn default() -> Self {
        Span {
            text: "",
            font_size: 14.0,
            weight: 400,
            color: 0xe000_0000,
            italic: false,
            family: 0,
            background: 0,
            decoration: 0,
            placeholder_width: 0.0,
            placeholder_ascent: 0.0,
            placeholder_descent: 0.0,
        }
    }
}

impl<'a> Span<'a> {
    pub fn new(text: &'a str) -> Span<'a> {
        Span {
            text,
            ..Default::default()
        }
    }
}

pub struct Layout {
    handle: *mut ffi::ZantTextLayout,
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
}

impl Layout {
    pub fn create(
        spans: &[Span<'_>],
        max_width: f32,
        line_height: f32,
    ) -> Result<Layout, RichTextError> {
        zant_sys::init();
        let start = crate::renderer::text::shaping_start();
        // SAFETY: every pointer in `native` points into one of the `spans`,
        // which outlive the call; the layout copies the text before returning.
        let handle = unsafe {
            let native: Vec<ffi::ZantRichSpan> = spans
                .iter()
                .map(|span| ffi::ZantRichSpan {
                    text: span.text.as_ptr().cast(),
                    length: span.text.len(),
                    font_size: span.font_size,
                    weight: span.weight,
                    color: span.color,
                    italic: span.italic as i32,
                    family: span.family,
                    background: span.background,
                    decoration: span.decoration,
                    placeholder_width: span.placeholder_width,
                    placeholder_ascent: span.placeholder_ascent,
                    placeholder_descent: span.placeholder_descent,
                })
                .collect();
            ffi::zant_rich_text_layout_create(native.as_ptr(), native.len(), max_width, line_height)
        };
        crate::renderer::text::shaping_finished(start);
        if handle.is_null() {
            return Err(RichTextError::Failed);
        }
        // SAFETY: fresh handle from the call above.
        let metrics = unsafe { ffi::zant_text_layout_metrics(handle) };
        Ok(Layout {
            handle,
            width: metrics.width,
            height: metrics.height,
            baseline: metrics.first_baseline,
        })
    }

    /// Re-wraps at another width, reusing the shaped text.
    pub fn relayout(&mut self, max_width: f32) {
        // SAFETY: live layout handle.
        unsafe {
            ffi::zant_text_layout_relayout(self.handle, max_width);
            let metrics = ffi::zant_text_layout_metrics(self.handle);
            self.width = metrics.width;
            self.height = metrics.height;
            self.baseline = metrics.first_baseline;
        }
    }

    pub fn handle(&self) -> *mut ffi::ZantTextLayout {
        self.handle
    }

    /// UTF-8 byte offset (into the concatenated span text) nearest to a point
    /// relative to the layout origin.
    pub fn index_at(&self, x: f32, y: f32) -> usize {
        // SAFETY: live layout handle.
        unsafe { ffi::zant_text_layout_index_at(self.handle, x, y) }
    }

    /// Selection rectangles for a UTF-8 byte range (into the concatenated span
    /// text); returns the number written.
    pub fn range_rects(&self, start: usize, end: usize, out: &mut [Rect]) -> usize {
        if out.is_empty() {
            return 0;
        }
        // SAFETY: `Rect` is repr(C) with the same layout as `ZantRect`, the
        // slice has `out.len()` elements and the handle is alive.
        let count = unsafe {
            ffi::zant_text_layout_range_rects(
                self.handle,
                start,
                end,
                out.as_mut_ptr().cast(),
                i32::try_from(out.len()).expect("int cast"),
            )
        };
        usize::try_from(count.max(0)).unwrap_or(0).min(out.len())
    }

    /// Placeholder boxes in span order (relative to the layout origin).
    pub fn placeholder_rects<'a>(&self, out: &'a mut [Rect]) -> &'a mut [Rect] {
        if out.is_empty() {
            return out;
        }
        // SAFETY: `Rect` is repr(C) with the same layout as `ZantRect`, and a
        // slice of `Rect` is exactly `capacity` native rectangles.
        let count = unsafe {
            ffi::zant_text_layout_placeholder_rects(
                self.handle,
                out.as_mut_ptr().cast(),
                i32::try_from(out.len()).expect("int cast"),
            )
        };
        let count = usize::try_from(count.max(0)).unwrap_or(0).min(out.len());
        &mut out[..count]
    }

    pub fn paint(&self, canvas: crate::renderer::canvas::Canvas, x: f32, y: f32) {
        // SAFETY: both handles belong to the current frame.
        unsafe {
            ffi::zant_canvas_draw_text_layout(canvas.handle(), self.handle, x, y);
        }
    }
}

impl Drop for Layout {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: we own the handle; it is destroyed once.
            unsafe { ffi::zant_text_layout_destroy(self.handle) };
            self.handle = std::ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_spans_measure_and_wrap() {
        let spans = [
            Span {
                text: "bold ",
                font_size: 16.0,
                weight: 700,
                ..Default::default()
            },
            Span {
                text: "and normal text that wraps eventually",
                font_size: 16.0,
                weight: 400,
                color: 0xff33_3333,
                ..Default::default()
            },
        ];
        let layout = Layout::create(&spans, 1000.0, 0.0).expect("layout");
        assert!(layout.width > 0.0);
        assert!(layout.height > 0.0);
        assert!(layout.baseline > 0.0);
    }

    #[test]
    fn an_italic_span_does_not_change_the_api_shape() {
        let spans = [Span {
            text: "italic",
            italic: true,
            decoration: 1,
            background: 0x22ff_0000,
            ..Default::default()
        }];
        let layout = Layout::create(&spans, 200.0, 0.0).expect("layout");
        assert!(layout.width > 0.0);
        assert_eq!(layout.index_at(0.0, 0.0), 0);
    }

    #[test]
    fn relayout_recomputes_the_height() {
        let spans = [Span::new(
            "one two three four five six seven eight nine ten",
        )];
        let mut layout = Layout::create(&spans, 1000.0, 0.0).expect("layout");
        let wide_height = layout.height;
        layout.relayout(60.0);
        assert!(layout.height > wide_height);
    }
}
