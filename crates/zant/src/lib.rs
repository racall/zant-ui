//! Zant UI: a native desktop UI framework.
//!
//! `unsafe` is confined to the FFI wrappers: `renderer/`, `platform/`,
//! `image.rs` and `pdf/native.rs`.
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
#[allow(unsafe_code)]
pub mod hot;
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
// The window configuration types are re-exported at the crate root.
pub use platform::platform::{WindowOptions, WindowStyle};
pub use str::Str;
pub mod theme;
pub mod tsx;
pub mod ui;

mod app_macro;
