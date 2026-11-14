//! TSX to Rust compiler (see docs/tsx.md and docs/rust-port/plan.md M4/M5).
//!
//! The front end (lexer, parser, AST, hook expansion) is a direct translation
//! of `tools/tsx/`; the back end generates Rust instead of Zig.
#![forbid(unsafe_code)]

pub mod ast;
