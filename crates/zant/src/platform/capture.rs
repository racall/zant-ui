//! Screens, other apps' windows, screen capture and text recognition: safe
//! wrappers of the C ABI (`zant_screen_*`, `zant_window_list`, `zant_ocr_*`).
//!
//! Desktop units: macOS points, Windows physical pixels; global coordinates
//! with the origin at the primary screen's top left, y down.

use zant_sys::platform as ffi;

use super::platform::SUPPORTED;

/// One connected screen.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenInfo {
    pub id: u32,
    /// Full frame in desktop units.
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Work area (minus menu bar / Dock / taskbar) in desktop units.
    pub work_x: f32,
    pub work_y: f32,
    pub work_width: f32,
    pub work_height: f32,
    /// Pixels per logical point.
    pub scale: f32,
    /// Desktop units per logical point (1 on macOS, `scale` on Windows).
    pub units_per_point: f32,
    pub primary: bool,
}

/// One on-screen window of any app, front to back.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowInfo {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub pid: u32,
    pub app: String,
}

/// The pixels of one screen: premultiplied 32-bit, `image::NATIVE_ORDER`.
pub struct ScreenCapture {
    pub screen_id: u32,
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
}

const MAX_SCREENS: usize = 16;

/// A zeroed C struct (plain integers, floats, chars and null pointers).
fn zeroed<T: Copy>() -> T {
    // SAFETY: only used for the bindgen POD structs of this file, for which
    // all-zero bytes are valid values.
    unsafe { std::mem::zeroed() }
}

pub fn screens() -> Vec<ScreenInfo> {
    if !SUPPORTED {
        return Vec::new();
    }
    let mut raw = [zeroed::<ffi::ZantScreenInfo>(); MAX_SCREENS];
    // SAFETY: `raw` holds MAX_SCREENS entries; the native side writes at most that many.
    let count = unsafe { ffi::zant_screen_list(raw.as_mut_ptr(), MAX_SCREENS as i32) };
    raw.iter()
        .take(count.clamp(0, MAX_SCREENS as i32) as usize)
        .map(|s| ScreenInfo {
            id: s.id,
            x: s.x,
            y: s.y,
            width: s.width,
            height: s.height,
            work_x: s.work_x,
            work_y: s.work_y,
            work_width: s.work_width,
            work_height: s.work_height,
            scale: s.scale,
            units_per_point: s.units_per_point,
            primary: s.primary != 0,
        })
        .collect()
}

pub fn windows() -> Vec<WindowInfo> {
    if !SUPPORTED {
        return Vec::new();
    }
    let mut raw: Vec<ffi::ZantWindowInfo> = vec![zeroed::<ffi::ZantWindowInfo>(); 512];
    // SAFETY: `raw` holds 512 entries; the native side writes at most that many.
    let count = unsafe { ffi::zant_window_list(raw.as_mut_ptr(), raw.len() as i32) };
    raw.truncate(count.clamp(0, 512) as usize);
    raw.iter()
        .map(|w| {
            let name: Vec<u8> = w.app.iter().take_while(|c| **c != 0).map(|c| *c as u8).collect();
            WindowInfo {
                x: w.x,
                y: w.y,
                width: w.width,
                height: w.height,
                pid: w.pid,
                app: String::from_utf8_lossy(&name).into_owned(),
            }
        })
        .collect()
}

pub fn capture_access() -> bool {
    // SAFETY: no arguments.
    SUPPORTED && unsafe { ffi::zant_screen_capture_access() != 0 }
}

pub fn request_capture_access() -> bool {
    // SAFETY: no arguments.
    SUPPORTED && unsafe { ffi::zant_screen_capture_request() != 0 }
}

