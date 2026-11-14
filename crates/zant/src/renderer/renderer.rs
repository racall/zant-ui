//! A renderer owns the Skia surface (GPU on the native host, CPU raster for
//! tests and `ZANT_SNAPSHOT`). The window backends create and drive their own
//! renderer through the platform C ABI, so only the raster path is exposed
//! here (tests, snapshots, offscreen rendering).

use std::ffi::CStr;

use crate::renderer::canvas::Canvas;
use zant_sys::skia as ffi;

/// Errors of renderer creation.
#[derive(Debug, thiserror::Error)]
pub enum RendererError {
    #[error("renderer creation failed")]
    CreationFailed,
}

pub struct Renderer {
    handle: *mut ffi::ZantRenderer,
    width: i32,
    height: i32,
}

impl Renderer {
    /// Creates a CPU raster renderer of `width` × `height` pixels.
    pub fn create_raster(width: u32, height: u32) -> Result<Renderer, RendererError> {
        zant_sys::init();
        let handle = {
            // SAFETY: plain C constructor; returns null on failure.
            unsafe {
                ffi::zant_renderer_create_raster(
                    width.try_into().expect("int cast"),
                    height.try_into().expect("int cast"),
                )
            }
        };
        if handle.is_null() {
            return Err(RendererError::CreationFailed);
        }
        Ok(Renderer {
            handle,
            width: i32::try_from(width).expect("int cast"),
            height: i32::try_from(height).expect("int cast"),
        })
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn resize(&mut self, width: u32, height: u32) -> bool {
        // SAFETY: live renderer handle.
        let result = unsafe {
            ffi::zant_renderer_resize(
                self.handle,
                width.try_into().expect("int cast"),
                height.try_into().expect("int cast"),
            )
        };
        if result {
            self.width = i32::try_from(width).expect("int cast");
            self.height = i32::try_from(height).expect("int cast");
        }
        result
    }

    /// Starts a frame; the canvas is valid until `end()`.
    pub fn begin(&mut self, scale: f32) -> Option<Canvas> {
        // SAFETY: live renderer handle.
        let canvas = unsafe { ffi::zant_renderer_begin(self.handle, scale) };
        if canvas.is_null() {
            None
        } else {
            Some(Canvas::from_opaque(canvas))
        }
    }

    pub fn end(&mut self) {
        // SAFETY: matches an earlier `begin`.
        unsafe { ffi::zant_renderer_end(self.handle) }
    }

    /// Writes the raster surface to `path` as a PNG.
    pub fn write_png(&self, path: &str) -> bool {
        let Ok(path) = std::ffi::CString::new(path) else {
            return false;
        };
        // SAFETY: live renderer handle, NUL-terminated path.
        unsafe { ffi::zant_renderer_write_png(self.handle, path.as_ptr()) }
    }

    /// Human-readable backend name from the bridge, e.g. "Raster CPU (Skia)"
    /// or "Metal" / "Direct3D 12".
    pub fn backend_name(&self) -> &str {
        // SAFETY: the bridge returns a NUL-terminated string that lives as long
        // as the renderer (static name per backend).
        unsafe {
            let name = ffi::zant_renderer_backend_name(self.handle);
            if name.is_null() {
                ""
            } else {
                CStr::from_ptr(name).to_str().unwrap_or("")
            }
        }
    }

    pub fn is_gpu(&self) -> bool {
        // SAFETY: live renderer handle.
        unsafe { ffi::zant_renderer_is_gpu(self.handle) }
    }

    /// Pixel at (x, y) of a raster renderer as 0xAARRGGBB (premultiplied);
    /// `None` outside the surface or without CPU pixels (tests).
    pub fn pixel_at(&self, x: usize, y: usize) -> Option<u32> {
        let mut width: i32 = 0;
        let mut height: i32 = 0;
        let mut stride: i32 = 0;
        // SAFETY: live renderer handle; the out pointers are valid.
        let pixels =
            unsafe { ffi::zant_renderer_pixels(self.handle, &mut width, &mut height, &mut stride) };
        if pixels.is_null() {
            return None;
        }
        if x >= usize::try_from(width).unwrap_or(0) || y >= usize::try_from(height).unwrap_or(0) {
            return None;
        }
        // SAFETY: the point is inside the surface, checked above.
        Some(unsafe {
            ffi::zant_renderer_pixel_argb(
                self.handle,
                x.try_into().expect("int cast"),
                y.try_into().expect("int cast"),
            )
        })
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: we own the handle; it is destroyed once.
            unsafe { ffi::zant_renderer_destroy(self.handle) };
            self.handle = std::ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_renderer_reports_its_backend_and_size() {
        let mut renderer = Renderer::create_raster(64, 48).expect("raster renderer");
        assert_eq!(renderer.width(), 64);
        assert_eq!(renderer.height(), 48);
        assert!(!renderer.is_gpu());
        assert_eq!(renderer.backend_name(), "Raster CPU (Skia)");
        let canvas = renderer.begin(1.0).expect("canvas");
        canvas.clear(0xFFFF_FFFF);
        renderer.end();
        assert_eq!(renderer.pixel_at(0, 0), Some(0xFFFF_FFFF));
        assert_eq!(renderer.pixel_at(1000, 0), None, "outside the surface");
    }

    #[test]
    fn resize_updates_the_reported_size() {
        let mut renderer = Renderer::create_raster(32, 32).expect("raster renderer");
        assert!(renderer.resize(80, 40));
        assert_eq!((renderer.width(), renderer.height()), (80, 40));
        assert!(renderer.pixel_at(79, 39).is_some());
    }
}
