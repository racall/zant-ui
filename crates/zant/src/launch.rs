//! Minimal application entry point.
//!
//! Re-exports [`crate::ui::app::run`] with the `App` trait and its options, so
//! applications can write `zant::launch::run(..)`.

pub use crate::ui::app::{App, RunOptions, run};
