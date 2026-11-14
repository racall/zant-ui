//! Tray icon / status bar item and its menu, including the PNG scaling of the
//! icons.
//!
//! The icons are PNG byte slices (8-bit, non-interlaced, square — the scaffold
//! ships one per platform); they are decoded, scaled to the system icon size
//! with an area-average filter in premultiplied alpha and handed to the
//! platform layer as straight-alpha RGBA.
#![forbid(unsafe_code)]

use crate::image::Image;
use crate::platform::platform;

/// Errors of [`set_tray`] / [`set_tray_icon`]: the tray is unavailable, or the
/// PNG could not be decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TrayError {
    /// The platform has no tray / status bar (or did not create the icon).
    #[error("tray / status bar item unavailable")]
    Unavailable,
    /// The icon bytes are not a decodable PNG / image.
    #[error("invalid tray icon image")]
    InvalidIcon,
}

/// One entry of the tray menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TrayItem<'a> {
    pub id: u32,
    pub title: &'a str,
    pub checked: bool,
    pub disabled: bool,
    pub separator: bool,
}

/// The tray icon and menu of [`set_tray`].
#[derive(Clone, Copy, Debug)]
pub struct TrayOptions<'a> {
    /// PNG (8-bit, non-interlaced), square; resized to the system size. On
    /// Windows the icon for a dark taskbar.
    pub icon: &'a [u8],
    /// Windows: the icon for a light taskbar; the tray follows the taskbar's
    /// light / dark mode. `None`: `icon` for both.
    pub icon_light: Option<&'a [u8]>,
    /// macOS: a template image (black + alpha) the menu bar tints.
    pub template: bool,
    pub tooltip: &'a str,
    /// The menu (macOS: opens on click; Windows: on right click, a left click
    /// sends `AppEvent::TrayClick`).
    pub items: &'a [TrayItem<'a>],
}

/// Icon and tooltip only.
#[derive(Clone, Copy, Debug)]
pub struct TrayIconOptions<'a> {
    pub icon: &'a [u8],
    pub icon_light: Option<&'a [u8]>,
    pub template: bool,
    pub tooltip: &'a str,
}

/// A decoded, decoded-size RGBA image.
struct Decoded {
    width: u32,
    height: u32,
    /// straight (not premultiplied) RGBA, row-major
    rgba: Vec<u8>,
}

impl Decoded {
    fn from_png(bytes: &[u8]) -> Result<Decoded, TrayError> {
        let image = Image::from_encoded(bytes).ok_or(TrayError::InvalidIcon)?;
        let rgba = image.read_rgba().ok_or(TrayError::InvalidIcon)?;
        Ok(Decoded {
            width: image.width,
            height: image.height,
            rgba,
        })
    }
}

/// A tray icon decoded and sized for the system.
struct TrayPixels {
    icon: Vec<u8>,
    light: Option<Vec<u8>>,
    width: u32,
    height: u32,
    scale: f32,
}

/// Decodes both icons and scales them to the system size (the
/// height is the platform's tray icon size, the width keeps the aspect ratio).
fn tray_pixels(icon_png: &[u8], light_png: Option<&[u8]>) -> Result<TrayPixels, TrayError> {
    let decoded = Decoded::from_png(icon_png)?;
    // macOS: 18 pt at @2x; Windows: the small-icon size for the DPI
    let height = platform::tray_icon_pixels();
    let width = if decoded.height == 0 {
        1
    } else {
        let scaled = (decoded.width as f32) * (height as f32) / (decoded.height as f32);
        (scaled.round() as u32).max(1)
    };
    let icon = if decoded.width == width && decoded.height == height {
        decoded.rgba
    } else {
        resize_rgba(&decoded.rgba, decoded.width, decoded.height, width, height)
    };
    let light = match light_png {
        Some(bytes) => {
            let decoded = Decoded::from_png(bytes)?;
            Some(if decoded.width == width && decoded.height == height {
                decoded.rgba
            } else {
                resize_rgba(&decoded.rgba, decoded.width, decoded.height, width, height)
            })
        }
        None => None,
    };
    Ok(TrayPixels {
        icon,
        light,
        width,
        height,
        scale: if platform::IS_MACOS { 2.0 } else { 1.0 },
    })
}

