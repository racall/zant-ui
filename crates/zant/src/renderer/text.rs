//! Shaped text layouts (Skia Paragraph) plus the caret / selection geometry
//! helpers the editor needs.

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
use std::time::Instant;

use crate::core::geometry::Rect;
use zant_sys::skia as ffi;

/// Errors of text layout creation.
#[derive(Debug, thiserror::Error)]
pub enum TextLayoutError {
    #[error("text layout creation failed")]
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Weight {
    Thin = 100,
    ExtraLight = 200,
    Light = 300,
    Regular = 400,
    Medium = 500,
    Semibold = 600,
    Bold = 700,
    ExtraBold = 800,
    Black = 900,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Family {
    Sans = 0,
    Serif = 1,
    Mono = 2,
    SerifCjk = 3,
    /// STIX Two Math (TeX-like math glyphs).
    Math = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Align {
    Left = 0,
    Center = 1,
    Right = 2,
}

#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub font_size: f32,
    pub weight: Weight,
    pub text_align: Align,
    pub wrap: bool,
    pub line_height: f32,
    pub color: u32,
    pub max_lines: u16,
    pub family: Family,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            font_size: 14.0,
            weight: Weight::Regular,
            text_align: Align::Left,
            wrap: true,
            line_height: 0.0,
            color: 0xe000_0000,
            max_lines: 0,
            family: Family::Sans,
        }
    }
}

/// Layout metrics in logical points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Metrics {
    pub width: f32,
    pub height: f32,
    pub intrinsic_width: f32,
    pub longest_line: f32,
    pub first_baseline: f32,
    pub line_count: usize,
}

fn metrics_from_native(native: ffi::ZantTextMetrics) -> Metrics {
    Metrics {
        width: native.width,
        height: native.height,
        intrinsic_width: native.max_intrinsic_width,
        longest_line: native.longest_line,
        first_baseline: native.first_baseline,
        line_count: usize::try_from(native.line_count.max(0)).unwrap_or(0),
    }
}

fn native_style(style: Style) -> ffi::ZantTextStyle {
    ffi::ZantTextStyle {
        font_size: style.font_size,
        // The C ABI takes `int` fields for the enums.
        weight: style.weight as i32,
        align: style.text_align as i32,
        wrap: style.wrap,
        line_height: style.line_height,
        color: style.color,
        max_lines: style.max_lines as i32,
        family: style.family as i32,
    }
}

/// Text layouts created and time spent shaping them (`ZANT_FRAME_STATS`).
///
/// The counters are atomics so the runtime can read and reset them from the
/// UI thread while the counter is bumped during layout.
pub struct LayoutStats {
    enabled: AtomicBool,
    count: AtomicU32,
    us: AtomicI64,
}

impl LayoutStats {
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Relaxed);
    }

    pub fn add(&self, us: i64) {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.us.fetch_add(us, Ordering::Relaxed);
    }

    /// Returns (count, microseconds) and resets the counters.
    pub fn take(&self) -> (u32, i64) {
        (
            self.count.swap(0, Ordering::Relaxed),
            self.us.swap(0, Ordering::Relaxed),
        )
    }
}

pub static LAYOUT_STATS: LayoutStats = LayoutStats {
    enabled: AtomicBool::new(false),
    count: AtomicU32::new(0),
    us: AtomicI64::new(0),
};

/// Measures the shaping time of one layout; `None` when stats are off.
pub(crate) fn shaping_start() -> Option<Instant> {
    if LAYOUT_STATS.enabled() {
        Some(Instant::now())
    } else {
        None
    }
}

pub(crate) fn shaping_finished(start: Option<Instant>) {
    if let Some(start) = start {
        LAYOUT_STATS.add(i64::try_from(start.elapsed().as_micros()).unwrap_or(i64::MAX));
    }
}

pub struct TextLayout {
    handle: *mut ffi::ZantTextLayout,
    metrics: Metrics,
}

impl TextLayout {
    /// Shapes `text`; `max_width` is used for wrapping when `style.wrap`.
    pub fn init(text: &str, style: Style, max_width: f32) -> Result<TextLayout, TextLayoutError> {
        zant_sys::init();
        let native_style = native_style(style);
        let start = shaping_start();
        // SAFETY: the text and style are valid for the call; the layout owns a
        // copy of the UTF-8 text (see skia_bridge.h).
        let handle = unsafe {
            ffi::zant_text_layout_create(text.as_ptr().cast(), text.len(), &native_style, max_width)
        };
        shaping_finished(start);
        if handle.is_null() {
            return Err(TextLayoutError::Failed);
        }
        // SAFETY: fresh handle from the call above.
        let metrics = metrics_from_native(unsafe { ffi::zant_text_layout_metrics(handle) });
        Ok(TextLayout { handle, metrics })
    }

    /// Re-wraps at another width, reusing the shaped text.
    pub fn relayout(&mut self, max_width: f32) {
        // SAFETY: live layout handle.
        unsafe { ffi::zant_text_layout_relayout(self.handle, max_width) };
        // SAFETY: live layout handle.
        self.metrics = metrics_from_native(unsafe { ffi::zant_text_layout_metrics(self.handle) });
    }

