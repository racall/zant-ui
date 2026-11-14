//! Port of `src/image.zig` — WP-1.1c.
//!
//! Decoded images held in memory, for `ui::image`. Use it for pixels that do
//! not come from a file: rendered PDF pages, downloaded or decoded images,
//! `data:` URIs, thumbnails.
//!
//! Creating an image is thread-safe (decode on a worker thread, then hand the
//! `Image` to the UI thread and call `ui::post_wake`). Nodes showing the image
//! keep their own reference, so dropping one may happen while it is shown.
//!
//! This is one of the few modules allowed to use `unsafe` (plan.md §3 rule 5):
//! `Image` is a reference-counted Skia image and `Send + Sync` because the
//! native layer creates it thread-safely (see `skia_bridge.h`).

use std::ffi::CString;
use std::sync::atomic::{AtomicU64, Ordering};

use zant_sys::skia as ffi;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Errors of `Image::encode_png` (Zig: `error.EncodeFailed`).
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("PNG encoding failed")]
    EncodeFailed,
}

/// Byte order `Image::from_pixels` expects on this platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelOrder {
    Rgba,
    Bgra,
}

/// Byte order `Image::from_pixels` expects on this platform.
pub const NATIVE_ORDER: PixelOrder = if cfg!(target_os = "windows") {
    PixelOrder::Bgra
} else {
    PixelOrder::Rgba
};

/// A decoded image. Cloning retains another handle to the same pixels; every
/// handle is released on drop.
pub struct Image {
    handle: *mut ffi::ZantImage,
    /// Unique per image (a node compares ids to notice a new image).
    pub id: u64,
    pub width: u32,
    pub height: u32,
}

// SAFETY: the native image is immutable once created and the bridge creates,
// retains and destroys it thread-safely (skia_bridge.h: "Thread-safe").
unsafe impl Send for Image {}
// SAFETY: same as above: shared access only reads immutable pixels.
unsafe impl Sync for Image {}

impl Image {
    fn wrap(handle: *mut ffi::ZantImage) -> Option<Image> {
        if handle.is_null() {
            return None;
        }
        // SAFETY: `handle` is a live ZantImage owned by this `Image`.
        let (width, height) = unsafe {
            (
                ffi::zant_image_width(handle).max(0),
                ffi::zant_image_height(handle).max(0),
            )
        };
        Some(Image {
            handle,
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            width: u32::try_from(width).unwrap_or(0),
            height: u32::try_from(height).unwrap_or(0),
        })
    }

    /// Copies premultiplied 32-bit pixels in the platform's native order
    /// (`NATIVE_ORDER`: RGBA on macOS, BGRA on Windows).
    pub fn from_pixels(pixels: &[u8], width: u32, height: u32, stride: u32) -> Option<Image> {
        if width == 0
            || height == 0
            || stride < width * 4
            || (pixels.len() as u64) < u64::from(stride) * u64::from(height)
        {
            return None;
        }
        // SAFETY: the buffer length was validated above; the bridge copies the
        // pixels before returning.
        let handle = unsafe {
            ffi::zant_image_from_pixels(
                pixels.as_ptr().cast(),
                i32::try_from(width).unwrap_or(i32::MAX),
                i32::try_from(height).unwrap_or(i32::MAX),
                i32::try_from(stride).unwrap_or(i32::MAX),
            )
        };
        Image::wrap(handle)
    }

    /// Decodes PNG, JPEG, GIF (first frame), BMP or ICO bytes.
    pub fn from_encoded(bytes: &[u8]) -> Option<Image> {
        if bytes.is_empty() {
            return None;
        }
        // SAFETY: valid slice for the call; the bridge decodes and copies.
        let handle = unsafe { ffi::zant_image_from_encoded(bytes.as_ptr().cast(), bytes.len()) };
        Image::wrap(handle)
    }

    /// Decodes `bytes` scaled down so the larger side is at most `max_dim`
    /// pixels (list thumbnails without keeping full-size pixels).
    pub fn thumbnail(bytes: &[u8], max_dim: u32) -> Option<Image> {
        if bytes.is_empty() || max_dim == 0 {
            return None;
        }
        let max_dim = i32::try_from(max_dim).unwrap_or(i32::MAX);
        // SAFETY: valid slice for the call; the bridge decodes and copies.
        let handle =
            unsafe { ffi::zant_image_thumbnail(bytes.as_ptr().cast(), bytes.len(), max_dim) };
        Image::wrap(handle)
    }

