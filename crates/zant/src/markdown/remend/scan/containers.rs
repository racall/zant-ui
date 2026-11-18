//! Skipping list markers and block quote / container prefixes at line starts.

use super::*;

/// Index just past a list marker and its space at `start`, or None.
pub(super) fn skip_list_marker(text: &str, start: usize) -> Option<usize> {
    let mut i = start;
    let c = at(text, i);
    if c == b'-' || c == b'*' || c == b'+' {
        i += 1;
    } else {
        while is_digit(text, i) && i - start < 9 {
            i += 1;
        }
        if i == start || (at(text, i) != b'.' && at(text, i) != b')') {
            return None;
        }
        i += 1;
    }
    if at(text, i) == b' ' {
        Some(i + 1)
    } else {
        None
    }
}

pub(super) struct ContainerPrefix {
    pub(super) content_start: usize,
    pub(super) list_indent: usize,
    pub(super) quote_depth: usize,
}

pub(super) fn skip_container_prefix(
    text: &str,
    line_start: usize,
    line_end: usize,
) -> ContainerPrefix {
    let b = text.as_bytes();
    let mut quote_depth = 0usize;
    let mut after_quotes = line_start;
    let mut starts_item = false;
    let mut i = line_start;
    loop {
        while i < line_end && b[i] == b' ' {
            i += 1;
        }
        if i < line_end && b[i] == b'>' {
            quote_depth += 1;
            i += if at(text, i + 1) == b' ' { 2 } else { 1 };
            after_quotes = i;
            starts_item = false;
            continue;
        }
        let after_marker = if i < line_end {
            skip_list_marker(text, i)
        } else {
            None
        };
        let Some(after) = after_marker else {
            return ContainerPrefix {
                content_start: i,
                list_indent: if starts_item { i - after_quotes } else { 0 },
                quote_depth,
            };
        };
        i = after;
        starts_item = true;
    }
}

pub(super) fn skip_quote_markers(
    text: &str,
    line_start: usize,
    line_end: usize,
    depth: usize,
) -> Option<usize> {
    let mut i = line_start;
    let mut found = 0usize;
    while found < depth {
        while i < line_end && at(text, i) == b' ' {
            i += 1;
        }
        if at(text, i) != b'>' {
            return None;
        }
        i += 1;
        if at(text, i) == b' ' {
            i += 1;
        }
        found += 1;
    }
    Some(i)
}
