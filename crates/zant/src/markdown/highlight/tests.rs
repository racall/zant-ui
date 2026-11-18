//! Tests for line tokenization and highlighter state.

use super::*;

#[test]
fn zig_line() {
    let mut state = State::default();
    let toks = line("zig", "const x: u32 = 42; // hi", &mut state);
    assert_eq!(toks[0].kind, Kind::Keyword);
    let has_num = toks.iter().any(|t| t.kind == Kind::Number);
    let has_comment = toks.iter().any(|t| t.kind == Kind::Comment);
    assert!(has_num && has_comment);
}

#[test]
fn block_comments_span_lines() {
    let mut state = State::default();
    let _ = line("js", "a /* start", &mut state);
    assert!(state.in_block_comment);
    let toks = line("js", "end */ b", &mut state);
    assert_eq!(toks[0].kind, Kind::Comment);
    assert!(!state.in_block_comment);
}
