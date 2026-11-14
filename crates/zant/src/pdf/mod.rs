//! PDF documents, text geometry and the viewer (`zant.pdf.view`). macOS uses
//! PDFKit, Windows PDFium (`pdfium.dll` next to the exe).

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
// The viewer (`zant.pdf.view`) and its options.
pub use viewer::{
    Fit, Labels, Match, Pos, Range, Tool, ViewOptions, Viewer, ZOOM_STEPS as zoom_steps, view,
};
