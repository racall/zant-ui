//! Launch at login and the system queries: the device name, the Downloads
//! folder and the system dark mode.
#![forbid(unsafe_code)]

use crate::platform::platform;

/// Whether the system starts the app at login.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginItem {
    Off,
    On,
    /// Registered, but the system does not start it until the user allows it
    /// in its settings ([`open_login_item_settings`]): macOS Login Items
    /// (new, or turned off there), Windows Task Manager / Settings > Apps >
    /// Startup (turned off there).
    NeedsApproval,
}

/// Errors of [`set_launch_at_login`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LaunchAtLoginError {
    /// An unpackaged / debug `zant run` build, or macOS before 13
    /// ([`launch_at_login_supported`]).
    #[error("launch at login is not possible here")]
    Unsupported,
    #[error("launch at login failed")]
    Failed,
}

/// Maps the platform answer (0 off, 1 on, 2 needs approval).
fn login_item(code: i32) -> LoginItem {
    match code {
        1 => LoginItem::On,
        2 => LoginItem::NeedsApproval,
        _ => LoginItem::Off,
    }
}

/// Maps the platform answer of `launch_at_login_set` (1 done, 0 failed,
/// -1 not possible).
fn launch_at_login_result(code: i32) -> Result<(), LaunchAtLoginError> {
    match code {
        1 => Ok(()),
        -1 => Err(LaunchAtLoginError::Unsupported),
        _ => Err(LaunchAtLoginError::Failed),
    }
}

/// The Downloads folder, or `None` when the platform did not answer (the
/// folder is unknown).
fn downloads_dir_or_none(dir: String) -> Option<String> {
    (!dir.is_empty()).then_some(dir)
}

/// Whether the system starts the app at login (read from the system each time:
/// the user can change it in System Settings / Task Manager too).
///
/// `app_id`: the bundle id (Windows names its Run entry with it).
pub fn launch_at_login(app_id: &str) -> LoginItem {
    login_item(platform::launch_at_login_get(app_id))
}

/// Turns launch at login on or off, the system's own way: macOS SMAppService
/// (the app's entry under Login Items; the app must be packaged, `zant
/// package`, and macOS 13 or newer), Windows the user's Run key (the app gets
/// `--zant-login`; turning it on also clears a "Disabled" set in Task
/// Manager). On macOS the system may still wait for the user's approval: read
/// [`launch_at_login`] afterwards. [`LaunchAtLoginError::Unsupported`] where
/// [`launch_at_login_supported`] is false.
pub fn set_launch_at_login(app_id: &str, on: bool) -> Result<(), LaunchAtLoginError> {
    launch_at_login_result(platform::launch_at_login_set(app_id, on))
}

/// Launch at login can be set by this build: a packaged app (macOS 13+), a
/// release build on Windows. False for `zant run`, so an app can say why the
/// switch is off instead of offering it.
pub fn launch_at_login_supported() -> bool {
    platform::launch_at_login_supported()
}

/// Opens the system's list of apps that start at login (macOS System
/// Settings > General > Login Items, Windows Settings > Apps > Startup),
/// where the user allows a [`LoginItem::NeedsApproval`] entry.
pub fn open_login_item_settings() {
    platform::open_login_item_settings();
}

/// The system opened the app at login. Windows created with
/// `.hidden_at_login` stay hidden then (a tray app starts in the background);
/// on macOS this is known once the event loop runs, so read it in actions and
/// events, not before `ui::app::run`.
pub fn launched_at_login() -> bool {
    platform::launched_at_login()
}

/// The computer's name as the user sees it ("Brian's MacBook Pro" on macOS,
/// the computer name on Windows).
pub fn device_name() -> String {
    platform::device_name()
}

/// The system uses the dark appearance (macOS Dark Mode, Windows app mode
/// Dark). Windows rebuild when it changes, so a theme hook can follow it:
///
/// ```ignore
/// fn theme(&self) -> Option<Theme> {
///     Some(if zant::desktop::system_dark_mode() {
///         zant::theme::tokens::ant_dark()
///     } else {
///         zant::theme::tokens::ant_light()
///     })
/// }
/// ```
pub fn system_dark_mode() -> bool {
    platform::system_dark_mode()
}

/// The user's Downloads folder, or `None` when unknown (Windows uses the
/// system-recorded location, so a moved folder is still right).
pub fn downloads_dir() -> Option<String> {
    downloads_dir_or_none(platform::downloads_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_codes_map_to_login_items() {
        assert_eq!(login_item(0), LoginItem::Off);
        assert_eq!(login_item(1), LoginItem::On);
        assert_eq!(login_item(2), LoginItem::NeedsApproval);
        assert_eq!(login_item(9), LoginItem::Off, "unknown codes are off");
    }

    #[test]
    fn launch_at_login_set_results_map_to_errors() {
        assert_eq!(launch_at_login_result(1), Ok(()));
        assert_eq!(
            launch_at_login_result(-1),
            Err(LaunchAtLoginError::Unsupported)
        );
        assert_eq!(launch_at_login_result(0), Err(LaunchAtLoginError::Failed));
        assert_eq!(launch_at_login_result(7), Err(LaunchAtLoginError::Failed));
    }

    #[test]
    fn an_unknown_downloads_folder_is_none() {
        assert_eq!(downloads_dir_or_none(String::new()), None);
        assert_eq!(
            downloads_dir_or_none(String::from("/Users/me/Downloads")),
            Some(String::from("/Users/me/Downloads"))
        );
    }

    #[test]
    fn the_system_queries_are_safe_without_a_window() {
        let _guard = crate::platform::platform::native_test_lock();
        let _ = launched_at_login();
        let _ = device_name();
        let _ = system_dark_mode();
        let _ = downloads_dir();
    }
}
