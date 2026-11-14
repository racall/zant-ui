//! Port of `src/pdf/native.zig` — WP-3.8.
//!
//! The C ABI of the native PDF backends (`src/platform/pdf.h`): PDFKit on
//! macOS, PDFium on Windows. All functions are thread-safe (the bridge
//! serialises per backend), so a render worker may call [`Doc::render`] while
//! the UI thread reads text geometry through the safe layer above
//! (`pdf::document`, `pdf::text`).
//!
//! This is one of the few modules allowed to use `unsafe` (plan.md §3 rule 5);
//! `pdf/document.rs`, `pdf/text.rs`, `pdf/renderer.rs` and
//! `pdf/annotations.rs` are safe code built on top of it.
//!
//! Buffer ownership follows the header: the backend writes into caller
//! buffers and frees nothing; the only malloc'd data (`zant_image_*`,
//! `zant_bytes_*`) is handled by their own safe wrappers.

use std::ffi::c_char;
use std::os::raw::c_int;

use zant_sys::pdf as ffi;

/// Raw glyph record of the native backends (`ZantPdfGlyph`): a loose box
/// (line height), a tight (ink) box, the Unicode scalar value, font size,
/// text-object id and `flags` (1 = space, 2 = empty box).
pub type Glyph = ffi::ZantPdfGlyph;

/// Raw link record of the native backends (`ZantPdfLink`).
pub type Link = ffi::ZantPdfLink;

/// Raw outline (bookmark) record of the native backends (`ZantPdfOutlineItem`).
pub type OutlineItem = ffi::ZantPdfOutlineItem;

/// One annotation to write, the Rust form of `ZantPdfAnnotation`
/// (`src/platform/pdf.h`): coordinates are points of the displayed page
/// (top-left origin, y down), colours are `0xAARRGGBB` with the alpha as
/// opacity.
///
/// The C struct carries `rect_count` / `stroke_count` explicitly; here they
/// are derived from the `Vec`s when marshalling (`rects.len() / 4`,
/// `stroke_lengths.len()`), so the filled `ZantPdfAnnotation` is identical.
#[derive(Clone, Debug, Default)]
pub struct Annotation {
    pub page: i32,
    /// `ZANT_PDF_ANNOT_*` (`annotations::Kind as i32`).
    pub kind: i32,
    /// `0xAARRGGBB`; alpha is the opacity.
    pub color: u32,
    /// Ink stroke width in points.
    pub width: f32,
    /// Text markup: one (x, y, width, height) box per line (4 floats per
    /// rect); note: the icon box.
    pub rects: Vec<f32>,
    /// Ink: points (x, y pairs) of all strokes, split by `stroke_lengths`.
    pub points: Vec<f32>,
    /// Ink: point count of every stroke.
    pub stroke_lengths: Vec<i32>,
    /// Comment / note text (UTF-8), `None` for none.
    pub contents: Option<String>,
}

/// `ZANT_PDF_ANNOT_*` values of `src/platform/pdf.h`.
pub mod annot_kind {
    use zant_sys::pdf as ffi;

    pub const HIGHLIGHT: i32 = ffi::ZANT_PDF_ANNOT_HIGHLIGHT as i32;
    pub const UNDERLINE: i32 = ffi::ZANT_PDF_ANNOT_UNDERLINE as i32;
    pub const STRIKEOUT: i32 = ffi::ZANT_PDF_ANNOT_STRIKEOUT as i32;
    pub const SQUIGGLY: i32 = ffi::ZANT_PDF_ANNOT_SQUIGGLY as i32;
    pub const INK: i32 = ffi::ZANT_PDF_ANNOT_INK as i32;
    pub const NOTE: i32 = ffi::ZANT_PDF_ANNOT_NOTE as i32;
}

/// An open native document (`ZantPdfDocument *`). Closed on drop.
///
/// The Zig side stores the bare `*native.Doc`; Rust keeps it in this wrapper
/// so `Document` can be safe code and so the close cannot be forgotten.
pub struct Doc {
    raw: *mut ffi::ZantPdfDocument,
}

// SAFETY: the header documents all calls as thread-safe (the backends
// serialise per document: PDFKit with an NSLock, PDFium with its global lock),
// and `Doc` gives no mutable access to the handle.
unsafe impl Send for Doc {}
// SAFETY: same as above; shared calls are read-only from Rust's point of view.
unsafe impl Sync for Doc {}

