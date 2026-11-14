//! WP-0.3 acceptance: the native layer compiles, links and works.
//!
//! Plan requirement: "冒烟测试：创建 raster renderer，画一个矩形，读回像素；
//! 创建一个 TextLayout 并量宽度。两平台通过" (docs/rust-port/plan.md WP-0.3).

use zant_sys::skia::*;

fn text_style() -> ZantTextStyle {
    ZantTextStyle {
        font_size: 16.0,
        weight: 400,
        align: 0,
        wrap: true,
        line_height: 0.0,
        color: 0xFF00_0000,
        max_lines: 0,
        family: 0,
    }
}

/// Fills a rectangle on a 64x64 raster surface and reads the pixels back.
#[test]
fn raster_renderer_draws_and_reads_a_rect() {
    zant_sys::init();
    // SAFETY: the pointers are checked and live for the duration of the scope;
    // each call follows the ownership rules documented in skia_bridge.h.
    unsafe {
        let renderer = zant_renderer_create_raster(64, 64);
        assert!(!renderer.is_null(), "raster renderer creation failed");
        assert!(
            !zant_renderer_is_gpu(renderer),
            "raster renderer reports a GPU"
        );

        let canvas = zant_renderer_begin(renderer, 1.0);
        assert!(!canvas.is_null(), "begin() returned no canvas");
        zant_canvas_clear(canvas, 0xFFFF_FFFF);
        zant_canvas_fill_rect(canvas, 8.0, 8.0, 48.0, 48.0, 0xFFFF_0000);
        zant_renderer_end(renderer);

        // Premultiplied 0xAARRGGBB, straight from the raster surface.
        assert_eq!(
            zant_renderer_pixel_argb(renderer, 32, 32),
            0xFFFF_0000,
            "the filled rectangle must be opaque red"
        );
        assert_eq!(
            zant_renderer_pixel_argb(renderer, 2, 2),
            0xFFFF_FFFF,
            "the area outside the rectangle stays white"
        );

        zant_renderer_destroy(renderer);
    }
}

/// Shapes a single-line paragraph and measures it.
#[test]
fn text_layout_measures_width() {
    zant_sys::init();
    let text = "Hello, Zant";
    let style = text_style();
    // SAFETY: the text and style outlive the layout; the layout owns a copy of
    // the UTF-8 text per the header contract.
    unsafe {
        let layout = zant_text_layout_create(text.as_ptr().cast(), text.len(), &style, 1000.0);
        assert!(!layout.is_null(), "text layout creation failed");
        let metrics = zant_text_layout_metrics(layout);
        assert!(
            metrics.width > 0.0,
            "measured width must be positive, got {}",
            metrics.width
        );
        assert!(
            metrics.width < 1000.0,
            "a 11-character line should not fill 1000pt, got {}",
            metrics.width
        );
        assert!(metrics.height > 0.0, "measured height must be positive");
        assert_eq!(metrics.line_count, 1, "single-line text must not wrap");
        zant_text_layout_destroy(layout);
    }
}
