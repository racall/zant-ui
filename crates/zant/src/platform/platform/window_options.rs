//! Window styles, titlebar insets, and conversion of window options to the native ABI.

use zant_sys::platform as ffi;

#[cfg(doc)]
use super::window::Window;
use super::{IS_WINDOWS, system::getenv};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum WindowStyle {
    Native = 0,
    HiddenTitlebar = 1,
    Borderless = 2,
    /// Floating panel (quick-access popup): no title bar, system rounded
    /// corners and shadow, above other windows, no Dock / taskbar entry; on
    /// macOS it takes the keyboard without bringing the app forward.
    Panel = 3,
    /// Full-screen cover for screenshot / selection UIs: borderless, opaque,
    /// above everything (menu bar, taskbar, full-screen apps), no Dock /
    /// taskbar entry; takes the keyboard without activating the app on
    /// macOS. Size it with [`Window::cover_screen`].
    Overlay = 4,
}

#[derive(Clone, Copy, Debug)]
pub struct WindowOptions {
    pub style: WindowStyle,
    pub width: f32,
    pub height: f32,
    pub min_width: f32,
    pub min_height: f32,
    pub background: u32,
    pub resizable: bool,
    pub always_on_top: bool,
    pub movable_by_background: bool,
    pub center: bool,
    /// `.hidden_titlebar` only: height of the app's own title bar; the
    /// traffic lights are vertically centred in it (0 = system default).
    pub titlebar_height: f32,
    /// Create the window without showing it (show it later).
    pub hidden: bool,
    /// Not shown when the system opened the app at login (a tray app starting
    /// in the background).
    pub hidden_at_login: bool,
}

impl Default for WindowOptions {
    fn default() -> Self {
        WindowOptions {
            style: WindowStyle::Native,
            width: 560.0,
            height: 420.0,
            min_width: 320.0,
            min_height: 240.0,
            background: 0xfff5_f5f5,
            resizable: true,
            always_on_top: false,
            movable_by_background: false,
            center: true,
            titlebar_height: 0.0,
            hidden: false,
            hidden_at_login: false,
        }
    }
}

/// Width the system keeps for its own window controls in an app-drawn
/// (`.hidden_titlebar`) title bar.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TitlebarInsets {
    pub left: f32,
    pub right: f32,
}

/// Windows caption button size (Windows 11: 46 × 32 at 100 %).
pub const CAPTION_BUTTON_WIDTH: f32 = 46.0;

pub fn titlebar_insets(style: WindowStyle) -> TitlebarInsets {
    if style != WindowStyle::HiddenTitlebar {
        return TitlebarInsets::default();
    }
    if IS_WINDOWS {
        TitlebarInsets {
            left: 0.0,
            right: CAPTION_BUTTON_WIDTH * 3.0,
        }
    } else {
        TitlebarInsets {
            left: 84.0,
            right: 0.0,
        }
    }
}

/// `ZANT_WINDOW_SIZE=WxH` (automation): forces the logical window size so
/// scripts can take screenshots at fixed sizes and compare them.
pub(super) fn parse_window_size(value: &str) -> Option<(f32, f32)> {
    let (width, height) = value.split_once('x')?;
    let width: f32 = width.parse().ok()?;
    let height: f32 = height.parse().ok()?;
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    Some((width, height))
}

fn size_override() -> Option<(f32, f32)> {
    parse_window_size(&getenv("ZANT_WINDOW_SIZE")?)
}

/// The C window options of `options` (with the `ZANT_WINDOW_SIZE` override).
pub fn native_window_options(options: WindowOptions) -> ffi::ZantWindowOptions {
    to_native_options(options)
}

pub(super) fn to_native_options(options: WindowOptions) -> ffi::ZantWindowOptions {
    let size = size_override();
    ffi::ZantWindowOptions {
        style: options.style as i32,
        width: size.map_or(options.width, |s| s.0),
        height: size.map_or(options.height, |s| s.1),
        min_width: options.min_width,
        min_height: options.min_height,
        background: options.background,
        resizable: i32::from(options.resizable),
        always_on_top: i32::from(options.always_on_top),
        movable_by_background: i32::from(options.movable_by_background),
        center: i32::from(options.center),
        titlebar_height: options.titlebar_height,
        hidden: i32::from(options.hidden),
        hidden_at_login: i32::from(options.hidden_at_login),
    }
}
