//! Port of `src/markdown/edit.zig` — WP-3.3.
//!
//! Pure Markdown editing commands used by the editor component: formatting
//! toggles, line prefixes, list continuation on Enter and list indentation.
//! Every command returns the new text plus the selection to apply.
//!
//! Zig allocated the results in a caller arena; the Rust port returns owned
//! `String`s (the editor's callbacks own what they hand to the app).

use crate::markdown::scan;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub cursor: usize,
}

impl Selection {
    pub fn start(self) -> usize {
        self.anchor.min(self.cursor)
    }

    pub fn end(self) -> usize {
        self.anchor.max(self.cursor)
    }

    pub fn collapsed(at: usize) -> Selection {
        Selection {
            anchor: at,
            cursor: at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Result {
    pub text: String,
    pub selection: Selection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Bold,
    Italic,
    Strikethrough,
    InlineCode,
    Link,
    Heading,
    Quote,
    BulletList,
    OrderedList,
    TaskList,
    CodeBlock,
    InlineMath,
    MathBlock,
    Rule,
    Table,
}

impl Command {
    pub fn label(self) -> &'static str {
        match self {
            Command::Bold => "B",
            Command::Italic => "I",
            Command::Strikethrough => "S",
            Command::InlineCode => "`",
            Command::Link => "🔗",
            Command::Heading => "H",
            Command::Quote => "❝",
            Command::BulletList => "•",
            Command::OrderedList => "1.",
            Command::TaskList => "☑",
            Command::CodeBlock => "{}",
            Command::InlineMath => "∑",
            Command::MathBlock => "$$",
            Command::Rule => "—",
            Command::Table => "▦",
        }
    }
}

fn clamp_sel(text: &str, sel: Selection) -> Selection {
    Selection {
        anchor: sel.anchor.min(text.len()),
        cursor: sel.cursor.min(text.len()),
    }
}

fn splice(text: &str, start: usize, end: usize, insert: &str) -> String {
    let mut out = String::with_capacity(text.len() - (end - start) + insert.len());
    out.push_str(&text[..start]);
    out.push_str(insert);
    out.push_str(&text[end..]);
    out
}

/// Toggles `open`…`close` around the selection (or inserts an empty pair).
pub fn wrap(text: &str, sel_in: Selection, open: &str, close: &str) -> Result {
    let sel = clamp_sel(text, sel_in);
    let s = sel.start();
    let e = sel.end();
    // already wrapped outside the selection → unwrap
    if s >= open.len()
        && e + close.len() <= text.len()
        && &text[s - open.len()..s] == open
        && &text[e..e + close.len()] == close
    {
        let out = splice(text, s - open.len(), e + close.len(), &text[s..e]);
        return Result {
            text: out,
            selection: Selection {
                anchor: s - open.len(),
                cursor: e - open.len(),
            },
        };
    }
    // selection includes the markers → unwrap
    let inner = &text[s..e];
    if inner.len() >= open.len() + close.len() && inner.starts_with(open) && inner.ends_with(close)
    {
        let body = &inner[open.len()..inner.len() - close.len()];
        let out = splice(text, s, e, body);
        return Result {
            text: out,
            selection: Selection {
                anchor: s,
                cursor: s + body.len(),
            },
        };
    }
    let out = splice(text, s, e, &format!("{open}{inner}{close}"));
    Result {
        text: out,
        selection: Selection {
            anchor: s + open.len(),
            cursor: e + open.len(),
        },
    }
}

fn line_start(text: &str, at: usize) -> usize {
    let mut i = at.min(text.len());
    while i > 0 && text.as_bytes()[i - 1] != b'\n' {
        i -= 1;
    }
    i
}

fn line_end(text: &str, at: usize) -> usize {
    let from = at.min(text.len());
    match text[from..].find('\n') {
        Some(index) => from + index,
        None => text.len(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Prefix {
    Quote,
    Bullet,
    Ordered,
    Task,
}

fn existing_prefix(line: &str, kind: Prefix) -> Option<usize> {
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
fn toggle_prefix(text: &str, sel_in: Selection, kind: Prefix) -> Result {
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
fn cycle_heading(text: &str, sel_in: Selection) -> Result {
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

fn insert_block(
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

pub fn apply(text: &str, sel: Selection, cmd: Command) -> Result {
    match cmd {
        Command::Bold => wrap(text, sel, "**", "**"),
        Command::Italic => wrap(text, sel, "*", "*"),
        Command::Strikethrough => wrap(text, sel, "~~", "~~"),
        Command::InlineCode => wrap(text, sel, "`", "`"),
        Command::InlineMath => wrap(text, sel, "$$", "$$"),
        Command::Link => {
            let c = clamp_sel(text, sel);
            let label = if c.end() > c.start() {
                &text[c.start()..c.end()]
            } else {
                "链接"
            };
            let mut out = String::with_capacity(text.len() + label.len() + 16);
            out.push_str(&text[..c.start()]);
            out.push('[');
            out.push_str(label);
            out.push_str("](https://)");
            out.push_str(&text[c.end()..]);
            let url_start = c.start() + 1 + label.len() + 2;
            Result {
                text: out,
                selection: Selection {
                    anchor: url_start,
                    cursor: url_start + "https://".len(),
                },
            }
        }
        Command::Heading => cycle_heading(text, sel),
        Command::Quote => toggle_prefix(text, sel, Prefix::Quote),
        Command::BulletList => toggle_prefix(text, sel, Prefix::Bullet),
        Command::OrderedList => toggle_prefix(text, sel, Prefix::Ordered),
        Command::TaskList => toggle_prefix(text, sel, Prefix::Task),
        Command::CodeBlock => insert_block(text, sel, "```\n", "\n```", "code"),
        Command::MathBlock => insert_block(text, sel, "$$\n", "\n$$", "E = mc^2"),
        Command::Rule => insert_block(text, sel, "", "", "---"),
        Command::Table => insert_block(
            text,
            sel,
            "| 列 1 | 列 2 |\n| --- | --- |\n| ",
            " |  |",
            "内容",
        ),
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Zig `edit.zig`: `test "wrap toggles"`.
    #[test]
    fn wrap_toggles() {
        let r1 = apply(
            "hello world",
            Selection {
                anchor: 6,
                cursor: 11,
            },
            Command::Bold,
        );
        assert_eq!(r1.text, "hello **world**");
        assert_eq!(r1.selection.start(), 8);
        let r2 = apply(&r1.text, r1.selection, Command::Bold);
        assert_eq!(r2.text, "hello world");
    }

    /// Zig `edit.zig`: `test "list continuation"`.
    #[test]
    fn list_continuation() {
        let r = continue_list("1. one", Selection::collapsed(6)).expect("list item");
        assert_eq!(r.text, "1. one\n2. ");
        assert_eq!(r.selection.cursor, 10);
        let t = continue_list("- [x] done", Selection::collapsed(10)).expect("task item");
        assert_eq!(t.text, "- [x] done\n- [ ] ");
        let end = continue_list("- a\n- ", Selection::collapsed(6)).expect("empty item");
        assert_eq!(end.text, "- a\n");
        assert!(continue_list("plain", Selection::collapsed(5)).is_none());
    }

    /// Zig `edit.zig`: `test "line prefixes and headings"`.
    #[test]
    fn line_prefixes_and_headings() {
        let q = apply(
            "a\nb",
            Selection {
                anchor: 0,
                cursor: 3,
            },
            Command::Quote,
        );
        assert_eq!(q.text, "> a\n> b");
        let o = apply(
            "- a\n- b",
            Selection {
                anchor: 0,
                cursor: 7,
            },
            Command::OrderedList,
        );
        assert_eq!(o.text, "1. a\n2. b");
        let h1 = apply("Title", Selection::collapsed(2), Command::Heading);
        assert_eq!(h1.text, "# Title");
        let h2 = apply(&h1.text, h1.selection, Command::Heading);
        assert_eq!(h2.text, "## Title");
        let ind = indent_list("- a", Selection::collapsed(3), false).expect("list line");
        assert_eq!(ind.text, "  - a");
    }
}
