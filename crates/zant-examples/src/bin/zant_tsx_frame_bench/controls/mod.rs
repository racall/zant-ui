//! The hand-written control groups of the TSX frame benchmark (WP-5.10c).
//!
//! Each module is a line-by-line Rust port of the Zig control group the Zig
//! `bench-tsx` used as the baseline (`examples/tsx/*_manual.zig`), except
//! [`settings`], which has no Zig control: the Zig benchmark measured the TSX
//! settings page alone, while plan.md §5 M5 requires a hand-written comparison
//! for every reported scenario.
//!
//! They are ordinary `ui::App` implementations with the same UI tree, keys and
//! state transitions as the TSX page, so the frame cost of the two sides is
//! comparable.
#![forbid(unsafe_code)]

pub mod counter;
pub mod list;
pub mod local;
pub mod markdown;
pub mod settings;
pub mod todos;
