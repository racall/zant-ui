//! Build script of the app.
//!
//! `zant_build::tsx_app()` compiles every `src/**/*.tsx` file into `$OUT_DIR`
//! (`App.rs`, one module per file, and `main.rs` with the `#[path] mod …;`
//! declarations that `src/main.rs` includes at its crate root) and handles the
//! platform extras: app icons
//! embedded from `assets/icons/`, the Windows manifest / `.ico` and the
//! `pdfium.dll` copy next to the executable. It also prints
//! `cargo:rerun-if-changed` for `src/**/*.tsx`, `native/**` and
//! `app.config.json`, so editing any of them rebuilds the app.
//!
//! The file is plain items (a `mod` body, no `#![…]` inner attributes), so
//! `include!` accepts it; the lint allowances the generated code needs are at
//! the top of `src/main.rs`.
//!
//! `RUSTC_VERSION` is the Rust compiler version shown by the template's
//! example native module (`native/system.rs`, `native/system.ts`).

fn main() {
    // The Rust version the example native module reports.
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    if let Ok(output) = std::process::Command::new(rustc).arg("--version").output() {
        let line = String::from_utf8_lossy(&output.stdout);
        let version = line.trim();
        let version = version.strip_prefix("rustc ").unwrap_or(version);
        println!("cargo:rustc-env=RUSTC_VERSION={version}");
    }

    // TSX → Rust in `$OUT_DIR/app.rs`, icons, Windows resources, pdfium.dll.
    zant_build::tsx_app();
}