/// Area-average resize (each output pixel averages the source area it covers),
/// in premultiplied alpha so transparent edges do not darken.
fn resize_rgba(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let mut out = vec![0u8; width as usize * height as usize * 4];
    let scale_x = f64::from(source_width) / f64::from(width);
    let scale_y = f64::from(source_height) / f64::from(height);
    for oy in 0..height {
        let y0 = f64::from(oy) * scale_y;
        let y1 = y0 + scale_y;
        for ox in 0..width {
            let x0 = f64::from(ox) * scale_x;
            let x1 = x0 + scale_x;
            let mut acc = [0.0f64; 4];
            let mut total = 0.0f64;
            let mut y = y0.floor() as u32;
            while f64::from(y) < y1 && y < source_height {
                let weight_y = y1.min(f64::from(y + 1)) - y0.max(f64::from(y));
                let mut x = x0.floor() as u32;
                while f64::from(x) < x1 && x < source_width {
                    let weight_x = x1.min(f64::from(x + 1)) - x0.max(f64::from(x));
                    let weight = weight_x * weight_y;
                    let at = (y as usize * source_width as usize + x as usize) * 4;
                    let pixel = &source[at..at + 4];
                    let alpha = f64::from(pixel[3]) / 255.0;
                    acc[0] += f64::from(pixel[0]) * alpha * weight;
                    acc[1] += f64::from(pixel[1]) * alpha * weight;
                    acc[2] += f64::from(pixel[2]) * alpha * weight;
                    acc[3] += alpha * weight;
                    total += weight;
                    x += 1;
                }
                y += 1;
            }
            let at = (oy as usize * width as usize + ox as usize) * 4;
            let alpha = if total > 0.0 { acc[3] / total } else { 0.0 };
            if alpha <= 0.0 {
                continue;
            }
            for channel in 0..3 {
                let value = (acc[channel] / total / alpha).clamp(0.0, 255.0).round();
                out[at + channel] = value as u8;
            }
            let value = (alpha * 255.0).clamp(0.0, 255.0).round();
            out[at + 3] = value as u8;
        }
    }
    out
}