    /// Draws the image into `rect`, clipped to `radius`; `fit` is the
    /// `ImageFit` discriminant (cover = 0, contain = 1, fill = 2).
    ///
    /// Port of `components/image.zig:paint`'s native call
    /// (`zant_canvas_draw_image_rect`), which WP-2.2 needs from
    /// `components::image` where `unsafe` is not allowed (plan.md §3 rule 5).
    pub fn draw_rect(
        &self,
        canvas: crate::renderer::canvas::Canvas,
        rect: crate::core::geometry::Rect,
        radius: f32,
        fit: i32,
    ) {
        // SAFETY: both handles are live for the current frame; `self` keeps the
        // image alive for the duration of the call.
        unsafe {
            ffi::zant_canvas_draw_image_rect(
                canvas.handle(),
                self.handle,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                radius,
                fit,
            );
        }
    }

    /// PNG bytes of the image.
    pub fn encode_png(&self) -> Result<Vec<u8>, ImageError> {
        let mut len: usize = 0;
        // SAFETY: live image handle; `len` is a valid out pointer.
        let bytes = unsafe { ffi::zant_image_encode_png(self.handle, &mut len) };
        if bytes.is_null() {
            return Err(ImageError::EncodeFailed);
        }
        // SAFETY: the bridge returned `len` initialised bytes; the buffer is
        // freed once, after the copy.
        let copy = unsafe {
            let slice = std::slice::from_raw_parts(bytes.cast::<u8>(), len);
            let copy = slice.to_vec();
            ffi::zant_bytes_free(bytes);
            copy
        };
        Ok(copy)
    }

    /// Straight-alpha RGBA pixels, `width * 4` bytes per row (the tray icon
    /// path in `desktop` needs the decoded pixels; the native call copies).
    pub fn read_rgba(&self) -> Option<Vec<u8>> {
        let size = usize::try_from(self.width)
            .ok()?
            .checked_mul(usize::try_from(self.height).ok()?)?
            .checked_mul(4)?;
        if size == 0 {
            return None;
        }
        let mut pixels = vec![0u8; size];
        // SAFETY: `pixels` has room for `size` bytes; the bridge is thread-safe
        // and returns 0 without writing when the sizes disagree.
        let read = unsafe {
            ffi::zant_image_read_rgba(self.handle, pixels.as_mut_ptr().cast(), pixels.len())
        };
        (read != 0).then_some(pixels)
    }

    /// Another reference to the same pixels (released with its own drop).
    pub fn retain(&self) -> Image {
        // SAFETY: live image handle; retain returns a new owned handle.
        let handle = unsafe { ffi::zant_image_retain(self.handle) };
        if handle.is_null() {
            return Image {
                handle: self.handle,
                id: self.id,
                width: self.width,
                height: self.height,
            };
        }
        Image {
            handle,
            id: self.id,
            width: self.width,
            height: self.height,
        }
    }

    /// Decodes a `data:image/...;base64,...` URI (`None` for other URIs).
    pub fn from_data_uri(uri: &str) -> Option<Image> {
        if !uri.starts_with("data:") {
            return None;
        }
        let comma = uri.find(',')?;
        let meta = &uri[5..comma];
        if !meta.ends_with(";base64") {
            return None;
        }
        let payload = &uri[comma + 1..];
        let bytes = base64_decode(payload)?;
        Image::from_encoded(&bytes)
    }
}

impl Clone for Image {
    /// `clone` is `retain`: another owned handle to the same pixels.
    fn clone(&self) -> Self {
        self.retain()
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: this handle is owned by `self` and released once.
            unsafe { ffi::zant_image_destroy(self.handle) };
            self.handle = std::ptr::null_mut();
        }
    }
}

