//! Procedural macros for Zant UI.
//!
//! - `#[derive(TsxProps)]`: exposes a plain Rust options struct to the TSX
//!   compiler (field names / types / defaults). See docs/rust-port/tsx-codegen.md.
//! - `#[derive(Table)]`: generates the SQLite schema of `db/sqlite.zig`.
//!   See docs/rust-port/plan.md WP-3.10.
use proc_macro::TokenStream;

/// Placeholder: real derives land in WP-5.2 / WP-3.10.
#[proc_macro_derive(TsxProps)]
pub fn derive_tsx_props(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}

/// Placeholder: real derive lands in WP-3.10.
#[proc_macro_derive(Table)]
pub fn derive_table(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}
