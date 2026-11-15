//! The framework shared by every UI generation in a hot-reload session.
//!
//! The hidden UI crate names this dependency `zant`, so generated paths and
//! exported macros work just as they do with a direct framework dependency.
//! Macro re-exports retain their original `$crate` (the underlying framework).
#![forbid(unsafe_code)]

pub use zant::*;