/// Every screen, in `screens()` order; empty on failure. Blocks.
pub fn capture_all() -> Vec<ScreenCapture> {
    if !SUPPORTED {
        return Vec::new();
    }
    let mut raw = [zeroed::<ffi::ZantCapture>(); MAX_SCREENS];
    // SAFETY: `raw` holds MAX_SCREENS entries.
    let count = unsafe { ffi::zant_screen_capture_all(raw.as_mut_ptr(), MAX_SCREENS as i32) };
    let count = count.clamp(0, MAX_SCREENS as i32) as usize;
    let mut out = Vec::with_capacity(count);
    for slot in raw.iter().take(count) {
        if slot.pixels.is_null() {
            out.push(ScreenCapture { screen_id: slot.screen_id, pixels: Vec::new(), width: 0, height: 0, stride: 0 });
            continue;
        }
        let len = slot.stride as usize * slot.height as usize;
        // SAFETY: the native side allocated `stride * height` bytes; freed right after the copy.
        let pixels = unsafe { std::slice::from_raw_parts(slot.pixels, len).to_vec() };
        // SAFETY: allocated by zant_screen_capture_all.
        unsafe { ffi::zant_capture_free(slot.pixels) };
        out.push(ScreenCapture {
            screen_id: slot.screen_id,
            pixels,
            width: slot.width.max(0) as u32,
            height: slot.height.max(0) as u32,
            stride: slot.stride.max(0) as u32,
        });
    }
    // slots past `count` were never written
    out
}

/// One recognized line; token and line ranges are UTF-8 byte ranges into
/// `text` (token ranges relative to the line).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OcrLine {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub tokens: Vec<OcrToken>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OcrToken {
    pub start: usize,
    pub end: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub fn ocr_available() -> bool {
    // SAFETY: no arguments.
    SUPPORTED && unsafe { ffi::zant_ocr_available() != 0 }
}

pub fn ocr_recognize(pixels: &[u8], width: u32, height: u32, stride: u32) -> Option<Vec<OcrLine>> {
    if !SUPPORTED || width == 0 || height == 0 || (stride as usize) < width as usize * 4 {
        return None;
    }
    if pixels.len() < stride as usize * height as usize {
        return None;
    }
    // SAFETY: `pixels` holds `stride * height` bytes (checked above).
    let result = unsafe { ffi::zant_ocr_recognize(pixels.as_ptr(), width as i32, height as i32, stride as i32) };
    if result.is_null() {
        return None;
    }
    // SAFETY: a non-null result from zant_ocr_recognize; arrays hold the stated counts.
    let lines = unsafe {
        let r = &*result;
        let text = if r.text.is_null() {
            &[][..]
        } else {
            std::slice::from_raw_parts(r.text.cast::<u8>(), r.text_len)
        };
        let raw_lines = if r.lines.is_null() { &[][..] } else { std::slice::from_raw_parts(r.lines, r.line_count) };
        let raw_tokens =
            if r.tokens.is_null() { &[][..] } else { std::slice::from_raw_parts(r.tokens, r.token_count) };
        let mut lines = Vec::with_capacity(raw_lines.len());
        for line in raw_lines {
            let start = line.start as usize;
            let end = (start + line.len as usize).min(text.len());
            let Ok(line_text) = std::str::from_utf8(text.get(start..end).unwrap_or_default()) else { continue };
            let first = line.first_token as usize;
            let tokens = raw_tokens
                .get(first..(first + line.token_count as usize).min(raw_tokens.len()))
                .unwrap_or_default()
                .iter()
                .filter_map(|t| {
                    let ts = (t.start as usize).checked_sub(start)?;
                    let te = ts + t.len as usize;
                    (te <= line_text.len() && line_text.is_char_boundary(ts) && line_text.is_char_boundary(te))
                        .then_some(OcrToken { start: ts, end: te, x: t.x, y: t.y, width: t.width, height: t.height })
                })
                .collect();
            lines.push(OcrLine {
                text: line_text.to_string(),
                x: line.x,
                y: line.y,
                width: line.width,
                height: line.height,
                tokens,
            });
        }
        ffi::zant_ocr_free(result);
        lines
    };
    Some(lines)
}

/// Seconds east of UTC of the local time zone now.
pub fn local_utc_offset() -> i32 {
    if !SUPPORTED {
        return 0;
    }
    // SAFETY: no arguments.
    unsafe { ffi::zant_local_utc_offset() }
}
