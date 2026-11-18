//! System settings, device information, paths, environment, and monotonic time.

use std::time::Instant;

use zant_sys::platform as ffi;

use super::SUPPORTED;

/// System "Reduce Motion" accessibility setting (no window needed).
pub fn reduce_motion() -> bool {
    // SAFETY: no arguments, no state.
    unsafe { ffi::zant_window_reduce_motion() != 0 }
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

/// Monotonic clock in microseconds.
///
/// Measured with `Instant` from the first call (callers only ever use
/// differences).
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

/// Hands memory the allocator keeps but no longer uses back to the system.
pub fn release_unused_memory() {
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
        }
        // SAFETY: a null zone means every zone; a goal of 0 frees what it can.
        unsafe {
            malloc_zone_pressure_relief(std::ptr::null_mut(), 0);
        }
    }
    #[cfg(target_os = "windows")]
    {
        unsafe extern "system" {
            fn GetProcessHeap() -> *mut std::ffi::c_void;
            fn HeapCompact(heap: *mut std::ffi::c_void, flags: u32) -> usize;
        }
        // SAFETY: the process heap handle is always valid.
        unsafe {
            HeapCompact(GetProcessHeap(), 0);
        }
    }
}
