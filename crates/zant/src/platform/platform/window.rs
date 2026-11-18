//! Native window handles, host callbacks, window operations, and UI wakeups.

use std::ffi::{CStr, CString};
use std::sync::atomic::{AtomicPtr, Ordering};

use crate::renderer::canvas::Canvas;
use zant_sys::platform as ffi;

use super::SUPPORTED;
use super::events::{Event, hit};
use super::window_options::{WindowOptions, to_native_options};

/// Errors of window creation.
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("unsupported platform")]
    UnsupportedPlatform,
    #[error("window creation failed")]
    WindowCreationFailed,
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
    /// By the pointer (top left just right of / below it, flipped where it
    /// does not fit, kept in the work area): popups opened by a hotkey.
    Pointer = 6,
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

/// One native window (`window.m` on macOS, `window.c` on Windows).
pub struct Window {
    handle: *mut ffi::ZantWindow,
    /// Keeps the boxed handler (and therefore the `ZantHost.userdata` pointer)
    /// alive for as long as the native window; read by the C trampolines.
    _host: Option<Box<HostBox>>,
    /// False for a window another owner created ([`Window::adopt`]): dropping
    /// this handle leaves the native window open.
    owned: bool,
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
            owned: true,
        })
    }

    /// Wraps a native window that already exists (hot reload: the host process
    /// owns it) and returns the callback table its events must be forwarded
    /// to. Dropping the handle keeps the native window open.
    pub fn adopt(
        handle: *mut ffi::ZantWindow,
        handler: Box<dyn HostHandler>,
    ) -> (Window, ffi::ZantHost) {
        let mut host = Box::new(HostBox { handler });
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
        set_post_wake_window(handle);
        let window = Window {
            handle,
            _host: Some(host),
            owned: false,
        };
        (window, table)
    }

    /// The raw pointer, for the few call sites that pass it back (runtime,
    /// snapshot wiring, desktop glue).
    pub fn raw(&self) -> *mut ffi::ZantWindow {
        self.handle
    }

    /// Restores and saves the frame under a stable name. An empty name disables
    /// saving; names containing NUL are ignored. Call just after creation.
    pub fn set_autosave(&self, name: &str) {
        let Ok(name) = CString::new(name) else {
            return;
        };
        // SAFETY: live window handle; native code copies the name during this call.
        unsafe { ffi::zant_window_set_autosave(self.handle, name.as_ptr()) }
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

    /// The system title (title bar, Dock / taskbar).
    pub fn set_title(&self, title: &str) {
        let Ok(title) = CString::new(title) else {
            return;
        };
        // SAFETY: live window handle, NUL-terminated UTF-8.
        unsafe { ffi::zant_window_set_title(self.handle, title.as_ptr()) }
    }

    /// Resizes the content area (points), keeping the top-left corner.
    pub fn set_size(&self, width: f32, height: f32) {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_set_size(self.handle, width, height) }
    }

    pub fn set_min_size(&self, width: f32, height: f32) {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_set_min_size(self.handle, width, height) }
    }

    /// 0 in a dimension: no limit.
    pub fn set_max_size(&self, width: f32, height: f32) {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_set_max_size(self.handle, width, height) }
    }

    pub fn set_always_on_top(&self, on: bool) {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_set_always_on_top(self.handle, i32::from(on)) }
    }

    pub fn set_fullscreen(&self, on: bool) {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_set_fullscreen(self.handle, i32::from(on)) }
    }

    pub fn is_fullscreen(&self) -> bool {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_is_fullscreen(self.handle) != 0 }
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

    /// Overlay windows: cover screen `index` (of `capture::screens()`), show
    /// on top and take the keyboard.
    pub fn cover_screen(&self, index: usize) {
        // SAFETY: live window handle.
        unsafe { ffi::zant_window_cover_screen(self.handle, index as i32) }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if !self.handle.is_null() && self.owned {
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
pub(super) static POST_WAKE_WINDOW: AtomicPtr<ffi::ZantWindow> =
    AtomicPtr::new(std::ptr::null_mut());

/// Registers the window `platform::post_wake` targets. The runtime sets this
/// for the window it wants background work to wake.
pub fn set_post_wake_window(window: *mut ffi::ZantWindow) {
    POST_WAKE_WINDOW.store(window, Ordering::Relaxed);
}

/// Thread-safe: asks the UI thread to run the `wake` callback once and redraw
/// (`ui::post_wake`). Does nothing before a window exists.
pub fn post_wake() {
    let handle = POST_WAKE_WINDOW.load(Ordering::Relaxed);
    if !handle.is_null() {
        // SAFETY: the native function is thread-safe and the pointer is the
        // live window registered by `create` / `set_post_wake_window` (the
        // window outlives the app's background work).
        unsafe { ffi::zant_window_post_wake(handle) }
    }
}
