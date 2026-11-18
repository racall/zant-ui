//! Native window and app event types, constants, and event accessors.

use zant_sys::platform as ffi;

use super::IS_MACOS;

/// `ZantEvent.type` values (ZANT_EVENT_*).
pub mod event_type {
    use zant_sys::platform as ffi;
    pub const POINTER_MOVE: i32 = ffi::ZANT_EVENT_POINTER_MOVE as i32;
    pub const POINTER_DOWN: i32 = ffi::ZANT_EVENT_POINTER_DOWN as i32;
    pub const POINTER_UP: i32 = ffi::ZANT_EVENT_POINTER_UP as i32;
    pub const POINTER_ENTER: i32 = ffi::ZANT_EVENT_POINTER_ENTER as i32;
    pub const POINTER_LEAVE: i32 = ffi::ZANT_EVENT_POINTER_LEAVE as i32;
    pub const KEY_DOWN: i32 = ffi::ZANT_EVENT_KEY_DOWN as i32;
    pub const KEY_UP: i32 = ffi::ZANT_EVENT_KEY_UP as i32;
    pub const TEXT_INPUT: i32 = ffi::ZANT_EVENT_TEXT_INPUT as i32;
    pub const COMPOSITION: i32 = ffi::ZANT_EVENT_COMPOSITION as i32;
    pub const WINDOW_RESIZE: i32 = ffi::ZANT_EVENT_WINDOW_RESIZE as i32;
    pub const WINDOW_FOCUS: i32 = ffi::ZANT_EVENT_WINDOW_FOCUS as i32;
    pub const WINDOW_BLUR: i32 = ffi::ZANT_EVENT_WINDOW_BLUR as i32;
    pub const WINDOW_CLOSE: i32 = ffi::ZANT_EVENT_WINDOW_CLOSE as i32;
    pub const SCROLL: i32 = ffi::ZANT_EVENT_SCROLL as i32;
    pub const WINDOW_CLOSE_REQUEST: i32 = ffi::ZANT_EVENT_WINDOW_CLOSE_REQUEST as i32;
    pub const APPEARANCE_CHANGE: i32 = ffi::ZANT_EVENT_APPEARANCE_CHANGE as i32;
}

/// `ZantEvent.key` values (ZANT_KEY_*).
pub mod key {
    use zant_sys::platform as ffi;
    pub const LEFT: i32 = ffi::ZANT_KEY_LEFT as i32;
    pub const RIGHT: i32 = ffi::ZANT_KEY_RIGHT as i32;
    pub const UP: i32 = ffi::ZANT_KEY_UP as i32;
    pub const DOWN: i32 = ffi::ZANT_KEY_DOWN as i32;
    pub const HOME: i32 = ffi::ZANT_KEY_HOME as i32;
    pub const END: i32 = ffi::ZANT_KEY_END as i32;
    pub const BACKSPACE: i32 = ffi::ZANT_KEY_BACKSPACE as i32;
    pub const DELETE: i32 = ffi::ZANT_KEY_DELETE as i32;
    pub const ESCAPE: i32 = ffi::ZANT_KEY_ESCAPE as i32;
    pub const TAB: i32 = ffi::ZANT_KEY_TAB as i32;
    pub const ENTER: i32 = ffi::ZANT_KEY_ENTER as i32;
    pub const PAGE_UP: i32 = ffi::ZANT_KEY_PAGE_UP as i32;
    pub const PAGE_DOWN: i32 = ffi::ZANT_KEY_PAGE_DOWN as i32;
    pub const SPACE: i32 = ffi::ZANT_KEY_SPACE as i32;
    /// F1 … F12 = F1 + 0 … 11
    pub const F1: i32 = ffi::ZANT_KEY_F1 as i32;
    pub const F12: i32 = ffi::ZANT_KEY_F12 as i32;
}

/// `ZantEvent.modifiers` bits (ZANT_MOD_*).
pub mod modifiers {
    use zant_sys::platform as ffi;
    pub const SHIFT: i32 = ffi::ZANT_MOD_SHIFT as i32;
    pub const CONTROL: i32 = ffi::ZANT_MOD_CONTROL as i32;
    pub const ALT: i32 = ffi::ZANT_MOD_ALT as i32;
    pub const META: i32 = ffi::ZANT_MOD_META as i32;
    pub const CAPS_LOCK: i32 = ffi::ZANT_MOD_CAPS_LOCK as i32;
}

/// `ZantEvent.button` values (ZANT_POINTER_BUTTON_*).
pub mod pointer_button {
    use zant_sys::platform as ffi;
    pub const LEFT: i32 = ffi::ZANT_POINTER_BUTTON_LEFT as i32;
    pub const RIGHT: i32 = ffi::ZANT_POINTER_BUTTON_RIGHT as i32;
    pub const MIDDLE: i32 = ffi::ZANT_POINTER_BUTTON_MIDDLE as i32;
}

/// Answers of `HostHandler::titlebar_hit` (ZANT_HIT_*).
pub mod hit {
    use zant_sys::platform as ffi;
    pub const CLIENT: i32 = ffi::ZANT_HIT_CLIENT as i32;
    pub const CAPTION: i32 = ffi::ZANT_HIT_CAPTION as i32;
    pub const MAXIMIZE: i32 = ffi::ZANT_HIT_MAXIMIZE as i32;
}

/// `ZantAppEvent.type` values (ZANT_APP_EVENT_*).
pub mod app_event_type {
    use zant_sys::platform as ffi;
    pub const TRAY_CLICK: i32 = ffi::ZANT_APP_EVENT_TRAY_CLICK as i32;
    pub const TRAY_MENU: i32 = ffi::ZANT_APP_EVENT_TRAY_MENU as i32;
    pub const HOTKEY: i32 = ffi::ZANT_APP_EVENT_HOTKEY as i32;
    pub const REOPEN: i32 = ffi::ZANT_APP_EVENT_REOPEN as i32;
    pub const CLIPBOARD: i32 = ffi::ZANT_APP_EVENT_CLIPBOARD as i32;
    pub const EXIT: i32 = ffi::ZANT_APP_EVENT_EXIT as i32;
}

/// The raw platform event handed to `HostHandler::event`.
pub type Event = ffi::ZantEvent;

/// An app-level event (tray menu, global hotkey, clipboard change, ...).
pub type AppEvent = ffi::ZantAppEvent;

/// UTF-8 text of a `text_input` / `composition` event (empty when absent).
pub fn event_text(event: &Event) -> &str {
    if event.text.is_null() || event.text_len == 0 {
        return "";
    }
    // SAFETY: the native side guarantees `text` points at `text_len` bytes that
    // stay valid for the duration of the event callback (platform.h).
    let bytes = unsafe { std::slice::from_raw_parts(event.text.cast::<u8>(), event.text_len) };
    std::str::from_utf8(bytes).unwrap_or("")
}

/// True when the event's modifier bits contain `modifier` (`ZANT_MOD_*`).
pub fn event_has(event: &Event, modifier: i32) -> bool {
    (event.modifiers & modifier) != 0
}

/// The platform-native command modifier: ⌘ on macOS, Ctrl on Windows.
pub fn event_command(event: &Event) -> bool {
    if IS_MACOS {
        event_has(event, modifiers::META)
    } else {
        event_has(event, modifiers::CONTROL)
    }
}
