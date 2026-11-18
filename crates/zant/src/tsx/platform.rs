//! The desktop functions generated TSX code imports from `"zant"`
//! (docs/tsx.md "桌面函数"):
//!
//! ```ignore
//! import { clipboard, app, mainWindow, wakeAfter, keyLabel, isMacos } from "zant";
//! clipboard.setText(entry.text);   app.quit();   mainWindow.show();
//! ```
//!
//! TS-friendly wrappers of [`crate::desktop`], [`crate::core::paths`] and
//! [`crate::ui`]: strings come back as owned `Str` values, failures read as `""` / `None` / `false`.
//!
//! Generated code names: `tsx::platform::clipboard::text()`,
//! `tsx::platform::app::quit()`, `tsx::platform::main_window::show()`,
//! `tsx::platform::wake_after(ms)`, `tsx::platform::key_label("mod+s")`,
//! `tsx::platform::IS_MACOS`.

use std::cell::RefCell;
use std::rc::Rc;

use crate::Str;
use crate::desktop;
use crate::image::Image;
use crate::tsx::Bytes;

/// The host is macOS (a constant for TSX).
pub const IS_MACOS: bool = desktop::IS_MACOS;
/// The host is Windows.
pub const IS_WINDOWS: bool = crate::platform::platform::IS_WINDOWS;

