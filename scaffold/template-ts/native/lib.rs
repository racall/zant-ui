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
//! - this file lists the modules and registers their implementations with the
//!   TSX runtime registry (`zant::tsx::native`) so they can also be resolved
//!   by name. `src/main.rs` calls [`register`] once, on the UI thread, before
//!   the first frame.
//!
//! Internal code (a database engine, a network client, …) goes in
//! subdirectories of `native/`. Long-running work belongs on a background
//! thread; wake the UI with `zant::ui::post_wake()`.

pub mod system;

/// Registers every native module with the runtime registry.
///
/// The generated static call path does not need the registry; it exists for
/// `native/<name>` lookups by name and for apps that register a module
/// themselves. Keep it in sync with the modules above.
pub fn register() {
    zant::tsx::native::register(system::Native);
}
