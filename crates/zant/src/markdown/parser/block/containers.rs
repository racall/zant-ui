//! Block quotes, GFM alerts, and nested lists with task items.

use crate::markdown::ast::{self, Block};
use crate::markdown::scan;

use super::Parser;

impl Parser {
    pub(super) fn blockquote(
        &mut self,
        lines: &[&str],
        start: usize,
        out: &mut Vec<Block>,
    ) -> usize {
        let mut inner: Vec<&str> = Vec::new();
        let mut i = start;
        let mut lazy_ok = false;
        while i < lines.len() {
            let line = lines[i];
            if is_quote_line(line) {
                let stripped = strip_quote(line);
                inner.push(stripped);
                lazy_ok = !scan::is_blank(stripped) && scan::fence_open(stripped).is_none();
                i += 1;
                continue;
            }
            // lazy continuation of a paragraph inside the quote
            if lazy_ok && !scan::is_blank(line) && !self.interrupts_paragraph(line) {
                inner.push(line);
                i += 1;
                continue;
            }
            break;
        }
        // GFM alerts: > [!NOTE]
        if !inner.is_empty() {
            let first = scan::trim_space_tab(inner[0]);
            if let Some(kind) = callout_kind(first) {
                let blocks = self.parse_lines(&inner[1..], None, 0);
                out.push(Block::Callout(ast::Callout { kind, blocks }));
                return i;
            }
        }
        let blocks = self.parse_lines(&inner, None, 0);
        out.push(Block::Quote(blocks));
        i
    }

    pub(super) fn list(&mut self, lines: &[&str], start: usize, out: &mut Vec<Block>) -> usize {
        let first = scan::list_marker(lines[start]).expect("list marker");
        let mut items: Vec<ast::ListItem> = Vec::new();
        let mut tight = true;
        let mut i = start;
        while i < lines.len() {
            let Some(marker) = scan::list_marker(lines[i]) else {
                break;
            };
            if !same_list_type(&first, &marker) {
                break;
            }
            if scan::is_thematic_break(lines[i]) {
                break;
            }
            let marker_indent = scan::indent_of(lines[i]);
            let content_indent = marker.content_indent;
            let mut item_lines: Vec<&str> = Vec::new();
            let mut rest = marker.rest;
            let mut checked: Option<bool> = None;
            if rest.len() >= 3
                && rest.as_bytes()[0] == b'['
                && rest.as_bytes()[2] == b']'
                && (rest.len() == 3 || rest.as_bytes()[3] == b' ')
            {
                match rest.as_bytes()[1] {
                    b' ' => checked = Some(false),
                    b'x' | b'X' => checked = Some(true),
                    _ => {}
                }
                if checked.is_some() {
                    rest = if rest.len() > 4 { &rest[4..] } else { "" };
                }
            }
            item_lines.push(rest);
            let mut j = i + 1;
            let mut saw_blank = false;
            let mut in_fence: Option<scan::Fence> = scan::fence_open(rest);
            while j < lines.len() {
                let line = lines[j];
                if scan::is_blank(line) {
                    item_lines.push("");
                    saw_blank = true;
                    j += 1;
                    continue;
                }
                let ind = scan::indent_of(line);
                let nested_marker = ind >= marker_indent + 2
                    && scan::list_marker(scan::strip_indent(line, ind)).is_some();
                if ind >= content_indent
                    || nested_marker
                    || (in_fence.is_some() && ind > marker_indent)
                {
                    let stripped = scan::strip_indent(line, ind.min(content_indent));
                    if let Some(f) = in_fence.as_ref() {
                        if scan::is_fence_close(stripped, f) {
                            in_fence = None;
                        }
                    } else if let Some(f) = scan::fence_open(stripped) {
                        in_fence = Some(f);
                    }
                    item_lines.push(stripped);
                    j += 1;
                    continue;
                }
                // lazy paragraph continuation
                if !saw_blank
                    && in_fence.is_none()
                    && !self.interrupts_paragraph(line)
                    && scan::list_marker(line).is_none()
                {
                    let prev = *item_lines.last().expect("item lines are never empty");
                    if !scan::is_blank(prev) {
                        item_lines.push(line.trim_start_matches(' '));
                        j += 1;
                        continue;
                    }
                }
                break;
            }
            // trailing blank lines belong between items, not to this item
            let mut content_end = item_lines.len();
            while content_end > 1 && scan::is_blank(item_lines[content_end - 1]) {
                content_end -= 1;
            }
            let trailing_blank = content_end < item_lines.len();
            for l in &item_lines[..content_end] {
                if scan::is_blank(l) {
                    tight = false;
                    break;
                }
            }
            let blocks = self.parse_lines(&item_lines[..content_end], None, 0);
            items.push(ast::ListItem { checked, blocks });
            i = j;
            if i < lines.len()
                && trailing_blank
                && let Some(next) = scan::list_marker(lines[i])
                && same_list_type(&first, &next)
            {
                tight = false;
            }
            // return the trailing blank lines to the caller when the list ends
            let ends_list = match if i < lines.len() {
                scan::list_marker(lines[i])
            } else {
                None
            } {
                None => true,
                Some(next) => !same_list_type(&first, &next),
            };
            if trailing_blank && ends_list {
                i -= item_lines.len() - content_end;
                break;
            }
        }
        out.push(Block::List(ast::List {
            ordered: first.ordered,
            start: first.number,
            tight,
            items,
        }));
        i
    }
}

fn is_quote_line(line: &str) -> bool {
    if scan::indent_of(line) > 3 {
        return false;
    }
    let t = line.trim_start_matches(' ');
    !t.is_empty() && t.as_bytes()[0] == b'>'
}

fn strip_quote(line: &str) -> &str {
    let t = line.trim_start_matches(' ');
    if t.len() > 1 && t.as_bytes()[1] == b' ' {
        &t[2..]
    } else {
        &t[1..]
    }
}

fn callout_kind(line: &str) -> Option<ast::CalloutKind> {
    if line.len() < 5 || !line.starts_with("[!") || !line.ends_with(']') {
        return None;
    }
    let name = &line[2..line.len() - 1];
    for (label, kind) in [
        ("NOTE", ast::CalloutKind::Note),
        ("TIP", ast::CalloutKind::Tip),
        ("IMPORTANT", ast::CalloutKind::Important),
        ("WARNING", ast::CalloutKind::Warning),
        ("CAUTION", ast::CalloutKind::Caution),
    ] {
        if name.eq_ignore_ascii_case(label) {
            return Some(kind);
        }
    }
    None
}

fn same_list_type(a: &scan::ListMarker<'_>, b: &scan::ListMarker<'_>) -> bool {
    if a.ordered != b.ordered {
        return false;
    }
    if a.ordered {
        a.delimiter == b.delimiter
    } else {
        a.bullet == b.bullet
    }
}

pub(super) fn strip_quote_prefix(line: &str) -> &str {
    let mut t = line;
    loop {
        let s = t.trim_start_matches(' ');
        if !s.is_empty() && s.as_bytes()[0] == b'>' {
            t = if s.len() > 1 && s.as_bytes()[1] == b' ' {
                &s[2..]
            } else {
                &s[1..]
            };
        } else {
            return t;
        }
    }
}
