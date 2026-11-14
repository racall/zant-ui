//! Port of `src/platform/dialog.zig` — WP-1.1d.
//!
//! Native modal dialogs and small platform services (AppKit on macOS,
//! IFileDialog / TaskDialog / ShellExecute on Windows).

use std::ffi::{CStr, CString};

use zant_sys::platform as ffi;

/// Extensions of `open_file` / `save_file` (Markdown and plain text).
pub const TEXT_EXTENSIONS: &str = "md,markdown,txt";

/// Errors of file dialogs / file writing (Zig: `error.WriteFailed`,
/// `error.NameTooLong`).
#[derive(Debug, thiserror::Error)]
pub enum DialogError {
    #[error("write failed")]
    WriteFailed,
    #[error("invalid path")]
    InvalidPath,
}

fn to_cstring(value: &str) -> Option<CString> {
    CString::new(value).ok()
}

/// Shows an open panel for Markdown/text files.
pub fn open_file() -> Option<String> {
    open_file_as(TEXT_EXTENSIONS)
}

/// Open panel restricted to `extensions` ("png,jpg"); empty allows any type.
pub fn open_file_as(extensions: &str) -> Option<String> {
    let extensions = to_cstring(extensions);
    // SAFETY: NUL-terminated extensions (or NULL); the returned path is freed
    // once, after the copy.
    unsafe {
        let raw = ffi::zant_open_file_dialog_ext(
            extensions
                .as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr()),
        );
        if raw.is_null() {
            return None;
        }
        let path = CStr::from_ptr(raw).to_string_lossy().into_owned();
        ffi::zant_dialog_free(raw);
        Some(path)
    }
}

/// Shows a save panel for Markdown/text files.
pub fn save_file(default_name: &str) -> Option<String> {
    save_file_as(default_name, TEXT_EXTENSIONS)
}

/// Save panel restricted to `extensions` ("csv,md"); empty allows any type.
pub fn save_file_as(default_name: &str, extensions: &str) -> Option<String> {
    let name = to_cstring(default_name);
    let extensions = to_cstring(extensions);
    // SAFETY: NUL-terminated strings (or NULL); the returned path is freed once.
    unsafe {
        let raw = ffi::zant_save_file_dialog_ext(
            name.as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr()),
            extensions
                .as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr()),
        );
        if raw.is_null() {
            return None;
        }
        let path = CStr::from_ptr(raw).to_string_lossy().into_owned();
        ffi::zant_dialog_free(raw);
        Some(path)
    }
}

/// Button of `confirm`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    First,
    Second,
    Third,
}

/// Modal alert with up to three buttons (e.g. 保存 / 不保存 / 取消).
pub fn confirm(title: &str, message: &str, buttons: &[&str]) -> Choice {
    let title = to_cstring(title);
    let message = to_cstring(message);
    let labels: Vec<Option<CString>> = (0..3)
        .map(|index| buttons.get(index).and_then(|label| to_cstring(label)))
        .collect();
    let ptr = |index: usize| {
        labels[index]
            .as_ref()
            .map_or(std::ptr::null(), |value| value.as_ptr())
    };
    // SAFETY: all strings are NUL-terminated and outlive the call.
    match unsafe {
        ffi::zant_confirm_dialog(
            title.as_ref().map_or(std::ptr::null(), |v| v.as_ptr()),
            message.as_ref().map_or(std::ptr::null(), |v| v.as_ptr()),
            ptr(0),
            ptr(1),
            ptr(2),
        )
    } {
        0 => Choice::First,
        1 => Choice::Second,
        _ => Choice::Third,
    }
}

/// Opens a URL with the default handler (browser, mail client, ...).
pub fn open_url(url: &str) -> bool {
    let Some(url) = to_cstring(url) else {
        return false;
    };
    // SAFETY: NUL-terminated URL.
    unsafe { ffi::zant_open_url(url.as_ptr()) != 0 }
}

/// Writes `data` to a UTF-8 path. Rust's `std::fs` uses the wide-character APIs
/// on Windows, so the native `zant_write_file` helper is not needed here.
pub fn write_file(path: &str, data: &[u8]) -> Result<(), DialogError> {
    std::fs::write(path, data).map_err(|_| DialogError::WriteFailed)
}