/// The platform items of a menu: ids and flags as they are (pure, testable).
fn tray_items(items: &[TrayItem<'_>]) -> Vec<platform::TrayItem> {
    items
        .iter()
        .map(|item| {
            let mut native =
                platform::TrayItem::new(item.title, i32::try_from(item.id).unwrap_or(i32::MAX));
            native.separator = item.separator;
            native.checked = item.checked;
            native.disabled = item.disabled;
            native
        })
        .collect()
}

/// Adds or updates the tray icon / status bar item and its menu.
///
/// From then on the app behaves like other tray / menu bar apps: its Dock icon
/// and menu bar (macOS) show only while one of its normal windows is open or
/// minimized; hide the main window and the app lives in the tray. (On Windows
/// a hidden window has no taskbar button anyway.)
pub fn set_tray(options: TrayOptions<'_>) -> Result<(), TrayError> {
    let pixels = tray_pixels(options.icon, options.icon_light)?;
    let items = tray_items(options.items);
    let created = platform::set_tray(
        &pixels.icon,
        pixels.light.as_deref(),
        pixels.width,
        pixels.height,
        pixels.scale,
        options.template,
        Some(options.tooltip),
        &items,
    );
    if created {
        Ok(())
    } else {
        Err(TrayError::Unavailable)
    }
}

/// Changes only the icon and tooltip of the tray icon [`set_tray`] added, e.g.
/// a "syncing" state while work runs. Cheap, and an open menu stays open.
pub fn set_tray_icon(options: TrayIconOptions<'_>) -> Result<(), TrayError> {
    let pixels = tray_pixels(options.icon, options.icon_light)?;
    let updated = platform::set_tray_icon(
        &pixels.icon,
        pixels.light.as_deref(),
        pixels.width,
        pixels.height,
        pixels.scale,
        options.template,
        Some(options.tooltip),
    );
    if updated {
        Ok(())
    } else {
        Err(TrayError::Unavailable)
    }
}

/// Removes the tray / status bar item.
pub fn remove_tray() {
    platform::remove_tray();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
        let image = Image::from_pixels(rgba, width, height, width * 4).expect("image");
        image.encode_png().expect("png")
    }

    #[test]
    fn tray_items_keep_their_ids_and_flags() {
        let items = [
            TrayItem {
                id: 1,
                title: "Open",
                ..Default::default()
            },
            TrayItem {
                id: 0,
                separator: true,
                ..Default::default()
            },
            TrayItem {
                id: 2,
                title: "Pause",
                checked: true,
                disabled: true,
                ..Default::default()
            },
        ];
        let native = tray_items(&items);
        assert_eq!(native.len(), 3);
        assert_eq!(native[0].id, 1);
        assert_eq!(
            native[0]
                .title
                .as_ref()
                .map(|title| title.to_str().unwrap()),
            Some("Open")
        );
        assert!(native[1].separator);
        assert_eq!(native[1].id, 0);
        assert_eq!(native[2].id, 2);
        assert!(native[2].checked && native[2].disabled);
    }

    #[test]
    fn resize_averages_in_premultiplied_alpha() {
        // 2×1: opaque red and fully transparent → the result keeps the colour
        // (a straight-alpha average would darken it) with half the alpha
        let source = [255, 0, 0, 255, 0, 0, 0, 0];
        let resized = resize_rgba(&source, 2, 1, 1, 1);
        assert_eq!(resized, vec![255, 0, 0, 128]);
        // a 1:1 resize is the source
        assert_eq!(resize_rgba(&source, 2, 1, 2, 1), source);
    }

    #[test]
    fn tray_pixels_scale_the_icon_to_the_system_size() {
        let bytes = png(
            2,
            2,
            &[
                10, 20, 30, 255, 10, 20, 30, 255, 10, 20, 30, 255, 10, 20, 30, 255,
            ],
        );
        let pixels = tray_pixels(&bytes, None).expect("pixels");
        let system = platform::tray_icon_pixels();
        assert_eq!(pixels.height, system);
        assert_eq!(pixels.width, system, "a square icon stays square");
        assert_eq!(pixels.scale, if platform::IS_MACOS { 2.0 } else { 1.0 });
        assert_eq!(pixels.icon.len(), system as usize * system as usize * 4);
        // a non-square icon keeps its aspect ratio (width from the height)
        let wide = png(4, 2, &[0u8; 4 * 4 * 2]);
        let pixels = tray_pixels(&wide, None).expect("pixels");
        assert_eq!(pixels.width, system * 2);
        assert_eq!(pixels.height, system);
    }

    #[test]
    fn a_light_icon_is_scaled_to_the_icon_size() {
        let bytes = png(2, 2, &[255u8; 16]);
        let pixels = tray_pixels(&bytes, Some(&bytes)).expect("pixels");
        let light = pixels.light.expect("light icon");
        assert_eq!(
            light.len(),
            pixels.width as usize * pixels.height as usize * 4
        );
        assert!(light.iter().all(|byte| *byte == 255));
    }

    #[test]
    fn invalid_icon_bytes_are_rejected() {
        let _guard = crate::platform::platform::native_test_lock();
        assert_eq!(
            tray_pixels(b"not a png", None).err(),
            Some(TrayError::InvalidIcon)
        );
        let empty = TrayIconOptions {
            icon: b"",
            icon_light: None,
            template: false,
            tooltip: "",
        };
        assert_eq!(set_tray_icon(empty).err(), Some(TrayError::InvalidIcon));
        remove_tray();
    }
}
