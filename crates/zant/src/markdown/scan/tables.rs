//! GFM table rows and delimiter rows.

use super::*;

/// Splits a GFM table row into trimmed cells, honouring `\|` escapes and
/// pipes inside code spans.
pub fn split_table_row(line: &str) -> Vec<Cow<'_, str>> {
    let mut t = trim_space_tab(line);
    if t.starts_with('|') {
        t = &t[1..];
    }
    if t.ends_with('|') && !(t.len() > 1 && t.as_bytes()[t.len() - 2] == b'\\') {
        t = &t[..t.len() - 1];
    }
    let mut cells: Vec<Cow<'_, str>> = Vec::new();
    let mut owned: Option<String> = None;
    let mut start = 0usize;
    let mut in_code = false;
    let mut i: usize = 0;
    let bytes = t.as_bytes();
    while i < t.len() {
        let c = bytes[i];
        if c == b'\\' && i + 1 < t.len() && bytes[i + 1] == b'|' {
            let buffer = owned.get_or_insert_with(String::new);
            buffer.push_str(&t[start..i]);
            buffer.push('|');
            i += 1;
            i += 1;
            start = i;
            continue;
        }
        if c == b'`' {
            in_code = !in_code;
        }
        if c == b'|' && !in_code {
            cells.push(cell_from(t, &mut owned, start, i));
            start = i + 1;
        }
        i += 1;
    }
    cells.push(cell_from(t, &mut owned, start, t.len()));
    cells
}

pub(super) fn cell_from<'a>(
    t: &'a str,
    owned: &mut Option<String>,
    start: usize,
    end: usize,
) -> Cow<'a, str> {
    match owned.take() {
        Some(mut buffer) => {
            buffer.push_str(&t[start..end]);
            Cow::Owned(trim_space_tab(&buffer).to_string())
        }
        None => Cow::Borrowed(trim_space_tab(&t[start..end])),
    }
}

/// Parses a delimiter row (`| :--- | ---: |`). Returns `None` if invalid.
pub fn table_delimiter(line: &str) -> Option<Vec<Align>> {
    if !line.as_bytes().contains(&b'-') {
        return None;
    }
    let t = trim_space_tab(line);
    if t.is_empty() {
        return None;
    }
    let cells = split_table_row(t);
    if cells.is_empty() {
        return None;
    }
    // a single column needs an explicit pipe to avoid matching setext/rules
    if cells.len() == 1 && !t.as_bytes().contains(&b'|') {
        return None;
    }
    let mut out = Vec::with_capacity(cells.len());
    for cell in &cells {
        if cell.is_empty() {
            return None;
        }
        let cell_bytes = cell.as_bytes();
        let left = cell_bytes[0] == b':';
        let right = cell_bytes[cell.len() - 1] == b':';
        let body = &cell[usize::from(left)..cell.len() - usize::from(right && cell.len() > 1)];
        if body.is_empty() {
            return None;
        }
        if body.bytes().any(|c| c != b'-') {
            return None;
        }
        out.push(if left && right {
            Align::Center
        } else if right {
            Align::Right
        } else if left {
            Align::Left
        } else {
            Align::None
        });
    }
    Some(out)
}
