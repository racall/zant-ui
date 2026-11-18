//! Source line normalization and byte offsets.

use crate::markdown::scan;

pub fn split_lines(source: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in source.split('\n') {
        let line = raw.trim_end_matches('\r');
        lines.push(scan::expand_leading_tabs(line).into_owned());
    }
    // a trailing newline does not start another line
    if !lines.is_empty() && !source.is_empty() && source.ends_with('\n') {
        let _ = lines.pop();
    }
    lines
}

/// Byte offset of line `index` in `source` (lines come from `split_lines`).
pub(super) fn line_offset(source: &str, index: usize) -> usize {
    let mut line: usize = 0;
    let mut i: usize = 0;
    while line < index && i < source.len() {
        if source.as_bytes()[i] == b'\n' {
            line += 1;
        }
        i += 1;
    }
    i
}

/// Byte offsets of the start of every line, plus `source.len()` at the end.
pub fn line_starts(source: &str) -> Vec<usize> {
    let mut starts: Vec<usize> = vec![0];
    for (i, c) in source.bytes().enumerate() {
        if c == b'\n' && i + 1 < source.len() {
            starts.push(i + 1);
        }
    }
    starts.push(source.len());
    starts
}
