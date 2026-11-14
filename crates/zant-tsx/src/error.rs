//! Diagnostics of the TSX parser: Zig `parser.zig`'s `error{Syntax}` becomes
//! [`ParseError`] and its `Diag` struct [`Diag`]. Allocation failures are not
//! errors in Rust, so `error.OutOfMemory` has no counterpart.

/// A failure of the parser (Zig `error.Syntax`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("syntax error")]
    Syntax,
}

/// The location and message of the first syntax error (Zig `parser.Diag`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diag {
    /// Byte offset into the source.
    pub loc: u32,
    pub msg: String,
}
