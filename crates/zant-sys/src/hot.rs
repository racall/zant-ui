//! The C ABI between the hot-reload host and a replaceable UI library.
//!
//! `zant run` keeps one host process alive: it owns the native window and the
//! shared native library. The app's UI (the generated TSX code, the `native/`
//! modules and the Rust framework) is a dynamic library that exports a single
//! function, [`ENTRY`], returning a [`ZantHotApi`]. When a source file changes
//! the host loads the rebuilt library, moves the saved state over and drops
//! the old instance; the window stays open.
//!
//! Only plain C types cross this boundary, so the host and each library may be
//! separate Rust artifacts with their own copy of the framework.

use std::ffi::c_void;

use crate::platform::{ZantHost, ZantWindow, ZantWindowOptions};

/// Version of this ABI; a library with another version needs a full restart.
pub const ABI_VERSION: u32 = 1;

/// Symbol name of the entry function (`extern "C" fn() -> *const ZantHotApi`).
pub const ENTRY: &[u8] = b"zant_hot_api\0";

/// The window the app asks for (from `app.config.json`).
#[repr(C)]
pub struct ZantHotConfig {
    /// NUL-terminated UTF-8 title.
    pub title: [u8; 256],
    pub options: ZantWindowOptions,
}

/// Receives the bytes of a state snapshot.
pub type ZantHotSink = unsafe extern "C" fn(ctx: *mut c_void, data: *const u8, len: usize);

/// The functions a UI library exposes to the host.
#[repr(C)]
pub struct ZantHotApi {
    pub abi: u32,
    /// Fills in the window configuration.
    pub config: unsafe extern "C" fn(out: *mut ZantHotConfig),
    /// Starts the app on an existing window. `state` is a snapshot taken by
    /// [`ZantHotApi::save`] of the previous library (null on a cold start);
    /// `host` receives the callbacks the window events are forwarded to.
    pub create: unsafe extern "C" fn(
        window: *mut ZantWindow,
        state: *const u8,
        state_len: usize,
        host: *mut ZantHost,
    ) -> *mut c_void,
    /// Hands a snapshot of the app state to `sink`.
    pub save: unsafe extern "C" fn(instance: *mut c_void, sink: ZantHotSink, ctx: *mut c_void),
    /// Shows a message over the UI (a build error); empty text hides it.
    pub notice: unsafe extern "C" fn(instance: *mut c_void, text: *const u8, len: usize),
    /// Stops the app and frees the instance; the window stays. `exiting` is
    /// non-zero when the app ends (its exit hooks run), zero for a reload.
    pub destroy: unsafe extern "C" fn(instance: *mut c_void, exiting: i32),
}
