//! The hand-written control groups of the TSX frame benchmark: one per
//! benchmark page, so every reported scenario has a hand-written comparison.
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
