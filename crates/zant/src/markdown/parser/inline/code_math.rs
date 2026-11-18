//! Code and math span delimiters and whitespace handling.

use std::borrow::Cow;

use super::bytes::{index_of_scalar_pos, run_length};

pub(super) fn trim_space_newline(text: &str) -> &str {
    text.trim_matches([' ', '\n'])
}

/// End of a code span opened by a run of `n` backticks at `start`, or `None`.
pub(super) fn code_span_end(text: &str, start: usize, n: usize) -> Option<usize> {
    let mut i = start + n;
    while let Some(j) = index_of_scalar_pos(text, i, b'`') {
        let len = run_length(text, j, b'`');
        if len == n {
            return Some(j);
        }
        i = j + len;
    }
    None
}

/// End of a math text span opened by `n` dollars at `start` (closing run
/// of exactly `n`), or `None`. Paragraph-scoped like code spans.
pub(super) fn math_text_end(text: &str, start: usize, n: usize) -> Option<usize> {
    let mut i = start + n;
    while let Some(j) = index_of_scalar_pos(text, i, b'$') {
        let len = run_length(text, j, b'$');
        if len == n {
            return Some(j);
        }
        i = j + len;
    }
    None
}

/// End of a single-dollar math span opened at `start` (pandoc rules), or
/// `None` when the `$` is literal. Only the first candidate closer is tried.
pub(super) fn single_dollar_end(text: &str, start: usize) -> Option<usize> {
    if start + 1 >= text.len() {
        return None;
    }
    let first = text.as_bytes()[start + 1];
    if first == b' ' || first == b'\t' || first == b'\n' {
        return None;
    }
    let mut from = start + 1;
    while let Some(k) = index_of_scalar_pos(text, from, b'$') {
        if text.as_bytes()[k - 1] == b'\\' {
            from = k + 1;
            continue;
        }
        if run_length(text, k, b'$') != 1 {
            return None;
        }
        let prev = text.as_bytes()[k - 1];
        if prev == b' ' || prev == b'\t' || prev == b'\n' {
            return None;
        }
        if k + 1 < text.len() && text.as_bytes()[k + 1].is_ascii_digit() {
            return None;
        }
        if k == start + 1 {
            return None;
        }
        return Some(k);
    }
    None
}

pub(super) fn collapse_newlines(text: &str) -> Cow<'_, str> {
    if !text.as_bytes().contains(&b'\n') {
        return Cow::Borrowed(text);
    }
    Cow::Owned(text.replace('\n', " "))
}
