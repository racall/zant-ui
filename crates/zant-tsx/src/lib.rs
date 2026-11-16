//! TSX to Rust compiler (see docs/tsx.md).
//!
//! The front end (lexer, parser, AST, hook expansion) reads the TSX subset;
//! the back end generates Rust.
#![forbid(unsafe_code)]

pub mod assets;
pub mod ast;
pub mod codegen;
pub mod compile;
pub mod dump;
pub mod entry;
pub mod error;
pub mod expr_parser;
pub mod hooks;
pub mod i18n;
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
