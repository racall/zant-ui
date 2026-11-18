//! Clipboard change notifications, metadata, and text, image, and file access.

use zant_sys::platform as ffi;

use super::SUPPORTED;

/// Sends `app_event_type::CLIPBOARD` after each clipboard change.
pub fn watch_clipboard(enable: bool) {
    if SUPPORTED {
        // SAFETY: no arguments.
        unsafe { ffi::zant_clipboard_watch(i32::from(enable)) }
    }
}

/// Changes whenever any app writes to the clipboard.
pub fn clipboard_change_count() -> u64 {
    if !SUPPORTED {
        return 0;
    }
    // SAFETY: no arguments.
    unsafe { ffi::zant_clipboard_change_count() }
}

/// `ZANT_CLIPBOARD_NO_HISTORY` / `ZANT_CLIPBOARD_NO_SYNC` bits the writer of
/// the clipboard set (password managers), 0 for ordinary content.
pub fn clipboard_privacy() -> i32 {
    if !SUPPORTED {
        return 0;
    }
    // SAFETY: no arguments.
    unsafe { ffi::zant_clipboard_privacy() }
}

pub fn set_clipboard(text: &str) -> bool {
    // SAFETY: valid slice for the call (the native side copies it).
    unsafe { ffi::zant_clipboard_set_text(text.as_ptr().cast(), text.len()) != 0 }
}

/// Empties the clipboard.
pub fn clear_clipboard() -> bool {
    if !SUPPORTED {
        return false;
    }
    // SAFETY: no arguments.
    unsafe { ffi::zant_clipboard_clear() != 0 }
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
