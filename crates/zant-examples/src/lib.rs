//! Runnable examples for Zant UI, ported from the Zig `examples/` and `src/`
//! entry points (WP-2.9, WP-3.11).
//!
//! The binaries keep the Zig executable names so `scripts/parity/compare.sh`
//! and the Windows desktop scripts find them: `zant-demo` (WP-1.5),
//! `zant-components` and `zant-gallery` (WP-2.9), `zant-markdown`,
//! `zant-math`, `zant-md-stream` and `zant-pdf` (WP-3.11). The application
//! state / UI trees live in the modules below so the headless tests in
//! `tests/` can build and paint the same code the binaries run.

pub mod components_app;
pub mod gallery_app;
pub mod markdown_app;
pub mod math_app;
pub mod pdf_app;
pub mod stream_app;
