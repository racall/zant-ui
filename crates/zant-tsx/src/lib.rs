//! TSX to Rust compiler (see docs/tsx.md and docs/rust-port/plan.md M4/M5).
//!
//! The front end (lexer, parser, AST, hook expansion) is a direct translation
//! of `tools/tsx/`; the back end generates Rust instead of Zig.
#![forbid(unsafe_code)]

pub mod ast;
pub mod codegen;
pub mod dump;
pub mod error;
pub mod expr_parser;
pub mod hooks;
pub mod import_parser;
pub mod jsx_entities;
pub mod jsx_parser;
pub mod jsx_text;
pub mod lexer;
pub mod loop_parser;
pub mod native_specs;
pub mod parser;
pub mod react_compat;
pub mod string_method_names;
pub mod tag_aliases;
pub mod tags;
pub mod type_parser;
pub mod ui_tags;
