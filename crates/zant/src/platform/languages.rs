//! The user's preferred UI languages, most preferred first, as the system
//! spells them (`zh-Hans-CN`, `en-US`); `zant::i18n::locale::canonical`
//! normalizes them.
//!
//! macOS: `CFLocaleCopyPreferredLanguages` (the System Settings language
//! list, including a per-app override). Windows:
//! `GetUserPreferredUILanguages(MUI_LANGUAGE_NAME)` (the display language
//! list, not the regional format). Elsewhere: `LC_ALL` / `LC_MESSAGES` /
//! `LANG`.

/// The preferred languages, most preferred first; empty when unknown.
pub fn preferred() -> Vec<String> {
    preferred_impl()
}

#[cfg(target_os = "macos")]
fn preferred_impl() -> Vec<String> {
    use std::ffi::{c_char, c_void};

    type CFIndex = isize;
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFLocaleCopyPreferredLanguages() -> *const c_void;
        fn CFArrayGetCount(array: *const c_void) -> CFIndex;
        fn CFArrayGetValueAtIndex(array: *const c_void, index: CFIndex) -> *const c_void;
        fn CFStringGetCString(
            string: *const c_void,
            buffer: *mut c_char,
            size: CFIndex,
            encoding: u32,
        ) -> u8;
        fn CFRelease(object: *const c_void);
    }
    const UTF8: u32 = 0x0800_0100;
    // SAFETY: the copied array is owned here and released once; its values
    // are CFStrings it keeps alive until then. Each buffer is writable for
    // its stated length and CFStringGetCString NUL-terminates on success.
    unsafe {
        let array = CFLocaleCopyPreferredLanguages();
        if array.is_null() {
            return Vec::new();
        }
        let mut languages = Vec::new();
        for index in 0..CFArrayGetCount(array) {
            let value = CFArrayGetValueAtIndex(array, index);
            let mut buffer = [0 as c_char; 64];
            if !value.is_null()
                && CFStringGetCString(value, buffer.as_mut_ptr(), buffer.len() as CFIndex, UTF8)
                    != 0
            {
                let text = std::ffi::CStr::from_ptr(buffer.as_ptr());
                languages.push(text.to_string_lossy().into_owned());
            }
        }
        CFRelease(array);
        languages
    }
}

#[cfg(windows)]
fn preferred_impl() -> Vec<String> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserPreferredUILanguages(
            flags: u32,
            count: *mut u32,
            buffer: *mut u16,
            size: *mut u32,
        ) -> i32;
    }
    const MUI_LANGUAGE_NAME: u32 = 0x8;
    let (mut count, mut size) = (0u32, 0u32);
    // SAFETY: a null buffer asks for the required size (in UTF-16 units).
    if unsafe {
        GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &mut count,
            std::ptr::null_mut(),
            &mut size,
        )
    } == 0
        || size == 0
    {
        return Vec::new();
    }
    let mut buffer = vec![0u16; size as usize];
    // SAFETY: the buffer holds `size` UTF-16 units, as the first call asked.
    if unsafe {
        GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &mut count,
            buffer.as_mut_ptr(),
            &mut size,
        )
    } == 0
    {
        return Vec::new();
    }
    // a NUL-separated list ending with an empty string
    buffer
        .split(|unit| *unit == 0)
        .filter(|name| !name.is_empty())
        .map(String::from_utf16_lossy)
        .collect()
}

#[cfg(not(any(target_os = "macos", windows)))]
fn preferred_impl() -> Vec<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_list_is_readable() {
        let languages = super::preferred();
        eprintln!("preferred languages: {languages:?}");
        // macOS and Windows always have a display language
        if cfg!(any(target_os = "macos", windows)) {
            assert!(!languages.is_empty());
        }
        for language in languages {
            assert!(!language.is_empty());
            assert!(!language.contains('\0'));
        }
    }
}
