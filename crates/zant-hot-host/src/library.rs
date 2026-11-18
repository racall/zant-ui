//! Loading a UI library and finding its entry (`dlopen` / `LoadLibraryW`).
//!
//! Only a successfully replaced, destroyed instance's library may be unloaded.
//! There is deliberately no `Drop`: a failed start may leave callbacks behind,
//! and a failed swap must keep the previous library available for restarting.
//! Unloading is the default ([`Library::unload`]); `ZANT_HOT_KEEP_LIBS=1` keeps
//! every version mapped for diagnosis.

use std::ffi::c_void;
use std::path::{Path, PathBuf};

use zant_sys::hot::{ABI_VERSION, ENTRY, ZantHotApi};

/// Register the shared Rust DLL directory once, before loading the first UI.
/// The registration lives for the process, including future library loads.
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

/// A loaded UI library. Dropping this value keeps the image mapped.
pub struct Library {
    handle: *mut c_void,
    api: *const ZantHotApi,
    path: PathBuf,
}

// SAFETY: loading happens on the command thread; ownership then moves to the
// UI thread. The handle and immutable API table can be transferred between
// threads; app entry points and unloading are only called on the UI thread.
unsafe impl Send for Library {}

impl Library {
    pub fn load(path: &Path) -> Result<Library, String> {
        let path = path.canonicalize().map_err(|error| error.to_string())?;
        let handle = open(&path)?;
        let entry = symbol(handle, ENTRY)
            .ok_or_else(|| format!("{}: no `zant_hot_api` export", path.display()))?;
        // SAFETY: `zant_hot_api` has this signature (zant::hot_entry!).
        let entry: extern "C" fn() -> *const ZantHotApi = unsafe { std::mem::transmute(entry) };
        let api = entry();
        if api.is_null() {
            return Err(format!("{}: `zant_hot_api` returned null", path.display()));
        }
        // SAFETY: the entry returns the library's static API table.
        let abi = unsafe { (*api).abi };
        if abi != ABI_VERSION {
            return Err(format!(
                "{}: hot-reload ABI {} (this host speaks {ABI_VERSION}); rebuild zant",
                path.display(),
                abi
            ));
        }
        Ok(Library { handle, api, path })
    }

    pub fn api(&self) -> &ZantHotApi {
        // SAFETY: this library stays loaded for the duration of the borrow.
        unsafe { &*self.api }
    }

    /// # Safety
    /// The instance must have been destroyed and its replacement successfully
    /// started. No callbacks or other pointers into this image may be used again.
    pub unsafe fn unload(self) {
        // unloading is the default; ZANT_HOT_KEEP_LIBS=1 (or the older
        // ZANT_HOT_UNLOAD=0) keeps every version mapped for diagnosis
        let keep = std::env::var_os("ZANT_HOT_KEEP_LIBS").is_some_and(|value| value == "1")
            || std::env::var_os("ZANT_HOT_UNLOAD").is_some_and(|value| value == "0");
        if !keep {
            // SAFETY: the caller guarantees that the old instance is gone.
            if !unsafe { close(self.handle) } {
                eprintln!("[zant-hot] cannot unload {}", self.path.display());
            }
        }
        self.trace(if keep { "kept" } else { "unloaded" });
    }

    pub fn trace(&self, action: &str) {
        if std::env::var_os("ZANT_HOT_TRACE").is_some() {
            let images = mapped_images(&self.path);
            let status = if images == 0 { "gone" } else { "still mapped" };
            let name = self.path.file_stem().unwrap_or_default().to_string_lossy();
            eprintln!(
                "[zant-hot] {action} {} ({status}) images={images}",
                name.strip_prefix("lib").unwrap_or(&name)
            );
        }
    }
}

#[cfg(unix)]
unsafe fn close(handle: *mut c_void) -> bool {
    unsafe extern "C" {
        fn dlclose(handle: *mut c_void) -> i32;
    }
    // SAFETY: the caller owns this live dlopen handle and will not use it again.
    unsafe { dlclose(handle) == 0 }
}

#[cfg(windows)]
unsafe fn close(handle: *mut c_void) -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn FreeLibrary(module: *mut c_void) -> i32;
    }
    // SAFETY: the caller owns this live LoadLibraryExW handle.
    unsafe { FreeLibrary(handle) != 0 }
}

#[cfg(target_os = "macos")]
fn mapped_images(path: &Path) -> usize {
    use std::ffi::CStr;
    use std::os::unix::ffi::OsStrExt;
    unsafe extern "C" {
        fn _dyld_image_count() -> u32;
        fn _dyld_get_image_name(index: u32) -> *const std::os::raw::c_char;
    }
    // SAFETY: dyld's image queries accept each index; a missing image is null.
    (0..unsafe { _dyld_image_count() })
        .filter(|&index| {
            // SAFETY: dyld returns a NUL-terminated image path or null.
            let name = unsafe { _dyld_get_image_name(index) };
            !name.is_null()
                && unsafe { CStr::from_ptr(name) }.to_bytes() == path.as_os_str().as_bytes()
        })
        .count()
}

#[cfg(windows)]
fn mapped_images(path: &Path) -> usize {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: valid NUL-terminated full path; this does not add a reference.
    usize::from(!unsafe { GetModuleHandleW(wide.as_ptr()) }.is_null())
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
