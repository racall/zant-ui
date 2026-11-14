//! Minimal application entry point (port of `src/launch.zig`).
//!
//! Zig's `launch.run(state, render, options)` is the [`crate::ui::app::run`]
//! function in Rust: the `App` trait replaces the `@hasDecl` hook detection.
//! This module only re-exports it so code ported from `launch.zig` can keep
//! writing `zant::launch::run(..)`.

pub use crate::ui::app::{App, RunOptions, run};
