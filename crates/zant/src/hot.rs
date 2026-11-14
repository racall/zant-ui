//! Hot reload: the side of the replaceable UI library.
//!
//! `zant run` keeps a host process with the native window alive and loads the
//! app's UI as a dynamic library. This module is what that library exposes:
//! [`hot_entry!`] exports the one C function the host looks up, and
//! [`Instance`] is the app running on the host's window. When a source file
//! changes, the host saves the state of the running instance
//! ([`crate::ui::App::hot_save`]), loads the rebuilt library, starts a new
//! instance with that state and drops the old one. The window never closes.
//!
//! Release builds do not use any of this; they are one statically linked
//! executable.

use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;

use zant_sys::hot::{ZantHotConfig, ZantHotSink};
use zant_sys::platform as ffi;

use crate::platform::platform::Window;
use crate::runtime::Runtime;
use crate::ui::{App, RunOptions};

pub use zant_sys::hot::{ABI_VERSION, ZantHotApi};

/// One app running on a window the host owns.
pub struct Instance {
    core: Rc<RefCell<Runtime>>,
    /// Kept alive for the callbacks the host forwards; dropping it leaves the
    /// native window open.
    _window: Rc<Window>,
}

impl Instance {
    /// Starts `app` on `window`, restoring `state` (a snapshot of the previous
    /// version) first. Returns the instance and the callbacks the host must
    /// forward the window's events to.
    pub fn start(
        mut app: impl App,
        options: RunOptions,
        window: *mut ffi::ZantWindow,
        state: Option<&str>,
    ) -> (Instance, ffi::ZantHost) {
        if let Some(state) = state {
            app.hot_load(state);
        }
        crate::motion::motion::set_enabled(
            !(crate::ui::app::env_flag("ZANT_NO_MOTION")
                || crate::platform::platform::reduce_motion()),
        );
        let core = Rc::new(RefCell::new(Runtime::init(app, options)));
        let (window, host) = Window::adopt(
            window,
            crate::runtime::host::RuntimeHost::boxed(Rc::clone(&core)),
        );
        let window = Rc::new(window);
        crate::ui::app::start(&core, &window);
        window.request_redraw(0);
        (
            Instance {
                core,
                _window: window,
            },
            host,
        )
    }

    /// The state the next version restores.
    pub fn save(&self) -> Option<String> {
        let app = self.core.borrow().shared_app();
        app.borrow().hot_save()
    }

    /// Shows `text` over the UI (a build error); `None` hides it.
    pub fn notice(&self, text: Option<String>) {
        let mut runtime = self.core.borrow_mut();
        runtime.hot_notice = text;
        runtime.invalidate();
    }

    /// Stops the app: timers and desktop registrations of this version end
    /// here; `exiting` also runs the app's exit hooks (the app ends, it is not
    /// being reloaded). The native window stays with the host.
    pub fn stop(self, exiting: bool) {
        if exiting {
            let app = self.core.borrow().shared_app();
            app.borrow_mut().on_exit();
        }
        crate::tsx::timers::set_running(false);
        crate::ui::app::stop(&self.core);
    }
}

/// Copies `title` into the fixed C buffer (truncated at a character boundary).
pub fn write_title(out: &mut ZantHotConfig, title: &str) {
    let mut end = title.len().min(out.title.len() - 1);
    while !title.is_char_boundary(end) {
        end -= 1;
    }
    out.title[..end].copy_from_slice(&title.as_bytes()[..end]);
    out.title[end] = 0;
}

/// The native window options of `options`.
pub fn native_options(options: &RunOptions) -> ffi::ZantWindowOptions {
    crate::platform::platform::native_window_options(options.window)
}

/// # Safety
/// `state` must be null or point at `len` bytes.
pub unsafe fn state_text<'a>(state: *const u8, len: usize) -> Option<&'a str> {
    if state.is_null() || len == 0 {
        return None;
    }
    // SAFETY: the caller passes a valid buffer.
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(state, len) }).ok()
}

/// # Safety
/// `instance` must come from `Box::into_raw(Box<Instance>)` and still be live.
pub unsafe extern "C" fn api_save(instance: *mut c_void, sink: ZantHotSink, ctx: *mut c_void) {
    if instance.is_null() {
        return;
    }
    // SAFETY: see above.
    let instance = unsafe { &*(instance as *const Instance) };
    if let Some(state) = instance.save() {
        // SAFETY: the host's sink copies the bytes during the call.
        unsafe { sink(ctx, state.as_ptr(), state.len()) };
    }
}

/// # Safety
/// As [`api_save`]; `text` must be null or point at `len` bytes.
pub unsafe extern "C" fn api_notice(instance: *mut c_void, text: *const u8, len: usize) {
    if instance.is_null() {
        return;
    }
    // SAFETY: see above.
    let instance = unsafe { &*(instance as *const Instance) };
    // SAFETY: the host passes a valid buffer.
    let text = unsafe { state_text(text, len) };
    instance.notice(text.map(str::to_owned));
}

/// # Safety
/// As [`api_save`]; the instance must not be used afterwards.
pub unsafe extern "C" fn api_destroy(instance: *mut c_void, exiting: i32) {
    if instance.is_null() {
        return;
    }
    // SAFETY: see above.
    let instance = unsafe { Box::from_raw(instance as *mut Instance) };
    instance.stop(exiting != 0);
}

/// Exports the hot-reload entry of a UI library.
///
/// `$prepare` is an expression of type `(impl App, RunOptions)`: the app and
/// its window options. It is evaluated once for the window configuration and
/// once per instance.
///
/// ```ignore
/// zant::hot_entry!(zant::tsx::app::prepare::<crate::app::App>(config()));
/// ```
#[macro_export]
macro_rules! hot_entry {
    ($prepare:expr) => {
        unsafe extern "C" fn __zant_hot_config(out: *mut $crate::hot::sys::ZantHotConfig) {
            let (_, options) = $prepare;
            // SAFETY: the host passes a valid out pointer.
            let out = unsafe { &mut *out };
            $crate::hot::write_title(out, &options.title);
            out.options = $crate::hot::native_options(&options);
        }

        unsafe extern "C" fn __zant_hot_create(
            window: *mut $crate::hot::sys::ZantWindow,
            state: *const u8,
            state_len: usize,
            host: *mut $crate::hot::sys::ZantHost,
        ) -> *mut ::std::ffi::c_void {
            // SAFETY: the host passes a valid buffer (or null).
            let state = unsafe { $crate::hot::state_text(state, state_len) };
            // A panic must not unwind into the host: report it as a failed
            // start (null) so the host can keep the previous version.
            let started = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let (app, options) = $prepare;
                $crate::hot::Instance::start(app, options, window, state)
            }));
            match started {
                Ok((instance, table)) => {
                    // SAFETY: the host passes a valid out pointer.
                    unsafe { *host = table };
                    ::std::boxed::Box::into_raw(::std::boxed::Box::new(instance)).cast()
                }
                Err(_) => ::std::ptr::null_mut(),
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn zant_hot_api() -> *const $crate::hot::ZantHotApi {
            static API: $crate::hot::ZantHotApi = $crate::hot::ZantHotApi {
                abi: $crate::hot::ABI_VERSION,
                config: __zant_hot_config,
                create: __zant_hot_create,
                save: $crate::hot::api_save,
                notice: $crate::hot::api_notice,
                destroy: $crate::hot::api_destroy,
            };
            &API
        }
    };
}

/// The C types [`hot_entry!`] names.
pub mod sys {
    pub use zant_sys::hot::ZantHotConfig;
    pub use zant_sys::platform::{ZantHost, ZantWindow};
}
