//! The stateful document widgets, under their old path
//! (`zant::components::widgets::markdown` / `pdf_viewer`); they live in
//! [`super::markdown`] and [`super::pdf_viewer`].

#[cfg(feature = "markdown")]
pub use super::markdown::{MarkdownOptions, MarkdownState, markdown};
#[cfg(feature = "pdf")]
pub use super::pdf_viewer::{PdfViewerOptions, PdfViewerState, pdf_viewer};
