//! Line prefixes: toggling quote / list / task prefixes, cycling headings and inserting blocks.

use super::*;

pub(super) fn line_start(text: &str, at: usize) -> usize {
    let mut i = at.min(text.len());
    while i > 0 && text.as_bytes()[i - 1] != b'\n' {
        i -= 1;
    }
    i
}

pub(super) fn line_end(text: &str, at: usize) -> usize {
    let from = at.min(text.len());
    match text[from..].find('\n') {
        Some(index) => from + index,
        None => text.len(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Prefix {
    Quote,
    Bullet,
    Ordered,
    Task,
}

pub(super) fn existing_prefix(line: &str, kind: Prefix) -> Option<usize> {
    let indent = scan::indent_of(line);
    let t = &line[indent..];
    match kind {
        Prefix::Quote => {
            if t.starts_with("> ") {
                Some(indent + 2)
            } else if t.starts_with('>') {
                Some(indent + 1)
            } else {
                None
            }
        }
        Prefix::Task => {
            let m = scan::list_marker(line)?;
            if m.ordered {
                return None;
            }
            if m.rest.starts_with("[ ] ")
                || m.rest.starts_with("[x] ")
                || m.rest.starts_with("[X] ")
            {
                return Some(m.content_indent + 4);
            }
            None
        }
        Prefix::Bullet | Prefix::Ordered => {
            let m = scan::list_marker(line)?;
            if m.ordered != (kind == Prefix::Ordered) {
                return None;
            }
            if kind == Prefix::Bullet && (m.rest.starts_with("[ ] ") || m.rest.starts_with("[x] "))
            {
                return None;
            }
            Some(m.content_indent)
        }
    }
}

/// Toggles a line prefix on every line touched by the selection.
pub(super) fn toggle_prefix(text: &str, sel_in: Selection, kind: Prefix) -> Result {
    let sel = clamp_sel(text, sel_in);
    let first = line_start(text, sel.start());
    let last_end = line_end(text, sel.end());
    // all lines already prefixed → remove
    let mut all = true;
    for line in text[first..last_end].split('\n') {
        if existing_prefix(line, kind).is_none() && !line.is_empty() {
            all = false;
        }
    }
    let mut out = String::with_capacity(text.len() + 16);
    out.push_str(&text[..first]);
    let mut number: usize = 1;
    let mut delta_first: isize = 0;
    let mut delta_total: isize = 0;
    for (index, line) in text[first..last_end].split('\n').enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let mut d: isize = 0;
        if all {
            match existing_prefix(line, kind) {
                Some(p) => {
                    out.push_str(&line[p..]);
                    d = -(p as isize);
                }
                None => out.push_str(line),
            }
        } else {
            // replace a different list marker instead of stacking
            let mut body = line;
            if kind != Prefix::Quote
                && let Some(m) = scan::list_marker(line)
            {
                body = m.rest;
                if body.starts_with("[ ] ") || body.starts_with("[x] ") {
                    body = &body[4..];
                }
                d -= (line.len() - body.len()) as isize;
            }
            let prefix = match kind {
                Prefix::Quote => "> ".to_owned(),
                Prefix::Bullet => "- ".to_owned(),
                Prefix::Task => "- [ ] ".to_owned(),
                Prefix::Ordered => format!("{number}. "),
            };
            number += 1;
            out.push_str(&prefix);
            out.push_str(body);
            d += prefix.len() as isize;
        }
        if index == 0 {
            delta_first = d;
        }
        delta_total += d;
    }
    out.push_str(&text[last_end..]);
    let s = sel.start() as isize + delta_first;
    let e = sel.end() as isize + delta_total;
    Result {
        text: out,
        selection: Selection {
            anchor: (first as isize).max(s) as usize,
            cursor: (first as isize).max(e) as usize,
        },
    }
}

/// Cycles the heading level of the current line: none → # → ## → ### → none.
pub(super) fn cycle_heading(text: &str, sel_in: Selection) -> Result {
    let sel = clamp_sel(text, sel_in);
    let ls = line_start(text, sel.cursor);
    let le = line_end(text, sel.cursor);
    let line = &text[ls..le];
    let mut level: usize = 0;
    let bytes = line.as_bytes();
    while level < line.len() && bytes[level] == b'#' {
        level += 1;
    }
    let has_space = level < line.len() && bytes[level] == b' ';
    let body = if level > 0 && has_space {
        &line[level + 1..]
    } else if level > 0 && level == line.len() {
        ""
    } else {
        line
    };
    let current: usize = if level > 0 && (has_space || level == line.len()) {
        level
    } else {
        0
    };
    let next: usize = if current >= 3 { 0 } else { current + 1 };
    let prefix = if next == 0 {
        String::new()
    } else {
        format!("{} ", "#".repeat(next))
    };
    let mut out = String::with_capacity(text.len() + 4);
    out.push_str(&text[..ls]);
    out.push_str(&prefix);
    out.push_str(body);
    out.push_str(&text[le..]);
    let old_prefix = line.len() - body.len();
    let shift = prefix.len() as isize - old_prefix as isize;
    let cur = ((ls + prefix.len()) as isize).max(sel.cursor as isize + shift) as usize;
    Result {
        text: out,
        selection: Selection::collapsed(cur),
    }
}

pub(super) fn insert_block(
    text: &str,
    sel_in: Selection,
    before: &str,
    after: &str,
    placeholder: &str,
) -> Result {
    let sel = clamp_sel(text, sel_in);
    let s = sel.start();
    let e = sel.end();
    let inner = if e > s { &text[s..e] } else { placeholder };
    // blocks start on their own line
    let lead: &str = if s > 0 && text.as_bytes()[s - 1] != b'\n' {
        "\n\n"
    } else if s > 1 && text.as_bytes()[s - 2] != b'\n' {
        "\n"
    } else {
        ""
    };
    let trail: &str = if e < text.len() && text.as_bytes()[e] != b'\n' {
        "\n\n"
    } else {
        ""
    };
    let mut out =
        String::with_capacity(text.len() + lead.len() + before.len() + inner.len() + after.len());
    out.push_str(&text[..s]);
    out.push_str(lead);
    out.push_str(before);
    out.push_str(inner);
    out.push_str(after);
    out.push_str(trail);
    out.push_str(&text[e..]);
    let body_start = s + lead.len() + before.len();
    Result {
        text: out,
        selection: Selection {
            anchor: body_start,
            cursor: body_start + inner.len(),
        },
    }
}
