//! Path-based helpers used by `PdfPreview` (single page preview): the
//! page count, a rendered page as a PNG temp file, and the page text. This
//! module is one of the FFI wrappers where `lib.rs` allows `unsafe`.

use std::ffi::{CStr, CString};

use zant_sys::pdf as ffi;

/// Page count of a PDF file (0 when it cannot be opened).
pub fn page_count(path: &str) -> usize {
    let Ok(path) = CString::new(path) else {
        return 0;
    };
    // SAFETY: NUL-terminated path; the call only reads it.
    let count = unsafe { ffi::zant_pdf_page_count(path.as_ptr()) };
    if count > 0 { count as usize } else { 0 }
}

/// Renders `page` at `width` × `height` pixels to a cached PNG temp file and
/// returns its path (a slice into the caller's `out` buffer).
/// `None` on failure.
pub fn render_page<'a>(
    path: &str,
    page: usize,
    width: u32,
    height: u32,
    out: &'a mut [u8],
) -> Option<&'a str> {
    let path = CString::new(path).ok()?;
    let page = i32::try_from(page).ok()?;
    let width = i32::try_from(width).ok()?;
    let height = i32::try_from(height).ok()?;
    // SAFETY: NUL-terminated path and a valid `out` buffer of `out.len()`
    // bytes; the call writes at most that many bytes including the NUL.
    let result = unsafe {
        ffi::zant_pdf_render_page(
            path.as_ptr(),
            page,
            width,
            height,
            out.as_mut_ptr().cast(),
            out.len(),
        )
    };
    if result != 0 {
        return None;
    }
    c_str_from(out)
}

/// UTF-8 text of a page (a slice into the caller's `out` buffer).
/// `None` on failure.
pub fn page_text<'a>(path: &str, page: usize, out: &'a mut [u8]) -> Option<&'a str> {
    let path = CString::new(path).ok()?;
    let page = i32::try_from(page).ok()?;
    // SAFETY: NUL-terminated path and a valid `out` buffer of `out.len()`
    // bytes; the call writes at most that many bytes including the NUL.
    let result =
        unsafe { ffi::zant_pdf_page_text(path.as_ptr(), page, out.as_mut_ptr().cast(), out.len()) };
    if result != 0 {
        return None;
    }
    c_str_from(out)
}

/// The NUL-terminated string the backend wrote into `buf`.
fn c_str_from(buf: &[u8]) -> Option<&str> {
    CStr::from_bytes_until_nul(buf).ok()?.to_str().ok()
}