impl Doc {
    /// Opens a PDF file; `password` unlocks encrypted documents. `locked` is
    /// set when a (different) password is needed (Zig: the `locked` out param
    /// of `zant_pdf_open`).
    pub fn open(path: &str, password: Option<&str>, locked: &mut bool) -> Option<Doc> {
        let path = std::ffi::CString::new(path).ok()?;
        let password = password.and_then(|value| std::ffi::CString::new(value).ok());
        let mut needs_password: c_int = 0;
        // SAFETY: both C strings live for the call; `needs_password` is a
        // valid out pointer. The returned handle is owned by the new `Doc`.
        let raw = unsafe {
            ffi::zant_pdf_open(
                path.as_ptr(),
                password
                    .as_ref()
                    .map_or(std::ptr::null(), |value| value.as_ptr()),
                &mut needs_password,
            )
        };
        *locked = needs_password != 0;
        if raw.is_null() {
            None
        } else {
            Some(Doc { raw })
        }
    }

    /// Page count (0 on error).
    pub fn count(&self) -> i32 {
        // SAFETY: live document handle; the call only reads it.
        unsafe { ffi::zant_pdf_count(self.raw) }
    }

    /// Displayed page size in points (rotation applied), `None` on error.
    pub fn size(&self, page: i32) -> Option<(f32, f32)> {
        let mut width: f32 = 0.0;
        let mut height: f32 = 0.0;
        // SAFETY: live document handle; the two floats are valid out pointers.
        let result = unsafe { ffi::zant_pdf_size(self.raw, page, &mut width, &mut height) };
        if result != 0 {
            None
        } else {
            Some((width, height))
        }
    }

    /// Renders the region (x, y, width, height) of the page scaled to
    /// `page_width` × `page_height` pixels into `pixels` (premultiplied 32-bit
    /// pixels in the native byte order, `stride` bytes per row). `true` on
    /// success.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        page: i32,
        page_width: i32,
        page_height: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        pixels: &mut [u8],
        stride: i32,
    ) -> bool {
        // SAFETY: live document handle; `pixels` is valid for
        // `stride * height` bytes (the safe caller allocates it).
        let result = unsafe {
            ffi::zant_pdf_render(
                self.raw,
                page,
                page_width,
                page_height,
                x,
                y,
                width,
                height,
                pixels.as_mut_ptr().cast(),
                stride,
            )
        };
        result == 0
    }

    /// Text glyphs of a page in reading order (Zig: the count call followed by
    /// the fill call in `document.loadText`). Empty when the page has no text
    /// layer or cannot be read.
    pub fn glyphs(&self, page: i32) -> Vec<Glyph> {
        // SAFETY: live document handle; a NULL `out` asks for the count.
        let count = unsafe { ffi::zant_pdf_glyphs(self.raw, page, std::ptr::null_mut(), 0) };
        if count <= 0 {
            return Vec::new();
        }
        let mut out = vec![ZERO_GLYPH; count as usize];
        // SAFETY: `out` is a valid buffer for `count` glyphs; the call fills
        // at most `count` entries and returns how many.
        let got = unsafe { ffi::zant_pdf_glyphs(self.raw, page, out.as_mut_ptr(), count) };
        out.truncate(got.max(0).min(count) as usize);
        out
    }

    /// Link areas of a page (count call, then fill call).
    pub fn links(&self, page: i32) -> Vec<Link> {
        // SAFETY: live document handle; a NULL `out` asks for the count.
        let count = unsafe { ffi::zant_pdf_links(self.raw, page, std::ptr::null_mut(), 0) };
        if count <= 0 {
            return Vec::new();
        }
        let mut out = vec![ZERO_LINK; count as usize];
        // SAFETY: `out` is a valid buffer for `count` links; the call fills at
        // most `count` entries and returns how many.
        let got = unsafe { ffi::zant_pdf_links(self.raw, page, out.as_mut_ptr(), count) };
        out.truncate(got.max(0).min(count) as usize);
        out
    }

    /// Bookmarks, flattened depth-first (count call, then fill call).
    pub fn outline(&self) -> Vec<OutlineItem> {
        // SAFETY: live document handle; a NULL `out` asks for the count.
        let count = unsafe { ffi::zant_pdf_outline(self.raw, std::ptr::null_mut(), 0) };
        if count <= 0 {
            return Vec::new();
        }
        let mut out = vec![ZERO_OUTLINE_ITEM; count as usize];
        // SAFETY: `out` is a valid buffer for `count` items; the call fills at
        // most `count` entries and returns how many.
        let got = unsafe { ffi::zant_pdf_outline(self.raw, out.as_mut_ptr(), count) };
        out.truncate(got.max(0).min(count) as usize);
        out
    }
}

