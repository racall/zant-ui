//! Desktop integration for apps that live beside the user's other work: extra
//! windows (a quick-access panel), the tray icon (Windows notification area) /
//! status bar item (macOS menu bar), system-wide hotkeys, hiding to the tray,
//! quitting, launch at login and noticing clipboard changes.
//!
//! The app calls these from its hooks:
//!
//! - [`App::on_start`](crate::ui::app::App::on_start): open a panel with
//!   [`open_window`], add the tray icon with [`set_tray`], register hotkeys,
//!   start background work;
//! - [`App::on_app_event`](crate::ui::app::App::on_app_event): tray clicks /
//!   menu items, hotkey presses, Dock clicks, clipboard changes;
//! - [`App::on_exit`](crate::ui::app::App::on_exit): stop workers and save.
//!
//! ```ignore
//! use zant::desktop;
//!
//! fn on_start(&mut self) {
//!     self.panel = desktop::open_window(
//!         desktop::WindowHooks::new(|| render_panel()),
//!         desktop::OpenOptions {
//!             title: String::from("History"),
//!             window: desktop::WindowOptions {
//!                 style: desktop::WindowStyle::Panel,
//!                 width: 360.0,
//!                 height: 480.0,
//!                 hidden: true,
//!                 ..Default::default()
//!             },
//!             ..Default::default()
//!         },
//!     )
//!     .ok();
//!     if let Some(panel) = &self.panel {
//!         panel.place(desktop::Place::TrayCorner, 12.0);
//!         panel.activate();
//!     }
//!     let _ = desktop::set_tray(desktop::TrayOptions {
//!         icon: tray_icon(),
//!         tooltip: "My App",
//!         items: &[
//!             desktop::TrayItem { id: 1, title: "Open", ..Default::default() },
//!             desktop::TrayItem { separator: true, ..Default::default() },
//!             desktop::TrayItem { id: 2, title: "Quit", ..Default::default() },
//!         ],
//!         ..Default::default()
//!     });
//!     let _ = desktop::register_hotkey(
//!         1,
//!         desktop::Shortcut::parse("meta+shift+v").unwrap(),
//!     );
//! }
//! ```
//!
//! All windows render the same application state; an action in any window
//! rebuilds all of them. Platform differences stay behind the platform layer
//! (nothing here is `cfg(windows)`-specific).
#![forbid(unsafe_code)]

pub mod app;
pub mod app_event;
pub mod clipboard;
pub mod hotkey;
pub mod screen;
pub mod tray;
pub mod window;

pub use app::{
    LaunchAtLoginError, LoginItem, device_name, downloads_dir, launch_at_login,
    launch_at_login_supported, launched_at_login, open_login_item_settings, set_launch_at_login,
    system_dark_mode,
};
pub use app_event::AppEvent;
pub use clipboard::{
    clipboard_change_count, clipboard_files, clipboard_image, clipboard_text, set_clipboard_files,
    set_clipboard_image, set_clipboard_text, watch_clipboard,
};
pub use hotkey::{Shortcut, register_hotkey, unregister_hotkey};
pub use screen::{
    OcrLine, OcrToken, ScreenCapture, ScreenInfo, WindowInfo, capture_screens, local_utc_offset,
    ocr_available, ocr_recognize, open_screen_capture_settings, request_screen_capture_access,
    screen_capture_access, screen_capture_requested, screens, visible_windows,
};
pub use tray::{
    TrayError, TrayIconOptions, TrayItem, TrayOptions, remove_tray, set_tray, set_tray_icon,
};
pub use window::{
    IS_MACOS, OpenOptions, Place, Window, WindowHooks, invalidate_all, main_window, open_window,
    quit, rebuild_only_this_window, relaunch, set_main_runtime, single_instance,
};

/// The window options of `open_window` (re-exported so apps do not import the
/// platform layer).
pub use crate::platform::platform::{WindowOptions, WindowStyle};

/// Installs the app-event and exit handlers of `ui::app::run` (internal).
pub(crate) fn install_app_hooks(app: crate::runtime::window::SharedApp) {
    app_event::install(app);
}

/// Stops delivering app events after the event loop ended.
pub(crate) fn clear_app_hooks() {
    app_event::clear();
}

#[cfg(test)]
mod tests {
    //! The public surface of `zant::desktop` (compile-time contract checks).

    #[test]
    fn the_public_desktop_api_is_importable() {
        use super::*;
        let _: fn(bool) = watch_clipboard;
        let _: fn() -> String = clipboard_text;
        let _: fn() -> Option<Vec<u8>> = clipboard_image;
        let _: fn() -> Option<String> = clipboard_files;
        let _: fn(&str) -> bool = set_clipboard_text;
        let _: fn(&str) -> bool = set_clipboard_files;
        let _: fn(&[u8]) -> bool = set_clipboard_image;
        let _: fn() -> u64 = clipboard_change_count;
        let _: fn(u32, Shortcut) -> bool = register_hotkey;
        let _: fn(u32) = unregister_hotkey;
        let _: fn(TrayOptions<'_>) -> Result<(), TrayError> = set_tray;
        let _: fn(TrayIconOptions<'_>) -> Result<(), TrayError> = set_tray_icon;
        let _: fn() = remove_tray;
        let _: fn() -> Option<Window> = main_window;
        let _: fn(
            WindowHooks,
            OpenOptions,
        ) -> Result<Window, crate::platform::platform::PlatformError> = open_window;
        let _: fn() = invalidate_all;
        let _: fn() = quit;
        let _: fn(&str) -> bool = single_instance;
        let _: fn(&str) -> LoginItem = launch_at_login;
        let _: fn(&str, bool) -> Result<(), LaunchAtLoginError> = set_launch_at_login;
        let _: fn() -> bool = launched_at_login;
        let _: fn() -> bool = launch_at_login_supported;
        let _: fn() = open_login_item_settings;
        let _: fn() -> bool = screen_capture_requested;
        let _: fn() = open_screen_capture_settings;
        let _: fn() -> String = device_name;
        let _: fn() -> Option<String> = downloads_dir;
        let _: fn() -> bool = system_dark_mode;
        let _ = AppEvent::TrayClick;
        let _ = Place::TrayCorner;
        let _ = IS_MACOS;
    }
}
