//! Safe wrappers over the platform C ABI (`platform.h` in
//! `zant-sys/native/platform/`, implemented by `macos/window.m` and
//! `windows/window.c`):
//! a main window plus optional extra windows, the `ZantHost` callback table
//! forwarded to a Rust [`HostHandler`], window services, clipboard, tray /
//! status bar item, global hotkeys, launch-at-login and native dialogs.
//!
//! The C callback table (`ZantHost`) is an `extern "C"` trampoline that
//! forwards to a `Box<dyn HostHandler>` stored next to the window.

mod app;
mod clipboard;
mod events;
mod system;
mod tray;
mod window;
mod window_options;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use app::assert_app_released;
pub(crate) use app::release_app;
pub use app::{
    AppEventHandler, clear_app_event_handler, install_app_event_handler, launch_at_login_get,
    launch_at_login_set, launch_at_login_supported, launched_at_login, open_login_item_settings,
    quit, register_hotkey, set_app_handler, single_instance, unregister_hotkey,
};
pub use clipboard::{
    clipboard_change_count, clipboard_files, clipboard_image_png, clipboard_privacy,
    clipboard_text, set_clipboard, set_clipboard_files, set_clipboard_image_png, watch_clipboard,
};
pub use events::{
    AppEvent, Event, app_event_type, event_command, event_has, event_text, event_type, hit, key,
    modifiers, pointer_button,
};
pub use system::{
    device_name, downloads_dir, getenv, monotonic_ms, monotonic_us, reduce_motion, system_dark_mode,
};
#[cfg(test)]
pub(crate) use tests::native_test_lock;
pub use tray::{TrayItem, remove_tray, set_tray, set_tray_icon, tray_icon_pixels};
pub use window::{
    Cursor, HostHandler, Place, PlatformError, Window, post_wake, set_post_wake_window,
};
pub use window_options::{
    CAPTION_BUTTON_WIDTH, TitlebarInsets, WindowOptions, WindowStyle, native_window_options,
    titlebar_insets,
};

pub const IS_MACOS: bool = cfg!(target_os = "macos");
pub const IS_WINDOWS: bool = cfg!(target_os = "windows");
pub const SUPPORTED: bool = IS_MACOS || IS_WINDOWS;