impl Drop for Doc {
    fn drop(&mut self) {
        // SAFETY: this handle is owned by `self` and closed exactly once.
        unsafe { ffi::zant_pdf_close(self.raw) };
    }
}

const ZERO_GLYPH: Glyph = Glyph {
    x: 0.0,
    y: 0.0,
    width: 0.0,
    height: 0.0,
    tight_x: 0.0,
    tight_y: 0.0,
    tight_width: 0.0,
    tight_height: 0.0,
    codepoint: 0,
    font_size: 0.0,
    run: 0,
    flags: 0,
};

const ZERO_LINK: Link = Link {
    x: 0.0,
    y: 0.0,
    width: 0.0,
    height: 0.0,
    dest_page: 0,
    dest_y: 0.0,
    uri: [0; 1024],
};

const ZERO_OUTLINE_ITEM: OutlineItem = OutlineItem {
    depth: 0,
    page: 0,
    title: [0; 256],
};

/// Writes a copy of the PDF at `source_path` to `out_path` with `annotations`
/// added as standard PDF annotations (Highlight, Underline, StrikeOut,
/// Squiggly, Ink, Text) that other viewers show and edit. `true` on success.
pub fn save_annotated(
    source_path: &str,
    password: Option<&str>,
    out_path: &str,
    annotations: &[Annotation],
) -> bool {
    let (Ok(source_path), Ok(out_path)) = (
        std::ffi::CString::new(source_path),
        std::ffi::CString::new(out_path),
    ) else {
        return false;
    };
    let password = password.and_then(|value| std::ffi::CString::new(value).ok());
    // The `CString`s stay alive until after the call; `raw` points into the
    // annotation `Vec`s for the same reason.
    let contents: Vec<Option<std::ffi::CString>> = annotations
        .iter()
        .map(|annotation| {
            annotation
                .contents
                .as_deref()
                .and_then(|text| std::ffi::CString::new(text).ok())
        })
        .collect();
    let raw: Vec<ffi::ZantPdfAnnotation> = annotations
        .iter()
        .zip(&contents)
        .map(|(annotation, contents)| ffi::ZantPdfAnnotation {
            page: annotation.page,
            kind: annotation.kind,
            color: annotation.color,
            width: annotation.width,
            rects: annotation.rects.as_ptr(),
            rect_count: i32::try_from(annotation.rects.len() / 4).expect("int cast"),
            points: annotation.points.as_ptr(),
            stroke_lengths: annotation.stroke_lengths.as_ptr(),
            stroke_count: i32::try_from(annotation.stroke_lengths.len()).expect("int cast"),
            contents: contents
                .as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr()),
        })
        .collect();
    // SAFETY: all strings and the annotation array live for the call; empty
    // vectors yield a non-null (dangling) pointer with a zero count, which the
    // bridge never dereferences.
    let result = unsafe {
        ffi::zant_pdf_save_annotated(
            source_path.as_ptr(),
            password
                .as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr()),
            out_path.as_ptr(),
            raw.as_ptr(),
            i32::try_from(raw.len()).expect("int cast"),
        )
    };
    result == 0
}

/// Bytes of a NUL-terminated C string buffer up to the first NUL
/// (`std.mem.sliceTo` on the Zig side). The whole buffer is returned when it
/// has no NUL.
pub fn bytes_until_nul(buf: &[c_char]) -> &[u8] {
    // SAFETY: `c_char` is exactly one readable byte; the slice covers the
    // array's memory, which the backend owns.
    let bytes = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<u8>(), buf.len()) };
    match bytes.iter().position(|&byte| byte == 0) {
        Some(end) => &bytes[..end],
        None => bytes,
    }
}

/// UTF-8 view of a NUL-terminated C string buffer (lossy for invalid bytes,
/// which the backends do not produce: link URIs and titles are UTF-8).
pub fn utf8_until_nul(buf: &[c_char]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(bytes_until_nul(buf))
}