    pub fn metrics(&self) -> Metrics {
        self.metrics
    }

    /// The raw layout pointer (native drawing helpers take it).
    pub fn handle(&self) -> *mut ffi::ZantTextLayout {
        self.handle
    }

    pub fn paint(&self, canvas: crate::renderer::canvas::Canvas, x: f32, y: f32) {
        canvas.draw_text_layout(self, x, y);
    }

    /// Caret geometry (relative to the text origin) at a UTF-8 byte offset.
    pub fn caret_rect(&self, byte_offset: usize) -> Option<Rect> {
        let mut native = ffi::ZantRect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        };
        // SAFETY: live layout handle; `native` is a valid out pointer.
        let ok = unsafe { ffi::zant_text_layout_caret_rect(self.handle, byte_offset, &mut native) };
        if !ok {
            return None;
        }
        Some(Rect::init(native.x, native.y, native.width, native.height))
    }

    /// UTF-8 byte offset closest to a point relative to the text origin.
    pub fn index_at(&self, x: f32, y: f32) -> usize {
        // SAFETY: live layout handle.
        unsafe { ffi::zant_text_layout_index_at(self.handle, x, y) }
    }

    /// Selection rectangles for a UTF-8 byte range; returns the number written.
    pub fn range_rects(&self, start: usize, end: usize, out: &mut [ffi::ZantRect]) -> usize {
        if out.is_empty() {
            return 0;
        }
        let capacity = i32::try_from(out.len()).expect("int cast");
        // SAFETY: live layout handle; `out` has `capacity` elements.
        let count = unsafe {
            ffi::zant_text_layout_range_rects(self.handle, start, end, out.as_mut_ptr(), capacity)
        };
        usize::try_from(count.max(0)).unwrap_or(0)
    }
}

impl Drop for TextLayout {
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
    fn metrics_match_the_text() {
        let layout = TextLayout::init("Hello", Style::default(), 1000.0).expect("layout");
        let metrics = layout.metrics();
        assert!(metrics.width > 0.0);
        assert!(metrics.height > 0.0);
        assert_eq!(metrics.line_count, 1);
    }

    #[test]
    fn wrapping_reports_more_lines_than_the_width_allows() {
        let layout =
            TextLayout::init("one two three four", Style::default(), 40.0).expect("layout");
        assert!(layout.metrics().line_count > 1);
        assert!(layout.metrics().width <= 40.0);
    }

    #[test]
    fn relayout_reevaluates_the_metrics() {
        let mut layout =
            TextLayout::init("one two three", Style::default(), 1000.0).expect("layout");
        assert_eq!(layout.metrics().line_count, 1);
        layout.relayout(20.0);
        assert!(layout.metrics().line_count > 1);
    }

    #[test]
    fn caret_rect_is_at_the_start_for_offset_zero() {
        let layout = TextLayout::init("Hello", Style::default(), 1000.0).expect("layout");
        let caret = layout.caret_rect(0).expect("caret");
        assert_eq!(caret.x, 0.0);
        assert!(caret.height > 0.0);
        assert_eq!(layout.index_at(0.0, 1.0), 0);
    }

    #[test]
    fn layout_stats_count_shaping_when_enabled() {
        // The counters are process-wide; other tests lay out text in parallel, so the
        // count can only be checked to have grown, not to be exact.
        LAYOUT_STATS.set_enabled(false);
        let _ = LAYOUT_STATS.take();
        LAYOUT_STATS.set_enabled(true);
        let _ = TextLayout::init("stats", Style::default(), 1000.0).expect("layout");
        LAYOUT_STATS.set_enabled(false);
        let (count, _us) = LAYOUT_STATS.take();
        assert!(count >= 1, "shaping must be counted while stats are on");
    }
}

/// Glyph-ink metrics of a single-line run (see `zant_text_measure_ink`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InkMetrics {
    pub advance: f32,
    /// Ink extent above the baseline (positive).
    pub ascent: f32,
    /// Ink extent below the baseline (positive).
    pub descent: f32,
    pub font_ascent: f32,
    pub font_descent: f32,
    /// Signed ink bounds relative to the baseline (y grows down).
    pub ink_top: f32,
    pub ink_bottom: f32,
}

fn ink_metrics_from_native(native: ffi::ZantInkMetrics) -> InkMetrics {
    InkMetrics {
        advance: native.advance,
        ascent: native.ascent,
        descent: native.descent,
        font_ascent: native.font_ascent,
        font_descent: native.font_descent,
        ink_top: native.ink_top,
        ink_bottom: native.ink_bottom,
    }
}

/// Measures a single-line glyph run's ink box without laying out a paragraph
/// (math layout, stretchy delimiters).
pub fn measure_ink(text: &str, size: f32, family: Family, weight: u16, italic: bool) -> InkMetrics {
    if text.is_empty() {
        return InkMetrics::default();
    }
    zant_sys::init();
    // SAFETY: the text outlives the call; the bridge only reads it.
    let native = unsafe {
        ffi::zant_text_measure_ink(
            text.as_ptr().cast(),
            text.len(),
            size,
            family as i32,
            i32::from(weight),
            i32::from(italic),
        )
    };
    ink_metrics_from_native(native)
}
