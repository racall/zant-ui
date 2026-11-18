//! App lifecycle, launch at login, app event callbacks, and global hotkeys.

use std::cell::RefCell;
use std::ffi::CString;
#[cfg(test)]
use std::sync::atomic::Ordering;

use zant_sys::platform as ffi;

use super::SUPPORTED;
use super::events::AppEvent;
#[cfg(test)]
use super::window::POST_WAKE_WINDOW;
use super::window::set_post_wake_window;

/// Ends the app (all windows) without asking again.
pub fn quit() {
    if SUPPORTED {
        // SAFETY: no arguments; ends the event loop.
        unsafe { ffi::zant_app_quit() }
    }
}

/// True for the first process with `app_id`; a later one asks the first to come
/// forward and gets false.
pub fn single_instance(app_id: &str) -> bool {
    if !SUPPORTED {
        return true;
    }
    let Ok(app_id) = CString::new(app_id) else {
        return true;
    };
    // SAFETY: NUL-terminated app id.
    unsafe { ffi::zant_app_single_instance(app_id.as_ptr()) != 0 }
}

/// 0 = off, 1 = on, 2 = waiting for the user's approval (macOS).
pub fn launch_at_login_get(app_id: &str) -> i32 {
    if !SUPPORTED {
        return 0;
    }
    let Ok(app_id) = CString::new(app_id) else {
        return 0;
    };
    // SAFETY: NUL-terminated app id.
    unsafe { ffi::zant_launch_at_login_get(app_id.as_ptr()) }
}

/// The process is `zant run`'s hot-reload host (debug or `--release`): its
/// executable starts nothing without the library path `zant run` passes.
fn hot_reload_host() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.file_stem().map(|stem| stem == "zant-hot-host"))
        .unwrap_or(false)
}

/// Launch at login can be set here: a packaged macOS app (13+); on Windows
/// a release build. A debug or hot-reload `zant run` build would register a
/// development executable (or the hot-reload host) to start at every login.
pub fn launch_at_login_supported() -> bool {
    if !SUPPORTED || hot_reload_host() || (cfg!(windows) && cfg!(debug_assertions)) {
        return false;
    }
    // SAFETY: no arguments.
    unsafe { ffi::zant_launch_at_login_supported() != 0 }
}

/// 1 = done, 0 = failed, -1 = not possible here.
pub fn launch_at_login_set(app_id: &str, on: bool) -> i32 {
    if !launch_at_login_supported() {
        return -1;
    }
    let Ok(app_id) = CString::new(app_id) else {
        return -1;
    };
    // SAFETY: NUL-terminated app id.
    unsafe { ffi::zant_launch_at_login_set(app_id.as_ptr(), i32::from(on)) }
}

pub fn launched_at_login() -> bool {
    // SAFETY: no arguments.
    SUPPORTED && unsafe { ffi::zant_app_launched_at_login() != 0 }
}

/// Opens the system's list of apps that start at login.
pub fn open_login_item_settings() {
    if SUPPORTED {
        // SAFETY: no arguments.
        unsafe { ffi::zant_open_login_item_settings() }
    }
}

/// Sets the app-level event handler (`ZantAppEvent`: tray menu, hotkey, ...).
///
/// # Safety
/// `userdata` is passed through to `handler`; the caller must keep it alive
/// until a new handler (or `None`) is installed.
pub unsafe fn set_app_handler(
    handler: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *const AppEvent)>,
    userdata: *mut std::ffi::c_void,
) {
    if SUPPORTED {
        // SAFETY: the caller keeps `userdata` alive while the handler is set.
        unsafe { ffi::zant_app_set_handler(handler, userdata) }
    }
}

/// Handler of the app-level native events (`ZantAppEvent`: tray click / menu,
/// hotkey, Dock reopen, clipboard change, exit).
///
/// The native side delivers these through a single C callback; this trait is
/// the safe half (`desktop` installs it from `ui::app::run`) so the high-level
/// modules do not need `unsafe`.
pub trait AppEventHandler {
    fn event(&mut self, event: &AppEvent);
}

thread_local! {
    /// The handler installed by [`install_app_event_handler`].
    static APP_EVENT_HANDLER: RefCell<Option<Box<dyn AppEventHandler>>> =
        const { RefCell::new(None) };
    /// Includes registrations made directly through the Rust desktop API.
    static HOTKEY_IDS: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
}

pub(super) unsafe extern "C" fn app_event_trampoline(
    _userdata: *mut std::ffi::c_void,
    event: *const AppEvent,
) {
    if event.is_null() {
        return;
    }
    // SAFETY: the native side owns `event` for the duration of the callback
    // (platform.h: app-level events are delivered on the UI thread).
    let event = unsafe { &*event };
    // A handler that replaces itself from inside the callback keeps the event:
    // `try_borrow_mut` never panics on the UI thread's re-entrancy.
    let _ = APP_EVENT_HANDLER.try_with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut()
            && let Some(handler) = slot.as_mut()
        {
            handler.event(event);
        }
    });
}

/// Installs the handler of the app-level native events; called by
/// `ui::app::run`.
pub fn install_app_event_handler(handler: impl AppEventHandler + 'static) {
    APP_EVENT_HANDLER.with(|slot| *slot.borrow_mut() = Some(Box::new(handler)));
    // SAFETY: the trampoline reads the thread-local slot; it needs no userdata.
    unsafe { ffi::zant_app_set_handler(Some(app_event_trampoline), std::ptr::null_mut()) };
}

/// Stops delivering app-level events (the app is ending).
pub fn clear_app_event_handler() {
    if SUPPORTED {
        // SAFETY: installs the null handler; the trampoline is not called.
        unsafe { ffi::zant_app_set_handler(None, std::ptr::null_mut()) };
    }
    let old = APP_EVENT_HANDLER.with(|slot| slot.take());
    drop(old);
}

pub(crate) fn release_app() {
    clear_app_event_handler();
    set_post_wake_window(std::ptr::null_mut());
    let ids = HOTKEY_IDS.with(|ids| std::mem::take(&mut *ids.borrow_mut()));
    for id in ids {
        unregister_hotkey(id);
    }
}

#[cfg(test)]
pub(crate) fn assert_app_released() {
    APP_EVENT_HANDLER.with(|slot| assert!(slot.borrow().is_none()));
    HOTKEY_IDS.with(|ids| assert!(ids.borrow().is_empty()));
    assert!(POST_WAKE_WINDOW.load(Ordering::Relaxed).is_null());
}

/// System-wide hotkey; false when the combination is taken or unsupported.
pub fn register_hotkey(id: i32, key: i32, modifiers: i32) -> bool {
    let registered = SUPPORTED && unsafe { ffi::zant_hotkey_register(id, key, modifiers) != 0 };
    HOTKEY_IDS.with(|ids| {
        let mut ids = ids.borrow_mut();
        ids.retain(|&current| current != id);
        if registered {
            ids.push(id);
        }
    });
    registered
}

pub fn unregister_hotkey(id: i32) {
    HOTKEY_IDS.with(|ids| ids.borrow_mut().retain(|&current| current != id));
    if SUPPORTED {
        // SAFETY: no arguments beyond the id.
        unsafe { ffi::zant_hotkey_unregister(id) }
    }
}
