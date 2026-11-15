//! Screens, the windows of other apps, screen capture and the built-in text
//! recognizer, for screenshot tools.
//!
//! Rects are desktop units: macOS points, Windows physical pixels; global,
//! origin at the primary screen's top left, y down. Convert a rect to the
//! logical points of an overlay covering screen `s` with
//! `(x - s.x) / s.units_per_point`.
//!
//! ```ignore
//! let screens = desktop::screens();
//! let windows = desktop::visible_windows();     // front to back
//! let shots = desktop::capture_screens();       // background thread
//! overlay.cover_screen(0);
//! ```
#![forbid(unsafe_code)]

use crate::platform::capture;

pub use crate::platform::capture::{OcrLine, OcrToken, ScreenCapture, ScreenInfo, WindowInfo};

/// Connected screens (the order `Window::cover_screen` and
/// [`capture_screens`] use).
pub fn screens() -> Vec<ScreenInfo> {
    capture::screens()
}

/// Normal windows of every app that are on screen, front to back (no
/// desktop, menu bar, Dock / taskbar or hidden / minimized windows; none
/// smaller than 40×40 points). Any thread.
pub fn visible_windows() -> Vec<WindowInfo> {
    capture::windows()
}

/// Screen recording permission (always true on Windows).
pub fn screen_capture_access() -> bool {
    capture::capture_access()
}

/// macOS: the system prompt the first time; afterwards the user enables it
/// in System Settings (usually followed by a restart). Returns
/// [`screen_capture_access`].
pub fn request_screen_capture_access() -> bool {
    capture::request_capture_access()
}

/// Every screen at native resolution without the cursor, in [`screens`]
/// order; empty without permission or on failure. Blocks: call it from a
/// background thread (never the UI thread). A screen that failed has empty
/// `pixels`.
pub fn capture_screens() -> Vec<ScreenCapture> {
    capture::capture_all()
}

/// The system has a built-in text recognizer (macOS Vision).
pub fn ocr_available() -> bool {
    capture::ocr_available()
}

/// Recognizes text in premultiplied 32-bit pixels (`image::NATIVE_ORDER`).
/// Lines with word-like token boxes in image pixels. Blocks; any thread.
pub fn ocr_recognize(pixels: &[u8], width: u32, height: u32, stride: u32) -> Option<Vec<OcrLine>> {
    capture::ocr_recognize(pixels, width, height, stride)
}

/// Seconds east of UTC of the local time zone right now (for showing
/// timestamps in local time).
pub fn local_utc_offset() -> i32 {
    capture::local_utc_offset()
}
