//! Flanking rules and closing delimiter searches for inline emphasis.

use crate::markdown::scan;

use super::bytes::run_length;
use super::code_math::code_span_end;

pub(super) fn left_flanking(text: &str, start: usize, len: usize) -> bool {
    let next = scan::cp_at(text, start + len);
    next != 0 && !scan::is_whitespace_cp(next)
}

pub(super) fn right_flanking(text: &str, start: usize) -> bool {
    let prev = scan::cp_before(text, start);
    prev != 0 && !scan::is_whitespace_cp(prev)
}

pub(super) fn underscore_can_open(text: &str, start: usize, len: usize) -> bool {
    if !left_flanking(text, start, len) {
        return false;
    }
    let prev = scan::cp_before(text, start);
    prev == 0 || !scan::is_word_cp(prev)
}

fn underscore_can_close(text: &str, start: usize, len: usize) -> bool {
    if !right_flanking(text, start) {
        return false;
    }
    let next = scan::cp_at(text, start + len);
    next == 0 || !scan::is_word_cp(next)
}

/// Finds a closing delimiter run of `c` with length >= `need`, skipping code
/// spans, escapes and links. Returns the start of the closing run.
pub(super) fn find_closer(text: &str, from: usize, c: u8, need: usize) -> Option<usize> {
    let mut i = from;
    while i < text.len() {
        let ch = text.as_bytes()[i];
        if ch == b'\\' {
            i += 2;
            continue;
        }
        if ch == b'`' {
            let n = run_length(text, i, b'`');
            if let Some(end) = code_span_end(text, i, n) {
                i = end + n;
                continue;
            }
            i += n;
            continue;
        }
        if ch == c {
            let n = run_length(text, i, c);
            let closes = if c == b'_' {
                underscore_can_close(text, i, n)
            } else {
                right_flanking(text, i)
            };
            if closes && n >= need && i > from {
                // `**b *c***`: the run also closes an inner opener, so the
                // outer closer is its tail.
                if n > need && has_inner_opener(text, from, i, c) {
                    return Some(i + n - need);
                }
                return Some(i);
            }
            // a run that can open again is skipped (nested emphasis)
            i += n;
            continue;
        }
        i += 1;
    }
    None
}

fn has_inner_opener(text: &str, from: usize, to: usize, c: u8) -> bool {
    let mut i = from;
    while i < to {
        if text.as_bytes()[i] == c {
            let n = run_length(text, i, c);
            let opens = if c == b'_' {
                underscore_can_open(text, i, n)
            } else {
                left_flanking(text, i, n)
            };
            if opens && !right_flanking(text, i) {
                return true;
            }
            i += n;
            continue;
        }
        i += 1;
    }
    false
}
