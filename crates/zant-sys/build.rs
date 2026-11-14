//! Compiles the native Skia / platform / PDF sources and links Skia.
//!
//! macOS: C++ (skia_bridge.cpp, skia_bridge_mac.mm, pdf.mm) and C
//! (window.m) compiled with `cc`, linking the static Skia archives in
//! `third_party/skia/skia-binaries/` and the system frameworks.
//!
//! Windows: MSVC compiles skia_bridge.cpp / skia_bridge_win.cpp /
//! pdf_win.cpp / window.c; the Skia static libs live in
//! `third_party/skia-win/`. ICU data and the math font are embedded by
//! `include_bytes!` and handed to the bridge at startup.
//!
//! See docs/rust-port/plan.md WP-0.3.

fn main() {
    // Filled in by WP-0.3.
}
