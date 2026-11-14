//! Port of `src/platform/platform.zig` — WP-1.1d.
//!
//! Safe wrappers over the platform C ABI (`src/platform/platform.h`, implemented
//! by `src/platform/macos/window.m` and `src/platform/windows/window.c`):
//! a main window plus optional extra windows, the `ZantHost` callback table
//! forwarded to a Rust [`HostHandler`], window services, clipboard, tray /
//! status bar item, global hotkeys, launch-at-login and native dialogs.
//!
//! The C callback table (`ZantHost`) is an `extern "C"` trampoline that
//! forwards to a `Box<dyn HostHandler>` stored next to the window; see
//! docs/rust-port/plan.md WP-1.1d.

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::Instant;

use crate::renderer::canvas::Canvas;
use zant_sys::platform as ffi;

pub const IS_MACOS: bool = cfg!(target_os = "macos");
pub const IS_WINDOWS: bool = cfg!(target_os = "windows");
pub const SUPPORTED: bool = IS_MACOS || IS_WINDOWS;

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

/// Errors of window creation (Zig: `error.UnsupportedPlatform` /
/// `error.WindowCreationFailed`).
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("unsupported platform")]
    UnsupportedPlatform,
    #[error("window creation failed")]
    WindowCreationFailed,
}

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
}

