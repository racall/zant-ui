//! Byte scanning helpers shared by the inline Markdown parser.

use crate::markdown::scan;

pub(super) fn at(text: &str, i: usize) -> u8 {
    if i < text.len() {
        text.as_bytes()[i]
    } else {
        0
    }
}

pub(super) fn run_length(text: &str, i: usize, c: u8) -> usize {
    let bytes = text.as_bytes();
    let mut n: usize = 0;
    while i + n < text.len() && bytes[i + n] == c {
        n += 1;
    }
    n
}

/// First `needle` at or after `from`.
pub(super) fn index_of_scalar_pos(text: &str, from: usize, needle: u8) -> Option<usize> {
    if from >= text.len() {
        return None;
    }
    text.as_bytes()[from..]
        .iter()
        .position(|c| *c == needle)
        .map(|p| p + from)
}

/// First occurrence of `needle` at or after `from`.
pub(super) fn index_of_pos(text: &str, from: usize, needle: &str) -> Option<usize> {
    let from = from.min(text.len());
    let hay = &text.as_bytes()[from..];
    hay.windows(needle.len())
        .position(|w| w == needle.as_bytes())
        .map(|p| p + from)
}

pub(super) fn is_ascii_punct(c: u8) -> bool {
    scan::is_ascii_punct(c)
}
