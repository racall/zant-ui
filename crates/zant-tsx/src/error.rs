//! Diagnostics of the TSX parser: Zig `parser.zig`'s `error{Syntax}` becomes
//! [`ParseError`] and its `Diag` struct [`Diag`]. Allocation failures are not
//! errors in Rust, so `error.OutOfMemory` has no counterpart.
//!
//! [`position`] and [`render_diag`] are the source-position / caret renderer of
//! Zig `compile.zig` (WP-4.4); the CLI and the tests format errors with them so
//! the text stays byte-compatible with the Zig compiler:
//! `path:line:col: error: msg`, the source line, and a caret.

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

/// A source position (Zig `compile.Position`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position<'a> {
    /// 1-based line number.
    pub line: usize,
    /// 1-based, in Unicode scalar values.
    pub column: usize,
    pub line_text: &'a str,
}

/// The line / column / source line at `loc` (Zig `compile.position`).
pub fn position(src: &str, loc: u32) -> Position<'_> {
    let at = (loc as usize).min(src.len());
    let line_start = src[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = src[at..].find('\n').map(|i| i + at).unwrap_or(src.len());
    let line = src[..line_start].matches('\n').count() + 1;
    let column = src[line_start..at].chars().count() + 1;
    Position {
        line,
        column,
        line_text: src[line_start..line_end].trim_end_matches('\r'),
    }
}

/// `path:line:col: error: msg` followed by the source line and a caret, the way
/// Zig reports errors (Zig `compile.renderDiag`).
pub fn render_diag(path: &str, src: &str, diag: &Diag) -> String {
    let pos = position(src, diag.loc);
    let mut caret = String::new();
    let mut col = 1;
    for cp in pos.line_text.chars() {
        if col >= pos.column {
            break;
        }
        caret.push(if cp == '\t' { '\t' } else { ' ' });
        col += 1;
    }
    format!(
        "{}:{}:{}: error: {}\n    {}\n    {}^\n",
        path, pos.line, pos.column, diag.msg, pos.line_text, caret
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    // Zig `compile.zig` tests for the diagnostic renderer.

    #[test]
    fn positions_count_lines_and_code_points() {
        let src = "a\nbé c";
        let pos = position(src, 6); // `c` (é is two bytes)
        assert_eq!(2, pos.line);
        assert_eq!(4, pos.column);
        assert_eq!("bé c", pos.line_text);
    }

    #[test]
    fn syntax_errors_render_with_a_caret() {
        let src = "function App() {\n  return <View></Text>;\n}\n";
        let mut p = Parser::init(src);
        let err = p.parse_module().unwrap_err();
        assert_eq!(ParseError::Syntax, err);
        let diag = p.diag.expect("parse error without a diagnostic");
        let text = render_diag("app.tsx", src, &diag);
        assert_eq!(
            "app.tsx:2:18: error: expected `</View>` to close <View>\n      return <View></Text>;\n                     ^\n",
            text
        );
    }
}