/// Where `place` puts a window, in the work area of the screen under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Place {
    Center = 0,
    TopLeft = 1,
    TopRight = 2,
    BottomLeft = 3,
    BottomRight = 4,
    /// By the status bar / tray icons: top right on macOS (under the menu
    /// bar), bottom right on Windows (above the taskbar, wherever it is).
    TrayCorner = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Cursor {
    Arrow = 0,
    PointingHand = 1,
    Ibeam = 2,
    Crosshair = 3,
    /// Splitters that drag left / right.
    ResizeHorizontal = 4,
    /// Splitters that drag up / down.
    ResizeVertical = 5,
    /// Pannable content (open hand).
    Grab = 6,
    /// Panning (closed hand).
    Grabbing = 7,
    NotAllowed = 8,
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

/// Application callbacks driven by the native event loop. The `ZantHost`
/// trampoline forwards to these; return `true` from `event` to consume it.
pub trait HostHandler {
    fn draw(&mut self, canvas: Canvas, width: f32, height: f32, dpr: f32);
    fn event(&mut self, event: &Event) -> bool;
    /// IME candidate window anchor in logical window coordinates.
    fn ime_rect(&mut self) -> Option<(f32, f32, f32, f32)>;
    /// Milliseconds until the next animation frame, or -1 for "no timer".
    fn next_frame_delay(&mut self) -> i32;
    /// `hit::*` for a point in logical window coordinates (app-drawn title bars).
    fn titlebar_hit(&mut self, _x: f32, _y: f32) -> i32 {
        hit::CLIENT
    }
    /// UI-thread callback after `Window::post_wake` (any thread).
    fn wake(&mut self) {}
}

/// Owns the `Box<dyn HostHandler>` the C callback table points at. Boxed so the
/// address stays stable when the `Window` moves.
struct HostBox {
    handler: Box<dyn HostHandler>,
}

unsafe extern "C" fn trampoline_draw(
    userdata: *mut std::ffi::c_void,
    canvas: *mut ffi::ZantCanvas,
    width: f32,
    height: f32,
    dpr: f32,
) {
    if userdata.is_null() {
        return;
    }
    // SAFETY: `userdata` is the `HostBox` our `create` leaked into the C table.
    let host = unsafe { &mut *(userdata as *mut HostBox) };
    host.handler
        .draw(Canvas::from_opaque(canvas.cast()), width, height, dpr);
}

unsafe extern "C" fn trampoline_event(
    userdata: *mut std::ffi::c_void,
    event: *const Event,
) -> std::os::raw::c_int {
    if userdata.is_null() || event.is_null() {
        return 0;
    }
    // SAFETY: `userdata` is our `HostBox`; the native side owns `event` for the
    // duration of the call.
    let (host, event) = unsafe { (&mut *(userdata as *mut HostBox), &*event) };
    i32::from(host.handler.event(event))
}

unsafe extern "C" fn trampoline_ime_rect(
    userdata: *mut std::ffi::c_void,
    x: *mut f32,
    y: *mut f32,
    width: *mut f32,
    height: *mut f32,
) -> std::os::raw::c_int {
    if userdata.is_null() {
        return 0;
    }
    // SAFETY: `userdata` is our `HostBox`; the out pointers are valid per the header.
    let host = unsafe { &mut *(userdata as *mut HostBox) };
    match host.handler.ime_rect() {
        Some((rect_x, rect_y, rect_width, rect_height)) => {
            // SAFETY: the native caller passes four valid out pointers.
            unsafe {
                *x = rect_x;
                *y = rect_y;
                *width = rect_width;
                *height = rect_height;
            }
            1
        }
        None => 0,
    }
}

unsafe extern "C" fn trampoline_next_frame_delay(
    userdata: *mut std::ffi::c_void,
) -> std::os::raw::c_int {
    if userdata.is_null() {
        return -1;
    }
    // SAFETY: `userdata` is our `HostBox`.
    let host = unsafe { &mut *(userdata as *mut HostBox) };
    host.handler.next_frame_delay()
}

unsafe extern "C" fn trampoline_titlebar_hit(
    userdata: *mut std::ffi::c_void,
    x: f32,
    y: f32,
) -> std::os::raw::c_int {
    if userdata.is_null() {
        return hit::CLIENT;
    }
    // SAFETY: `userdata` is our `HostBox`.
    let host = unsafe { &mut *(userdata as *mut HostBox) };
    host.handler.titlebar_hit(x, y)
}

unsafe extern "C" fn trampoline_wake(userdata: *mut std::ffi::c_void) {
    if userdata.is_null() {
        return;
    }
    // SAFETY: `userdata` is our `HostBox`.
    let host = unsafe { &mut *(userdata as *mut HostBox) };
    host.handler.wake();
}

/// `ZANT_WINDOW_SIZE=WxH` (automation): forces the logical window size so
/// `scripts/parity/capture.sh` can screenshot at fixed sizes and compare the Zig
/// and Rust builds (Zig change log in docs/rust-port/status.md).
fn parse_window_size(value: &str) -> Option<(f32, f32)> {
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

fn to_native_options(options: WindowOptions) -> ffi::ZantWindowOptions {
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

/// One native window (`window.m` on macOS, `window.c` on Windows).
pub struct Window {
    handle: *mut ffi::ZantWindow,
    /// Keeps the boxed handler (and therefore the `ZantHost.userdata` pointer)
    /// alive for as long as the native window; read by the C trampolines.
    _host: Option<Box<HostBox>>,
}

impl Window {
    pub fn create(
        title: &str,
        options: WindowOptions,
        handler: Box<dyn HostHandler>,
    ) -> Result<Window, PlatformError> {
        if !SUPPORTED {
            return Err(PlatformError::UnsupportedPlatform);
        }
        let title = CString::new(title).map_err(|_| PlatformError::WindowCreationFailed)?;
        let mut host = Box::new(HostBox { handler });
        // The box lives on the heap, so the pointer stays valid when `host` is
        // moved into the `Window` below.
        let host_ptr = (&mut *host as *mut HostBox).cast::<std::ffi::c_void>();
        let table = ffi::ZantHost {
            userdata: host_ptr,
            draw: Some(trampoline_draw),
            event: Some(trampoline_event),
            ime_rect: Some(trampoline_ime_rect),
            next_frame_delay: Some(trampoline_next_frame_delay),
            titlebar_hit: Some(trampoline_titlebar_hit),
            wake: Some(trampoline_wake),
        };
        let native = to_native_options(options);
        // Windows: the native window creation initialises Skia (the renderer
        // lives behind the platform ABI), so the static data zant-sys embeds
        // (`icudtl.dat`, STIX Two Math) must be handed to the bridge first.
        // Without this the window path skips `zant_sys::init()` — only the
        // raster renderer used by tests called it — and Skia falls back to
        // looking for `icudtl.dat` next to the exe (`check(fUnicode)` aborts
        // in SkParagraph when text is shaped). macOS has no static data.
        #[cfg(target_os = "windows")]
        zant_sys::init();
        // SAFETY: title and table are valid for the call; the native side copies
        // them (the host table is read during create only, per platform.h).
        let handle = unsafe { ffi::zant_window_create(title.as_ptr(), &native, &table) };
        if handle.is_null() {
            return Err(PlatformError::WindowCreationFailed);
        }
        // The first window is the wake target for background work; the runtime
        // may point it elsewhere for multiple windows.
        if POST_WAKE_WINDOW.load(Ordering::Relaxed).is_null() {
            set_post_wake_window(handle);
        }
        Ok(Window {
            handle,
            _host: Some(host),
        })
    }

    /// The raw pointer, for the few call sites that pass it back (runtime,
    /// snapshot wiring, desktop glue).
    pub fn raw(&self) -> *mut ffi::ZantWindow {
        self.handle
    }

    /// Runs the native event loop for all windows until it ends.
    pub fn run(&self) {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_run(self.handle) }
    }

    /// Asks for a redraw after `delay_ms` (0 = next frame).
    pub fn request_redraw(&self, delay_ms: i32) {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_request_redraw(self.handle, delay_ms) }
    }

    /// Thread-safe: asks the UI thread to run the `wake` callback once and redraw.
    pub fn post_wake(&self) {
        // SAFETY: live window handle; the native side is documented thread-safe.
        unsafe { ffi::zant_window_post_wake(self.handle) }
    }

    pub fn set_cursor(&self, cursor: Cursor) {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_set_cursor(self.handle, cursor as i32) }
    }

    /// Window background colour (0xAARRGGBB); on Windows it also picks a light
    /// or dark system title bar.
    pub fn set_background(&self, argb: u32) {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_set_background(self.handle, argb) }
    }

    /// Unsaved-changes state (dot in the close button on macOS, "*" before the
    /// title on Windows).
    pub fn set_document_edited(&self, edited: bool) {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_set_document_edited(self.handle, i32::from(edited)) }
    }

    pub fn renderer_name(&self) -> &str {
        // SAFETY: live window handle; the returned string lives as long as the
        // window (static name per backend).
        unsafe {
            let name = ffi::zant_window_renderer_name(self.handle);
            if name.is_null() {
                ""
            } else {
                CStr::from_ptr(name).to_str().unwrap_or("")
            }
        }
    }

    pub fn schedule_snapshot(&self, path: &str, delay_ms: i32) {
        let Ok(path) = CString::new(path) else {
            return;
        };
        // SAFETY: live window handle, NUL-terminated path.
        unsafe { ffi::zant_window_schedule_snapshot(self.handle, path.as_ptr(), delay_ms) }
    }

    pub fn minimize(&self) {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_minimize(self.handle) }
    }

    pub fn toggle_maximize(&self) {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_toggle_maximize(self.handle) }
    }

    pub fn is_maximized(&self) -> bool {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_is_maximized(self.handle) != 0 }
    }

    /// Asks the window to close; the close-request hook may veto it.
    pub fn close(&self) {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_close(self.handle) }
    }

    pub fn is_active(&self) -> bool {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_is_active(self.handle) != 0 }
    }

    pub fn set_visible(&self, visible: bool) {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_set_visible(self.handle, i32::from(visible)) }
    }

    pub fn is_visible(&self) -> bool {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_is_visible(self.handle) != 0 }
    }

    pub fn is_minimized(&self) -> bool {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_is_minimized(self.handle) != 0 }
    }

    /// Shows the window, brings it forward and gives it the keyboard.
    pub fn activate(&self) {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_activate(self.handle) }
    }

    pub fn place(&self, place: Place, margin: f32) {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_place(self.handle, place as i32, margin) }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: we own the window; it is destroyed before the host box
            // (the native window must not call back after this).
            unsafe { ffi::zant_window_destroy(self.handle) };
            self.handle = std::ptr::null_mut();
        }
        // the handler is dropped with `self.host`
    }
}

