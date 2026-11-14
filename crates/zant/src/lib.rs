//! Zant UI: a native desktop UI framework.
//!
//! The module tree mirrors the Zig `src/` tree one-to-one so the port can be
//! reviewed side by side; see docs/rust-port/plan.md.
//!
//! `unsafe` is confined to the FFI wrappers: `renderer/`, `platform/`,
//! `image.rs` and `pdf/native.rs` (plan.md §3 rule 5).
#![deny(unsafe_code)]

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
pub mod layout;
pub mod markdown;
pub mod math;
pub mod motion;
pub mod pdf;
pub mod runtime;
pub mod store;
pub mod theme;
pub mod tsx;
pub mod ui;
