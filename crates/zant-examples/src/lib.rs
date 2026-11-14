//! Runnable examples for Zant UI.
//!
//! The binaries are `zant-demo`, `zant-components`, `zant-gallery`,
//! `zant-markdown`, `zant-math`, `zant-md-stream` and `zant-pdf` (the Windows
//! desktop scripts find them by these names). The application state / UI
//! trees live in the modules below so the headless tests in `tests/` can
//! build and paint the same code the binaries run.

pub mod components_app;
pub mod gallery_app;
pub mod markdown_app;
pub mod math_app;
pub mod pdf_app;
pub mod stream_app;
