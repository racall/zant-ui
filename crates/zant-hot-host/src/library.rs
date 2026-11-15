//! Loading a UI library and finding its entry (`dlopen` / `LoadLibraryW`).
//!
//! A loaded library is never unloaded: closures and threads of an old version
//! may still exist when the next one takes over, and unloading their code
//! would crash. Only app code is replaced; the Rust framework and std remain
//! shared across generations. Unloading app libraries is a separate change.

use std::ffi::c_void;
use std::path::Path;

use zant_sys::hot::{ABI_VERSION, ENTRY, ZantHotApi};

/// Register the shared Rust DLL directory once, before loading the first UI.
/// The registration lives for the process, just like the loaded libraries.
pub fn dependencies(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn AddDllDirectory(path: *const u16) -> *mut c_void;
        }
        let path = path.canonicalize().map_err(|error| error.to_string())?;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: absolute NUL-terminated wide path; keep the cookie registered
        // for future loads (the OS releases it on process exit).
        if unsafe { AddDllDirectory(wide.as_ptr()) }.is_null() {
            return Err(format!(
                "cannot register {}: {}",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }
    }
    #[cfg(not(windows))]
    let _ = path; // macOS uses the UI library's LC_RPATH entries.
    Ok(())
}

/// The API table of a loaded UI library (valid for the rest of the process).
pub struct Library {
    pub api: &'static ZantHotApi,
}

impl Library {
    pub fn load(path: &Path) -> Result<Library, String> {
        let handle = open(path)?;
        let entry = symbol(handle, ENTRY)
            .ok_or_else(|| format!("{}: no `zant_hot_api` export", path.display()))?;
        // SAFETY: `zant_hot_api` has this signature (zant::hot_entry!).
        let entry: extern "C" fn() -> *const ZantHotApi = unsafe { std::mem::transmute(entry) };
        let api = entry();
        if api.is_null() {
            return Err(format!("{}: `zant_hot_api` returned null", path.display()));
        }
        // SAFETY: the table is a `static` of the library, which stays loaded.
        let api: &'static ZantHotApi = unsafe { &*api };
        if api.abi != ABI_VERSION {
            return Err(format!(
                "{}: hot-reload ABI {} (this host speaks {ABI_VERSION}); rebuild zant",
                path.display(),
                api.abi
            ));
        }
        Ok(Library { api })
    }
}

#[cfg(unix)]
fn open(path: &Path) -> Result<*mut c_void, String> {
    use std::ffi::{CStr, CString};
    use std::os::unix::ffi::OsStrExt;
    unsafe extern "C" {
        fn dlopen(path: *const std::os::raw::c_char, mode: i32) -> *mut c_void;
        fn dlerror() -> *const std::os::raw::c_char;
    }
    const RTLD_NOW: i32 = 0x2;
    const RTLD_LOCAL: i32 = 0x4;
    let name = CString::new(path.as_os_str().as_bytes()).map_err(|_| "bad path".to_string())?;
    // SAFETY: valid NUL-terminated path.
    let handle = unsafe { dlopen(name.as_ptr(), RTLD_NOW | RTLD_LOCAL) };
    if handle.is_null() {
        // SAFETY: dlerror returns a static C string (or null).
        let message = unsafe { dlerror() };
        let message = if message.is_null() {
            "unknown error".to_string()
        } else {
            // SAFETY: non-null C string from dlerror.
            unsafe { CStr::from_ptr(message) }
                .to_string_lossy()
                .into_owned()
        };
        return Err(format!("cannot load {}: {message}", path.display()));
    }
    Ok(handle)
}

#[cfg(unix)]
fn symbol(handle: *mut c_void, name: &[u8]) -> Option<*mut c_void> {
    unsafe extern "C" {
        fn dlsym(handle: *mut c_void, name: *const std::os::raw::c_char) -> *mut c_void;
    }
    // SAFETY: live handle, NUL-terminated name.
    let address = unsafe { dlsym(handle, name.as_ptr().cast()) };
    (!address.is_null()).then_some(address)
}

#[cfg(windows)]
fn open(path: &Path) -> Result<*mut c_void, String> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
        fn GetLastError() -> u32;
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // DEFAULT_DIRS includes the host directory (native/PDF DLLs), System32,
    // and USER_DIRS registered by dependencies(). Avoid cwd / PATH lookup.
    const LOAD_LIBRARY_SEARCH_DEFAULT_DIRS: u32 = 0x1000;
    const LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR: u32 = 0x100;
    // SAFETY: valid absolute NUL-terminated wide path, no file handle.
    let handle = unsafe {
        LoadLibraryExW(
            wide.as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_DEFAULT_DIRS | LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
        )
    };
    if handle.is_null() {
        // SAFETY: plain Win32 call.
        let code = unsafe { GetLastError() };
        return Err(format!("cannot load {} (error {code})", path.display()));
    }
    Ok(handle)
}

#[cfg(windows)]
fn symbol(handle: *mut c_void, name: &[u8]) -> Option<*mut c_void> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }
    // SAFETY: live module handle, NUL-terminated name.
    let address = unsafe { GetProcAddress(handle, name.as_ptr()) };
    (!address.is_null()).then_some(address)
}
