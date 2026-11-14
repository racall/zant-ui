//! Zant UI: a native desktop UI framework.
//!
//! The module tree mirrors the Zig `src/` tree one-to-one so the port can be
//! reviewed side by side; see docs/rust-port/plan.md.
//!
//! `unsafe` is confined to the FFI wrappers: `renderer/`, `platform/`,
//! `image.rs` and `pdf/native.rs` (plan.md §3 rule 5).
#![deny(unsafe_code)]

// Lets `#[derive(Table)]` / `#[derive(TsxProps)]` generated code spell
// `::zant::...` paths when compiling *inside* the zant crate as well.
extern crate self as zant;

#[allow(unsafe_code)]
pub mod image;
#[allow(unsafe_code)]
pub mod platform;
#[allow(unsafe_code)]
pub mod renderer;

pub mod components;
pub mod core;
pub mod db;
pub mod desktop;
pub mod error;
pub mod launch;
pub mod layout;
pub mod markdown;
pub mod math;
pub mod motion;
pub mod pdf;
pub mod runtime;
pub mod store;
pub mod str;

pub use error::{Error, Result};
// Zig `root.zig` re-exports the window configuration types at the crate root.
pub use platform::platform::{WindowOptions, WindowStyle};
pub use str::Str;
pub mod theme;
pub mod tsx;
pub mod ui;