/// Standard-alphabet base64 decoder (no crate dependency, like Zig's
/// `std.base64.standard.Decoder`). Accepts padding; returns `None` on invalid
/// input or length.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3 + 3);
    let mut chunk = [0u8; 4];
    let mut filled = 0usize;
    let mut padding = 0usize;
    for &byte in bytes {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => {
                padding += 1;
                chunk[filled] = 0;
                filled += 1;
                if filled == 4 {
                    if padding > 2 {
                        return None;
                    }
                    let word = u32::from(chunk[0]) << 18
                        | u32::from(chunk[1]) << 12
                        | u32::from(chunk[2]) << 6
                        | u32::from(chunk[3]);
                    out.push((word >> 16) as u8);
                    if padding < 2 {
                        out.push((word >> 8) as u8);
                    }
                    if padding < 1 {
                        out.push(word as u8);
                    }
                    filled = 0;
                }
                continue;
            }
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        if padding > 0 {
            return None;
        }
        chunk[filled] = value;
        filled += 1;
        if filled == 4 {
            let word = u32::from(chunk[0]) << 18
                | u32::from(chunk[1]) << 12
                | u32::from(chunk[2]) << 6
                | u32::from(chunk[3]);
            out.push((word >> 16) as u8);
            out.push((word >> 8) as u8);
            out.push(word as u8);
            filled = 0;
        }
    }
    if filled != 0 {
        return None;
    }
    Some(out)
}

/// Writes an image's PNG bytes to `path` (helper the Zig side built with
/// `zant_write_file`; kept here so snapshot/debug code has one entry point).
pub fn write_png(path: &str, bytes: &[u8]) -> bool {
    let Ok(path) = CString::new(path) else {
        return false;
    };
    // SAFETY: NUL-terminated path and a valid buffer for the call.
    unsafe { ffi::zant_write_file(path.as_ptr(), bytes.as_ptr().cast(), bytes.len()) != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_decoder_matches_the_standard_alphabet() {
        assert_eq!(base64_decode("").unwrap(), Vec::<u8>::new());
        assert_eq!(base64_decode("TWFu").unwrap(), b"Man");
        assert_eq!(base64_decode("TWE=").unwrap(), b"Ma");
        assert_eq!(base64_decode("TQ==").unwrap(), b"M");
        assert!(base64_decode("TQ=").is_none());
        assert!(base64_decode("!!!!").is_none());
    }

    #[test]
    fn invalid_pixel_buffers_are_rejected() {
        assert!(
            Image::from_pixels(&[0; 8], 2, 2, 8).is_none(),
            "stride too small"
        );
        assert!(
            Image::from_pixels(&[0; 8], 2, 2, 8).is_none(),
            "buffer too small"
        );
        assert!(Image::from_encoded(&[]).is_none());
        assert!(Image::thumbnail(&[1, 2, 3], 0).is_none());
    }

    #[test]
    fn pixels_round_trip_through_png_and_thumbnails() {
        let mut pixels = vec![0u8; 40 * 20 * 4];
        for pixel in pixels.chunks_exact_mut(4) {
            match NATIVE_ORDER {
                PixelOrder::Rgba => pixel.copy_from_slice(&[255, 0, 0, 255]),
                PixelOrder::Bgra => pixel.copy_from_slice(&[0, 0, 255, 255]),
            }
        }
        let image = Image::from_pixels(&pixels, 40, 20, 40 * 4).expect("image");
        assert_eq!((image.width, image.height), (40, 20));
        let png = image.encode_png().expect("png");
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));

        let thumb = Image::thumbnail(&png, 8).expect("thumbnail");
        assert!(thumb.width <= 8 && thumb.height <= 8);
        assert!(thumb.width > 0 && thumb.height > 0);

        let same = Image::thumbnail(&png, 100).expect("thumbnail");
        assert!(same.width >= 40, "small images are not enlarged");

        let data_uri = format!("data:image/png;base64,{}", base64_encode(&png));
        let decoded = Image::from_data_uri(&data_uri).expect("data uri image");
        assert_eq!((decoded.width, decoded.height), (40, 20));
        assert!(Image::from_data_uri("https://example.com/a.png").is_none());
    }

    #[test]
    fn retain_and_clone_keep_the_same_identity() {
        let pixels = vec![0u8; 16];
        let image = Image::from_pixels(&pixels, 2, 2, 8).expect("image");
        let kept = image.retain();
        let cloned = image.clone();
        assert_eq!(kept.id, image.id);
        assert_eq!(cloned.id, image.id);
    }

    fn base64_encode(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let b0 = chunk[0];
            let b1 = chunk.get(1).copied().unwrap_or(0);
            let b2 = chunk.get(2).copied().unwrap_or(0);
            out.push(ALPHABET[(b0 >> 2) as usize] as char);
            out.push(ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
            if chunk.len() > 1 {
                out.push(ALPHABET[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                out.push(ALPHABET[(b2 & 0x3F) as usize] as char);
            } else {
                out.push('=');
            }
        }
        out
    }
}
