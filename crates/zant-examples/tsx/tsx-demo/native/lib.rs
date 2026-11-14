//! Native modules of the app (`docs/tsx-native.md`).
//!
//! `src/` holds only TypeScript / TSX. Bottom-layer code is Rust, in one
//! module per `native/<name>.ts` spec:
//!
//! - `native/<name>.ts` declares the types and `export declare function`s
//!   (the interface, the single source of truth);
//! - `native/<name>.rs` implements them (zant-tsx generates the spec types
//!   and one trait per module, so a missing or mistyped function is a rustc
//!   error at the `impl`);
//! - [`spec`] is the generated module tree of every spec (`native/spec.rs`,
//!   written by `build.rs` — do not edit it, and do not add it to git);
//! - this file lists the modules and registers their implementations with the
//!   TSX runtime registry (`zant::tsx::native`) so they can also be resolved
//!   by name. `src/main.rs` calls [`register`] once, on the UI thread, before
//!   the first frame.
//!
//! Internal code (a database engine, a network client, …) goes in
//! subdirectories of `native/`. Long-running work belongs on a background
//! thread; wake the UI with `zant::ui::post_wake()`.

pub mod system;

/// The generated spec types and traits of `native/*.ts`, one `pub mod` per
/// spec (`crate::native::spec::<name>`). `build.rs`
/// (`zant_build::tsx_app()`) writes `native/spec.rs` from the specs before the
/// crate is compiled; the file is generated and gitignored. `rustfmt::skip`:
/// it is generated, so `cargo fmt --check` must not rewrite it (the
/// generated file has a trailing blank line rustfmt removes).
#[rustfmt::skip]
pub mod spec;

/// Registers every native module with the runtime registry.
///
/// The generated static call path does not need the registry; it exists for
/// `native/<name>` lookups by name and for apps that register a module
/// themselves. Keep it in sync with the modules above.
pub fn register() {
    zant::tsx::native::register(system::Native);
}
