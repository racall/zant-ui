//! Port of `src/pdf/` — WP-3.8.
//!
//! PDF documents, text geometry and the viewer (`zant.pdf.view`). macOS uses
//! PDFKit, Windows PDFium (`pdfium.dll` next to the exe). The module root
//! mirrors `src/pdf/mod.zig`; the viewer is WP-3.9.

pub mod annotations;
pub mod document;
#[allow(unsafe_code)]
pub mod native;
pub mod renderer;
pub mod text;
pub mod viewer;

pub use annotations::Annotation;
pub use document::{Document, Link, OpenError, OutlineEntry, Size};
/// Raw glyph record of the native backends (tests, custom geometry).
pub use native::Glyph as NativeGlyph;
pub use renderer::Renderer;
// WP-3.9: the viewer (`zant.pdf.view`) and its options, like `src/pdf/mod.zig`.
pub use viewer::{
    Fit, Labels, Match, Pos, Range, Tool, ViewOptions, Viewer, ZOOM_STEPS as zoom_steps, view,
};
