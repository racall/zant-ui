//! Continuing and indenting lists on Enter / Tab.

use super::*;

/// Enter inside a list item: continue the list, or end it on an empty item.
/// Returns `None` when the caret is not in a list item (normal newline).
pub fn continue_list(text: &str, sel_in: Selection) -> Option<Result> {
    let sel = clamp_sel(text, sel_in);
    if sel.anchor != sel.cursor {
        return None;
    }
    let ls = line_start(text, sel.cursor);
    let le = line_end(text, sel.cursor);
    let line = &text[ls..le];
    let mut quote_prefix = "";
    let mut body = line;
    if line.trim_start_matches(' ').starts_with('>') {
        let qi = line.find('>').expect("checked above");
        let end = if qi + 1 < line.len() && line.as_bytes()[qi + 1] == b' ' {
            qi + 2
        } else {
            qi + 1
        };
        quote_prefix = &line[..end];
        body = &line[end..];
    }
    let Some(marker) = scan::list_marker(body) else {
        if quote_prefix.is_empty() {
            return None;
        }
        // continue a block quote
        if body.trim_matches(' ').is_empty() {
            return Some(Result {
                text: splice(text, ls, le, ""),
                selection: Selection::collapsed(ls),
            });
        }
        let mut out = String::with_capacity(text.len() + 1 + quote_prefix.len());
        out.push_str(&text[..sel.cursor]);
        out.push('\n');
        out.push_str(quote_prefix);
        out.push_str(&text[sel.cursor..]);
        return Some(Result {
            text: out,
            selection: Selection::collapsed(sel.cursor + 1 + quote_prefix.len()),
        });
    };
    let content_start = ls + quote_prefix.len() + marker.content_indent;
    if sel.cursor < content_start.min(le) {
        return None;
    }
    let mut rest = marker.rest;
    let is_task = rest.starts_with("[ ] ")
        || rest.starts_with("[x] ")
        || rest.starts_with("[X] ")
        || rest == "[ ]";
    if is_task {
        rest = if rest.len() >= 4 { &rest[4..] } else { "" };
    }
    if rest.trim_matches(' ').is_empty() {
        // empty item: remove the marker and stop the list
        return Some(Result {
            text: splice(text, ls, le, quote_prefix),
            selection: Selection::collapsed(ls + quote_prefix.len()),
        });
    }
    let indent = &body[..scan::indent_of(body)];
    let next_marker = if marker.ordered {
        format!(
            "{}{}{} ",
            indent,
            marker.number + 1,
            marker.delimiter as char
        )
    } else {
        format!(
            "{}{} {}",
            indent,
            marker.bullet as char,
            if is_task { "[ ] " } else { "" }
        )
    };
    let mut out = String::with_capacity(text.len() + 1 + quote_prefix.len() + next_marker.len());
    out.push_str(&text[..sel.cursor]);
    out.push('\n');
    out.push_str(quote_prefix);
    out.push_str(&next_marker);
    out.push_str(&text[sel.cursor..]);
    Some(Result {
        text: out,
        selection: Selection::collapsed(sel.cursor + 1 + quote_prefix.len() + next_marker.len()),
    })
}

/// Tab / Shift-Tab on list lines: indent or outdent by two spaces.
/// Returns `None` when the selection is not on list items.
pub fn indent_list(text: &str, sel_in: Selection, outdent: bool) -> Option<Result> {
    let sel = clamp_sel(text, sel_in);
    let first = line_start(text, sel.start());
    let last_end = line_end(text, sel.end());
    let mut any = false;
    for line in text[first..last_end].split('\n') {
        if scan::list_marker(scan::strip_indent(line, 64)).is_some() {
            any = true;
        }
    }
    if !any {
        return None;
    }
    let mut out = String::with_capacity(text.len() + 8);
    out.push_str(&text[..first]);
    let mut delta_first: isize = 0;
    let mut delta_total: isize = 0;
    for (index, line) in text[first..last_end].split('\n').enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let d: isize = if outdent {
            let n = scan::indent_of(line).min(2);
            out.push_str(&line[n..]);
            -(n as isize)
        } else {
            out.push_str("  ");
            out.push_str(line);
            2
        };
        if index == 0 {
            delta_first = d;
        }
        delta_total += d;
    }
    out.push_str(&text[last_end..]);
    let s = (first as isize).max(sel.start() as isize + delta_first) as usize;
    let e = (first as isize).max(sel.end() as isize + delta_total) as usize;
    Some(Result {
        text: out,
        selection: Selection {
            anchor: s,
            cursor: e,
        },
    })
}
