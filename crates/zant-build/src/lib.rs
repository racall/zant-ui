//! Helpers called from an application's `build.rs`:
//! `zant_build::tsx_app()` compiles `src/**/*.tsx` into Rust in `OUT_DIR`,
//! embeds icons / the Windows manifest, and copies `pdfium.dll` next to the
//! binary. Replaces `addTsxApp` / `configureApp` in `build.zig`.
#![forbid(unsafe_code)]