/// The window `post_wake` wakes (the first window created; the runtime may
/// change it later for multi-window apps). A raw pointer is stored so any
/// thread can wake the UI thread, which `zant_window_post_wake` documents as
/// thread-safe.
static POST_WAKE_WINDOW: AtomicPtr<ffi::ZantWindow> = AtomicPtr::new(std::ptr::null_mut());

/// Registers the window `platform::post_wake` targets. The runtime sets this
/// for the window it wants background work to wake.
pub fn set_post_wake_window(window: *mut ffi::ZantWindow) {
    POST_WAKE_WINDOW.store(window, Ordering::Relaxed);
}

/// Thread-safe: asks the UI thread to run the `wake` callback once and redraw
/// (Zig: `ui.postWake`). Does nothing before a window exists.
pub fn post_wake() {
    let handle = POST_WAKE_WINDOW.load(Ordering::Relaxed);
    if !handle.is_null() {
        // SAFETY: the native function is thread-safe and the pointer is the
        // live window registered by `create` / `set_post_wake_window` (the
        // window outlives the app's background work).
        unsafe { ffi::zant_window_post_wake(handle) }
    }
}

/// System "Reduce Motion" accessibility setting (no window needed).
pub fn reduce_motion() -> bool {
    // SAFETY: no arguments, no state.
    unsafe { ffi::zant_window_reduce_motion() != 0 }
}

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

