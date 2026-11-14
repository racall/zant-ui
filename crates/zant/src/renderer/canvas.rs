//! Port of `src/renderer/canvas.zig` — WP-1.1c.
//!
//! `Canvas` is a thin handle over the Skia canvas the native host hands to the
//! frame callback. It is `Copy` like the Zig struct: the native canvas outlives
//! every handle used during one frame.

use crate::core::geometry::Rect;
use zant_sys::skia as ffi;

/// A drawing context for one frame.
#[derive(Clone, Copy, Debug)]
pub struct Canvas {
    handle: *mut ffi::ZantCanvas,
}

impl Canvas {
    /// Wraps the canvas pointer passed to `Host::draw`.
    pub fn from_opaque(handle: *mut ffi::ZantCanvas) -> Canvas {
        Canvas { handle }
    }

    /// The raw pointer, for the few places that pass it back to the native side
    /// (rich text painting, PDF tiles).
    pub fn handle(&self) -> *mut ffi::ZantCanvas {
        self.handle
    }

    pub fn save(&self) {
        // SAFETY: `handle` is the live canvas of the current frame.
        unsafe { ffi::zant_canvas_save(self.handle) }
    }

    pub fn restore(&self) {
        // SAFETY: matches an earlier `save` on the same canvas.
        unsafe { ffi::zant_canvas_restore(self.handle) }
    }

    pub fn translate(&self, x: f32, y: f32) {
        // SAFETY: live canvas handle.
        unsafe { ffi::zant_canvas_translate(self.handle, x, y) }
    }

    pub fn scale(&self, sx: f32, sy: f32) {
        // SAFETY: live canvas handle.
        unsafe { ffi::zant_canvas_scale(self.handle, sx, sy) }
    }

    /// Draws into an offscreen layer composited with `alpha` on `restore()`.
    pub fn save_layer_alpha(&self, bounds: Rect, alpha: f32) {
        // SAFETY: live canvas handle.
        unsafe {
            ffi::zant_canvas_save_layer_alpha(
                self.handle,
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                alpha,
            )
        }
    }

    pub fn clip_rect(&self, rect: Rect) {
        // SAFETY: live canvas handle.
        unsafe { ffi::zant_canvas_clip_rect(self.handle, rect.x, rect.y, rect.width, rect.height) }
    }

    pub fn clear(&self, color: u32) {
        // SAFETY: live canvas handle.
        unsafe { ffi::zant_canvas_clear(self.handle, color) }
    }

    pub fn fill_rect(&self, rect: Rect, color: u32) {
        // SAFETY: live canvas handle.
        unsafe {
            ffi::zant_canvas_fill_rect(self.handle, rect.x, rect.y, rect.width, rect.height, color)
        }
    }

    pub fn fill_round_rect(&self, rect: Rect, radius: f32, color: u32) {
        // SAFETY: live canvas handle.
        unsafe {
            ffi::zant_canvas_fill_round_rect(
                self.handle,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                radius,
                color,
            )
        }
    }

    pub fn shadow_round_rect(&self, rect: Rect, radius: f32, color: u32, blur: f32) {
        // SAFETY: live canvas handle.
        unsafe {
            ffi::zant_canvas_shadow_round_rect(
                self.handle,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                radius,
                color,
                blur,
            )
        }
    }

    pub fn stroke_round_rect(&self, rect: Rect, radius: f32, color: u32, width: f32) {
        // SAFETY: live canvas handle.
        unsafe {
            ffi::zant_canvas_stroke_round_rect(
                self.handle,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                radius,
                color,
                width,
            )
        }
    }

    pub fn draw_line(&self, x0: f32, y0: f32, x1: f32, y1: f32, color: u32, width: f32) {
        // SAFETY: live canvas handle.
        unsafe { ffi::zant_canvas_draw_line(self.handle, x0, y0, x1, y1, color, width) }
    }

    pub fn draw_text_layout(&self, layout: &crate::renderer::text::TextLayout, x: f32, y: f32) {
        // SAFETY: both handles belong to the current frame; the layout is alive.
        unsafe {
            ffi::zant_canvas_draw_text_layout(self.handle, layout.handle(), x, y);
        }
    }
}
