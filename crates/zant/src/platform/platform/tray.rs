//! Tray and status bar icons, tooltips, and menu items.

use std::ffi::CString;

use zant_sys::platform as ffi;

use super::SUPPORTED;

/// One entry of a tray / status bar menu.
pub struct TrayItem {
    pub title: Option<CString>,
    pub id: i32,
    pub separator: bool,
    pub checked: bool,
    pub disabled: bool,
}

impl TrayItem {
    pub fn new(title: &str, id: i32) -> TrayItem {
        TrayItem {
            title: CString::new(title).ok(),
            id,
            separator: false,
            checked: false,
            disabled: false,
        }
    }

    pub fn separator() -> TrayItem {
        TrayItem {
            title: None,
            id: 0,
            separator: true,
            checked: false,
            disabled: false,
        }
    }
}

/// Straight-alpha RGBA icon, `scale` pixels per point (macOS); `rgba_light`
/// (same size, Windows) for a light taskbar.
#[allow(clippy::too_many_arguments)] // mirrors the C ABI of ZantTrayItem
pub fn set_tray(
    rgba: &[u8],
    rgba_light: Option<&[u8]>,
    width: u32,
    height: u32,
    scale: f32,
    template_image: bool,
    tooltip: Option<&str>,
    items: &[TrayItem],
) -> bool {
    if !SUPPORTED {
        return false;
    }
    let tooltip = tooltip.and_then(|text| CString::new(text).ok());
    let native: Vec<ffi::ZantTrayItem> = items
        .iter()
        .map(|item| ffi::ZantTrayItem {
            title: item.title.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
            id: item.id,
            separator: i32::from(item.separator),
            checked: i32::from(item.checked),
            disabled: i32::from(item.disabled),
        })
        .collect();
    // SAFETY: every pointer in `native` outlives the call; the native side
    // copies the icon and reads the menu items during the call.
    unsafe {
        ffi::zant_tray_set(
            if rgba.is_empty() {
                std::ptr::null()
            } else {
                rgba.as_ptr()
            },
            rgba_light.map_or(std::ptr::null(), |light| light.as_ptr()),
            i32::try_from(width).unwrap_or(i32::MAX),
            i32::try_from(height).unwrap_or(i32::MAX),
            scale,
            i32::from(template_image),
            tooltip
                .as_ref()
                .map_or(std::ptr::null(), |text| text.as_ptr()),
            if native.is_empty() {
                std::ptr::null()
            } else {
                native.as_ptr()
            },
            i32::try_from(native.len()).unwrap_or(i32::MAX),
        ) != 0
    }
}

/// Icon and tooltip only (an open menu stays open); false without a tray icon.
#[allow(clippy::too_many_arguments)] // mirrors the C ABI
pub fn set_tray_icon(
    rgba: &[u8],
    rgba_light: Option<&[u8]>,
    width: u32,
    height: u32,
    scale: f32,
    template_image: bool,
    tooltip: Option<&str>,
) -> bool {
    if !SUPPORTED {
        return false;
    }
    let tooltip = tooltip.and_then(|text| CString::new(text).ok());
    // SAFETY: valid icon slices and tooltip for the call.
    unsafe {
        ffi::zant_tray_set_icon(
            if rgba.is_empty() {
                std::ptr::null()
            } else {
                rgba.as_ptr()
            },
            rgba_light.map_or(std::ptr::null(), |light| light.as_ptr()),
            i32::try_from(width).unwrap_or(i32::MAX),
            i32::try_from(height).unwrap_or(i32::MAX),
            scale,
            i32::from(template_image),
            tooltip
                .as_ref()
                .map_or(std::ptr::null(), |text| text.as_ptr()),
        ) != 0
    }
}

pub fn remove_tray() {
    if SUPPORTED {
        // SAFETY: no arguments.
        unsafe { ffi::zant_tray_remove() }
    }
}

/// Pixel size the tray / status bar icon should have on this display (at least 8).
pub fn tray_icon_pixels() -> u32 {
    if !SUPPORTED {
        return 16;
    }
    // SAFETY: no arguments.
    u32::try_from(unsafe { ffi::zant_tray_icon_pixels() }.max(8)).unwrap_or(16)
}