/// 1 = done, 0 = failed, -1 = not possible here.
pub fn launch_at_login_set(app_id: &str, on: bool) -> i32 {
    if !SUPPORTED {
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
/// modules do not need `unsafe` (plan.md §3 rule 5).
pub trait AppEventHandler {
    fn event(&mut self, event: &AppEvent);
}

thread_local! {
    /// The handler installed by [`install_app_event_handler`].
    static APP_EVENT_HANDLER: RefCell<Option<Box<dyn AppEventHandler>>> =
        const { RefCell::new(None) };
}

unsafe extern "C" fn app_event_trampoline(
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

/// Installs the handler of the app-level native events (Zig
/// `zant.desktop.setAppEventHandler` / `setExitHandler`); called by
/// `ui::app::run`.
pub fn install_app_event_handler(handler: impl AppEventHandler + 'static) {
    APP_EVENT_HANDLER.with(|slot| *slot.borrow_mut() = Some(Box::new(handler)));
    // SAFETY: the trampoline reads the thread-local slot; it needs no userdata.
    unsafe { ffi::zant_app_set_handler(Some(app_event_trampoline), std::ptr::null_mut()) };
}

/// Stops delivering app-level events (the app is ending).
pub fn clear_app_event_handler() {
    APP_EVENT_HANDLER.with(|slot| *slot.borrow_mut() = None);
    if SUPPORTED {
        // SAFETY: installs the null handler; the trampoline is not called.
        unsafe { ffi::zant_app_set_handler(None, std::ptr::null_mut()) };
    }
}

/// Serializes the tests that call native services which macOS does not expect
/// from several threads at once (the pasteboard, the status bar, …). The
/// framework itself calls them on the UI thread only; the test harness runs
/// every test on its own thread.
#[cfg(test)]
static NATIVE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Acquires [`NATIVE_TEST_LOCK`] (a poisoned lock is reused: one failing test
/// must not break the others).
#[cfg(test)]
pub(crate) fn native_test_lock() -> std::sync::MutexGuard<'static, ()> {
    NATIVE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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

/// System-wide hotkey; false when the combination is taken or unsupported.
pub fn register_hotkey(id: i32, key: i32, modifiers: i32) -> bool {
    SUPPORTED && unsafe { ffi::zant_hotkey_register(id, key, modifiers) != 0 }
}

pub fn unregister_hotkey(id: i32) {
    if SUPPORTED {
        // SAFETY: no arguments beyond the id.
        unsafe { ffi::zant_hotkey_unregister(id) }
    }
}

/// Sends `app_event_type::CLIPBOARD` after each clipboard change.
pub fn watch_clipboard(enable: bool) {
    if SUPPORTED {
        // SAFETY: no arguments.
        unsafe { ffi::zant_clipboard_watch(i32::from(enable)) }
    }
}

/// The computer's name as the user sees it.
pub fn device_name() -> String {
    if !SUPPORTED {
        return String::new();
    }
    let mut buffer = vec![0u8; 256];
    // SAFETY: the native call writes at most `buffer.len()` bytes.
    let needed = unsafe { ffi::zant_device_name(buffer.as_mut_ptr().cast(), buffer.len()) };
    let len = needed.min(buffer.len());
    buffer.truncate(len);
    let end = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
    String::from_utf8_lossy(&buffer[..end]).into_owned()
}

/// The system uses the dark appearance.
pub fn system_dark_mode() -> bool {
    SUPPORTED && unsafe { ffi::zant_system_dark_mode() != 0 }
}

/// Changes whenever any app writes to the clipboard.
pub fn clipboard_change_count() -> u64 {
    if !SUPPORTED {
        return 0;
    }
    // SAFETY: no arguments.
    unsafe { ffi::zant_clipboard_change_count() }
}

/// Monotonic clock in microseconds.
///
/// Zig called `clock_gettime(MONOTONIC)` / `QueryPerformanceCounter`; Rust's
/// `Instant` is the std equivalent (the port only ever uses differences).
pub fn monotonic_us() -> i64 {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let epoch = EPOCH.get_or_init(Instant::now);
    i64::try_from(epoch.elapsed().as_micros()).unwrap_or(i64::MAX)
}

/// Monotonic clock in milliseconds.
pub fn monotonic_ms() -> i64 {
    monotonic_us() / 1000
}

/// Process environment variable.
pub fn getenv(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

pub fn set_clipboard(text: &str) -> bool {
    // SAFETY: valid slice for the call (the native side copies it).
    unsafe { ffi::zant_clipboard_set_text(text.as_ptr().cast(), text.len()) != 0 }
}

/// Text on the clipboard (empty when it holds no text).
pub fn clipboard_text() -> Option<String> {
    if !SUPPORTED {
        return None;
    }
    // SAFETY: no arguments; returns the required size including the NUL.
    let required = unsafe { ffi::zant_clipboard_get_text(std::ptr::null_mut(), 0) };
    if required == 0 {
        return Some(String::new());
    }
    let mut buffer = vec![0u8; required];
    // SAFETY: `buffer` has room for `required` bytes.
    unsafe { ffi::zant_clipboard_get_text(buffer.as_mut_ptr().cast(), buffer.len()) };
    let end = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
    Some(String::from_utf8_lossy(&buffer[..end]).into_owned())
}

/// PNG of the image on the clipboard, or `None` when it holds no image.
pub fn clipboard_image_png() -> Option<Vec<u8>> {
    if !SUPPORTED {
        return None;
    }
    let mut len: usize = 0;
    // SAFETY: `len` is a valid out pointer; the returned buffer is malloc'd and
    // freed once, after the copy.
    unsafe {
        let bytes = ffi::zant_clipboard_get_image_png(&mut len);
        if bytes.is_null() {
            return None;
        }
        let copy = std::slice::from_raw_parts(bytes, len).to_vec();
        ffi::zant_clipboard_free(bytes.cast());
        Some(copy)
    }
}

pub fn set_clipboard_image_png(png: &[u8]) -> bool {
    if !SUPPORTED || png.is_empty() {
        return false;
    }
    // SAFETY: valid slice for the call (the native side copies it).
    unsafe { ffi::zant_clipboard_set_image_png(png.as_ptr(), png.len()) != 0 }
}

/// Files and folders on the clipboard, paths UTF-8, one per line.
pub fn clipboard_files() -> Option<String> {
    if !SUPPORTED {
        return None;
    }
    let mut len: usize = 0;
    // SAFETY: `len` is a valid out pointer; the buffer is freed after the copy.
    unsafe {
        let bytes = ffi::zant_clipboard_get_files(&mut len);
        if bytes.is_null() {
            return None;
        }
        let copy = std::slice::from_raw_parts(bytes.cast::<u8>(), len).to_vec();
        ffi::zant_clipboard_free(bytes.cast());
        Some(String::from_utf8_lossy(&copy).into_owned())
    }
}

/// Puts files on the clipboard (paths one per line).
pub fn set_clipboard_files(paths: &str) -> bool {
    if !SUPPORTED || paths.is_empty() {
        return false;
    }
    // SAFETY: valid slice for the call (the native side copies it).
    unsafe { ffi::zant_clipboard_set_files(paths.as_ptr().cast(), paths.len()) != 0 }
}

/// The user's Downloads folder (empty when unknown).
pub fn downloads_dir() -> String {
    if !SUPPORTED {
        return String::new();
    }
    let mut buffer = vec![0u8; 4096];
    // SAFETY: the native call writes at most `buffer.len()` bytes.
    let len = unsafe { ffi::zant_downloads_dir(buffer.as_mut_ptr().cast(), buffer.len()) }
        .min(buffer.len());
    buffer.truncate(len);
    String::from_utf8_lossy(&buffer).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titlebar_insets_reserve_the_system_controls() {
        assert_eq!(
            titlebar_insets(WindowStyle::Native),
            TitlebarInsets::default()
        );
        let hidden = titlebar_insets(WindowStyle::HiddenTitlebar);
        if IS_MACOS {
            assert_eq!(hidden.left, 84.0, "traffic lights on the left");
            assert_eq!(hidden.right, 0.0);
        } else if IS_WINDOWS {
            assert_eq!(hidden.left, 0.0);
            assert_eq!(hidden.right, CAPTION_BUTTON_WIDTH * 3.0);
        }
    }

    #[test]
    fn window_size_override_parses_and_ignores_invalid_values() {
        assert_eq!(parse_window_size("900x640"), Some((900.0, 640.0)));
        assert_eq!(parse_window_size("0x640"), None);
        assert_eq!(parse_window_size("900x"), None);
        assert_eq!(parse_window_size("900"), None);
        assert_eq!(parse_window_size("abcxdef"), None);
    }

    #[test]
    fn monotonic_clock_advances() {
        let first = monotonic_us();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = monotonic_us();
        assert!(second > first);
        assert_eq!(monotonic_ms(), monotonic_us() / 1000);
    }

    #[test]
    fn the_app_event_trampoline_forwards_to_the_installed_handler() {
        #[derive(Default)]
        struct Recorder {
            events: Vec<(i32, i32)>,
        }
        impl AppEventHandler for Recorder {
            fn event(&mut self, event: &AppEvent) {
                self.events.push((event.type_, event.id));
            }
        }
        struct Shared(std::rc::Rc<std::cell::RefCell<Recorder>>);
        impl AppEventHandler for Shared {
            fn event(&mut self, event: &AppEvent) {
                self.0.borrow_mut().event(event);
            }
        }
        let recorder = std::rc::Rc::new(std::cell::RefCell::new(Recorder::default()));
        install_app_event_handler(Shared(std::rc::Rc::clone(&recorder)));
        let event = AppEvent {
            type_: app_event_type::TRAY_MENU,
            id: 3,
        };
        // SAFETY: the trampoline only reads the event for the call, exactly as
        // the native side calls it.
        unsafe { app_event_trampoline(std::ptr::null_mut(), &event) };
        assert_eq!(
            recorder.borrow().events,
            vec![(app_event_type::TRAY_MENU, 3)]
        );
        clear_app_event_handler();
        // after clearing, events are dropped
        // SAFETY: same as above.
        unsafe { app_event_trampoline(std::ptr::null_mut(), &event) };
        assert_eq!(recorder.borrow().events.len(), 1);
    }

    #[test]
    fn unsupported_platform_helpers_are_quiet() {
        let _guard = native_test_lock();
        // Every helper must be safe to call before a window exists.
        let _ = reduce_motion();
        let _ = system_dark_mode();
        let _ = launched_at_login();
        let _ = clipboard_change_count();
        let _ = tray_icon_pixels();
        let _ = device_name();
        let _ = downloads_dir();
    }
}