thread_local! {
    /// The app's bundle id, set by `tsx::app::run`.
    static APP_ID: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Sets the app's bundle id (`tsx::app::run` does).
pub fn set_app_id(id: &str) {
    APP_ID.with(|current| *current.borrow_mut() = Some(id.to_owned()));
}

/// The app's bundle id, `None` before the app started.
pub fn app_id() -> Option<String> {
    APP_ID.with(|current| current.borrow().clone())
}

/// `"off"` / `"on"` / `"needsApproval"` (macOS: waiting in System Settings).
pub mod app {
    use super::*;
    use crate::desktop::LoginItem;

    /// `app.quit()`.
    pub fn quit() {
        desktop::quit();
    }

    /// `app.launchAtLogin()`.
    pub fn launch_at_login() -> Str {
        let id = super::app_id().unwrap_or_default();
        match desktop::launch_at_login(&id) {
            LoginItem::Off => Str::Static("off"),
            LoginItem::On => Str::Static("on"),
            LoginItem::NeedsApproval => Str::Static("needsApproval"),
        }
    }

    /// `app.setLaunchAtLogin(on)`: false when the system refused (on macOS a
    /// `zant run` build cannot).
    pub fn set_launch_at_login(on: bool) -> bool {
        let id = super::app_id().unwrap_or_default();
        desktop::set_launch_at_login(&id, on).is_ok()
    }

    /// `app.launchedAtLogin()`.
    pub fn launched_at_login() -> bool {
        desktop::launched_at_login()
    }

    /// `app.canLaunchAtLogin()`: false for `zant run` builds.
    pub fn can_launch_at_login() -> bool {
        desktop::launch_at_login_supported()
    }

    /// `app.openLoginItemSettings()`.
    pub fn open_login_item_settings() {
        desktop::open_login_item_settings();
    }

    /// `app.deviceName()`.
    pub fn device_name() -> Str {
        Str::from(desktop::device_name())
    }

    /// `app.downloadsDir()`: `None` when the folder is unknown.
    pub fn downloads_dir() -> Option<Str> {
        desktop::downloads_dir().map(Str::from)
    }

    /// `app.dataDir()`: the app's own data folder (`zant.paths.appDataDir`
    /// with the bundle id); `None` before the app started (the app id
    /// is unset).
    pub fn data_dir() -> Option<Str> {
        let id = super::app_id()?;
        crate::core::paths::app_data_dir(&id).map(|path| Str::copy(&path.to_string_lossy()))
    }
}

/// `clipboard.*` (`import { clipboard } from "zant"`).
pub mod clipboard {
    use super::*;

    /// The text on the clipboard ("" when it holds none).
    pub fn text() -> Str {
        Str::from(crate::desktop::clipboard_text())
    }

    pub fn set_text(value: &str) -> bool {
        crate::desktop::set_clipboard_text(value)
    }

    /// PNG bytes, `None` when the clipboard holds no image.
    pub fn image() -> Option<Bytes> {
        crate::desktop::clipboard_image().map(Bytes::from)
    }

    /// Accepts `&Bytes`, `&[u8]` and `&Vec<u8>`: generated code passes the
    /// bytes it got from [`image`] or an imported asset.
    pub fn set_image(png: impl AsRef<[u8]>) -> bool {
        crate::desktop::set_clipboard_image(png.as_ref())
    }

    /// Copied files and folders: absolute paths, one per line; `None` for none.
    pub fn files() -> Option<Str> {
        crate::desktop::clipboard_files().map(Str::from)
    }

    pub fn set_files(lines: &str) -> bool {
        crate::desktop::set_clipboard_files(lines)
    }

    /// Grows with every change (also the app's own writes).
    pub fn change_count() -> f64 {
        crate::desktop::clipboard_change_count() as f64
    }
}

/// The app's main window (the root component's).
pub mod main_window {
    /// `mainWindow.show()`.
    pub fn show() {
        if let Some(window) = crate::desktop::main_window() {
            window.show();
        }
    }

    /// `mainWindow.hide()`.
    pub fn hide() {
        if let Some(window) = crate::desktop::main_window() {
            window.hide();
        }
    }

    /// `mainWindow.activate()`.
    pub fn activate() {
        if let Some(window) = crate::desktop::main_window() {
            window.activate();
        }
    }

    /// `mainWindow.focus("search")`: focus the node with that key after the
    /// next layout.
    pub fn focus(key: &str) {
        if let Some(window) = crate::desktop::main_window() {
            window.focus(key);
        }
    }

    /// `mainWindow.setTitle(title)`.
    pub fn set_title(title: &str) {
        if let Some(window) = crate::desktop::main_window() {
            window.set_title(title);
        }
    }

    /// `mainWindow.setSize(width, height)`: the content size in points.
    pub fn set_size(width: f64, height: f64) {
        if let Some(window) = crate::desktop::main_window() {
            window.set_size(width as f32, height as f32);
        }
    }

    pub fn set_min_size(width: f64, height: f64) {
        if let Some(window) = crate::desktop::main_window() {
            window.set_min_size(width as f32, height as f32);
        }
    }

    /// 0 in a dimension leaves it unlimited.
    pub fn set_max_size(width: f64, height: f64) {
        if let Some(window) = crate::desktop::main_window() {
            window.set_max_size(width as f32, height as f32);
        }
    }

    pub fn set_always_on_top(on: bool) {
        if let Some(window) = crate::desktop::main_window() {
            window.set_always_on_top(on);
        }
    }

    pub fn set_fullscreen(on: bool) {
        if let Some(window) = crate::desktop::main_window() {
            window.set_fullscreen(on);
        }
    }

    pub fn fullscreen() -> bool {
        crate::desktop::main_window().is_some_and(|window| window.is_fullscreen())
    }

    pub fn visible() -> bool {
        crate::desktop::main_window().is_some_and(|window| window.is_visible())
    }

    pub fn active() -> bool {
        crate::desktop::main_window()
            .is_some_and(|window| window.is_visible() && window.is_active())
    }

    pub fn minimized() -> bool {
        crate::desktop::main_window().is_some_and(|window| window.is_minimized())
    }
}

/// `keyLabel("mod+shift+s")`: the platform's label ("⇧⌘S" / "Ctrl+Shift+S").
pub fn key_label(spec: &str) -> Str {
    Str::from(crate::core::keys::label(spec))
}

/// `shortcutLabel("ctrl+cmd+v")`: an imported shortcut string as the platform
/// shows it; "" when it is not a valid shortcut.
pub fn shortcut_label(shortcut: &str) -> Str {
    match crate::desktop::Shortcut::parse(shortcut) {
        Some(parsed) => Str::from(parsed.label()),
        None => Str::default(),
    }
}

struct AssetImage {
    /// Address + length identify the static asset bytes.
    ptr: usize,
    len: usize,
    image: Option<Rc<Image>>,
}

thread_local! {
    /// Decoded image assets, cached by their static bytes.
    static ASSET_IMAGES: RefCell<Vec<AssetImage>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn release_app() {
    ASSET_IMAGES.with(|images| images.borrow_mut().clear());
    APP_ID.with(|id| *id.borrow_mut() = None);
}

#[cfg(test)]
pub(crate) fn asset_image_count() -> usize {
    ASSET_IMAGES.with(|images| images.borrow().len())
}

/// An imported image asset (`import icon from "./icon.png"`) as an
/// `ImageOptions::image` value: decoded on first use and kept (assets are
/// static bytes, cached by address); `None` when it is not a decodable image.
pub fn asset_image(bytes: &'static [u8]) -> Option<Rc<Image>> {
    let ptr = bytes.as_ptr() as usize;
    ASSET_IMAGES.with(|images| {
        let mut images = images.borrow_mut();
        if let Some(entry) = images
            .iter()
            .find(|entry| entry.ptr == ptr && entry.len == bytes.len())
        {
            return entry.image.clone();
        }
        let image = Image::from_encoded(bytes).map(Rc::new);
        images.push(AssetImage {
            ptr,
            len: bytes.len(),
            image: image.clone(),
        });
        image
    })
}

/// `wakeAfter(ms)`: rebuilds after `ms` milliseconds (call it while
/// rendering: a toast that hides itself reads `performance.now()` against its
/// deadline). The delay is clamped.
pub fn wake_after(ms: f64) {
    if ms.is_nan() || ms <= 0.0 {
        crate::ui::wake_after(0);
        return;
    }
    crate::ui::wake_after(ms.min(f64::from(u32::MAX)) as u32);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui;

    #[test]
    fn wake_after_clamps_what_ts_passes() {
        let _ = ui::take_wake();
        wake_after(-5.0);
        assert_eq!(ui::take_wake(), Some(0));
        wake_after(1e12);
        assert_eq!(ui::take_wake(), Some(u32::MAX));
        wake_after(250.7);
        assert_eq!(ui::take_wake(), Some(250));
        let _ = ui::take_wake();
    }

    #[test]
    fn the_data_folder_needs_the_environment() {
        // a fresh thread: the app id is unset
        assert_eq!(app_id(), None);
        assert!(app::data_dir().is_none());
        set_app_id("dev.zant.tsx-test");
        let dir = app::data_dir().expect("an app id is set");
        assert!(dir.contains("dev.zant.tsx-test"), "got {dir}");
    }

    #[test]
    fn labels_and_shortcuts_are_wrapped() {
        assert!(!key_label("mod+s").is_empty());
        let valid = shortcut_label("ctrl+alt+k");
        assert!(!valid.is_empty());
        assert_eq!(shortcut_label("nonsense+").as_str(), "");
        assert_eq!(IS_WINDOWS, crate::platform::platform::IS_WINDOWS);
        assert_eq!(IS_MACOS, crate::desktop::IS_MACOS);
    }
}
