//! Port of `src/markdown/parser/inline.zig` — WP-3.2.
//!
//! Inline Markdown parser: emphasis, code spans, math, links, images,
//! autolinks, footnote references, inline HTML and character references.
//!
//! Emphasis follows CommonMark's flanking rules, relaxed the way Streamdown's
//! CJK plugin relaxes them: a closing run only needs a non-space character
//! before it, so `**中文。**后文` stays bold.

use std::borrow::Cow;
use std::collections::HashMap;

use super::super::ast::{Span, Style};
use super::super::entities;
use super::super::remend;
use super::super::scan;

#[derive(Clone)]
pub struct LinkDef {
    pub url: String,
    pub title: String,
}

pub type RefMap = HashMap<String, LinkDef>;

pub struct Context<'a> {
    pub refs: &'a RefMap,
    /// Footnote labels in order of first reference.
    pub footnote_order: &'a mut Vec<String>,
    /// Recognise `$x$` inline math. On by default with pandoc's rules so
    /// prices stay text: the opener must be followed by a non-space, the
    /// closer preceded by a non-space and not followed by a digit
    /// (`$20 and $10` is text, `$x$` is math).
    pub single_dollar_math: bool,
    /// `\(…\)` and `\[…\]` math (see `block::Options.latex_delimiters`).
    pub latex_delimiters: bool,
}

/// The parser's span builder. Zig keeps slices into the source document and
/// merges adjacent runs by pointer adjacency; the Rust port owns the final
/// strings but tracks the source offset of the last pushed run so the same
/// runs are merged.
struct Builder<'a, 'r> {
    ctx: &'a mut Context<'r>,
    source: &'a str,
    out: Vec<Span>,
    /// Source byte range of the last span when its text is a sub-slice of
    /// `source` (used for Zig's `last.text.ptr + last.text.len == text.ptr`).
    last_source: Option<(usize, usize)>,
}

impl<'a, 'r> Builder<'a, 'r> {
    fn source_range(&self, text: &str) -> Option<(usize, usize)> {
        let base = self.source.as_ptr() as usize;
        let start = text.as_ptr() as usize;
        if start >= base && start + text.len() <= base + self.source.len() {
            Some((start - base, start - base + text.len()))
        } else {
            None
        }
    }

    fn push(&mut self, text: &str, style: Style, url: &str) {
        if text.is_empty() {
            return;
        }
        let range = self.source_range(text);
        let adjacent = match (self.last_source, range) {
            (Some((last_start, last_end)), Some((start, _end))) if last_end == start => {
                Some((last_start, last_end))
            }
            _ => None,
        };
        if let Some(last) = self.out.last_mut()
            // merge adjacent runs with identical styling
            && last.style == style
            && last.url == url
            && !style.footnote_ref
            && let Some((last_start, last_end)) = adjacent
        {
            last.text.push_str(text);
            self.last_source = Some((last_start, last_end + text.len()));
            return;
        }
        self.out.push(Span {
            text: text.to_string(),
            style,
            url: url.to_string(),
        });
        self.last_source = range;
    }

    fn push_text(&mut self, text: &str, style: Style, url: &str) {
        if text.is_empty() {
            return;
        }
        if style.code || style.math {
            return self.push(text, style, url);
        }
        let decoded = entities::decode(text);
        if matches!(&decoded, Cow::Borrowed(_)) {
            return self.push(text, style, url);
        }
        self.out.push(Span {
            text: decoded.into_owned(),
            style,
            url: url.to_string(),
        });
        self.last_source = None;
    }
}

pub fn parse(ctx: &mut Context<'_>, text: &str) -> Vec<Span> {
    let mut b = Builder {
        ctx,
        source: text,
        out: Vec::new(),
        last_source: None,
    };
    parse_into(&mut b, text, Style::default(), "");
    b.out
}

/// Parses with a base style (used for table cells / headings with extra style).
pub fn parse_styled(ctx: &mut Context<'_>, text: &str, style: Style) -> Vec<Span> {
    let mut b = Builder {
        ctx,
        source: text,
        out: Vec::new(),
        last_source: None,
    };
    parse_into(&mut b, text, style, "");
    b.out
}

fn at(text: &str, i: usize) -> u8 {
    if i < text.len() {
        text.as_bytes()[i]
    } else {
        0
    }
}

fn run_length(text: &str, i: usize, c: u8) -> usize {
    let bytes = text.as_bytes();
    let mut n: usize = 0;
    while i + n < text.len() && bytes[i + n] == c {
        n += 1;
    }
    n
}

/// Zig `std.mem.indexOfScalarPos`: first `needle` at or after `from`.
fn index_of_scalar_pos(text: &str, from: usize, needle: u8) -> Option<usize> {
    if from >= text.len() {
        return None;
    }
    text.as_bytes()[from..]
        .iter()
        .position(|c| *c == needle)
        .map(|p| p + from)
}

/// Zig `std.mem.indexOfPos(u8, text, from, needle)`.
fn index_of_pos(text: &str, from: usize, needle: &str) -> Option<usize> {
    let from = from.min(text.len());
    let hay = &text.as_bytes()[from..];
    hay.windows(needle.len())
        .position(|w| w == needle.as_bytes())
        .map(|p| p + from)
}

fn trim_space_newline(text: &str) -> &str {
    text.trim_matches([' ', '\n'])
}

/// End of a code span opened by a run of `n` backticks at `start`, or `None`.
fn code_span_end(text: &str, start: usize, n: usize) -> Option<usize> {
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
fn math_text_end(text: &str, start: usize, n: usize) -> Option<usize> {
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
fn single_dollar_end(text: &str, start: usize) -> Option<usize> {
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

fn left_flanking(text: &str, start: usize, len: usize) -> bool {
    let next = scan::cp_at(text, start + len);
    next != 0 && !scan::is_whitespace_cp(next)
}

fn right_flanking(text: &str, start: usize) -> bool {
    let prev = scan::cp_before(text, start);
    prev != 0 && !scan::is_whitespace_cp(prev)
}

fn underscore_can_open(text: &str, start: usize, len: usize) -> bool {
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
fn find_closer(text: &str, from: usize, c: u8, need: usize) -> Option<usize> {
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

fn is_ascii_punct(c: u8) -> bool {
    scan::is_ascii_punct(c)
}

/// Balanced `[...]` starting at `open` (`text[open] == '['`). Returns index of `]`.
fn match_bracket(text: &str, open: usize) -> Option<usize> {
    let mut depth: usize = 0;
    let mut i = open;
    while i < text.len() {
        match text.as_bytes()[i] {
            b'\\' => i += 2,
            b'`' => {
                let n = run_length(text, i, b'`');
                i = if let Some(end) = code_span_end(text, i, n) {
                    end + n
                } else {
                    i + n
                };
            }
            b'[' => {
                depth += 1;
                i += 1;
            }
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

struct Destination<'a> {
    url: &'a str,
    title: &'a str,
    end: usize,
}

/// Parses `(url "title")` starting at `open` (`text[open] == '('`).
fn parse_destination(text: &str, open: usize) -> Option<Destination<'_>> {
    let bytes = text.as_bytes();
    let mut i = open + 1;
    while i < text.len() && (bytes[i] == b' ' || bytes[i] == b'\n') {
        i += 1;
    }
    let url_start: usize;
    let url_end: usize;
    if at(text, i) == b'<' {
        let close = index_of_scalar_pos(text, i + 1, b'>')?;
        url_start = i + 1;
        url_end = close;
        i = close + 1;
    } else {
        url_start = i;
        let mut depth: usize = 0;
        loop {
            if i >= text.len() {
                break;
            }
            let c = bytes[i];
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == b'(' {
                depth += 1;
            }
            if c == b')' {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            if c == b' ' || c == b'\n' {
                break;
            }
            i += 1;
        }
        url_end = i.min(text.len());
    }
    while i < text.len() && (bytes[i] == b' ' || bytes[i] == b'\n') {
        i += 1;
    }
    let mut title: &str = "";
    if i < text.len() && (bytes[i] == b'"' || bytes[i] == b'\'' || bytes[i] == b'(') {
        let close_char: u8 = if bytes[i] == b'(' { b')' } else { bytes[i] };
        let close = index_of_scalar_pos(text, i + 1, close_char)?;
        title = &text[i + 1..close];
        i = close + 1;
        while i < text.len() && (bytes[i] == b' ' || bytes[i] == b'\n') {
            i += 1;
        }
    }
    if at(text, i) != b')' {
        return None;
    }
    Some(Destination {
        url: &text[url_start..url_end],
        title,
        end: i + 1,
    })
}

fn normalize_label(label: &str) -> String {
    let mut out: Vec<u8> = Vec::new();
    let mut space = false;
    for c in label.trim_matches([' ', '\t', '\n']).bytes() {
        if c == b' ' || c == b'\t' || c == b'\n' {
            space = true;
            continue;
        }
        if space && !out.is_empty() {
            out.push(b' ');
        }
        space = false;
        out.push(c.to_ascii_lowercase());
    }
    String::from_utf8(out).expect("labels stay valid UTF-8")
}

pub fn lookup_ref(ctx: &Context<'_>, label: &str) -> Option<LinkDef> {
    if ctx.refs.is_empty() {
        return None;
    }
    let key = normalize_label(label);
    ctx.refs.get(&key).cloned()
}

pub fn normalize_ref_label(label: &str) -> String {
    normalize_label(label)
}

/// Bare autolink (GFM): http://, https://, www., or an email address.
fn bare_autolink_end(text: &str, i: usize) -> Option<usize> {
    let rest = &text[i..];
    let is_url =
        rest.starts_with("http://") || rest.starts_with("https://") || rest.starts_with("www.");
    if !is_url {
        return None;
    }
    if i > 0 && scan::is_word_cp(scan::cp_before(text, i)) {
        return None;
    }
    let mut end = i;
    let mut parens: isize = 0;
    while end < text.len() {
        let c = text.as_bytes()[end];
        if c <= b' ' || c == b'<' || c == b'>' || c == b'"' || c == b'`' {
            break;
        }
        // CJK punctuation ends an autolink (Streamdown CJK plugin)
        if c >= 0x80 {
            let cp = scan::cp_at(text, end);
            if scan::is_unicode_punct(cp) || scan::is_cjk(cp) {
                break;
            }
            end += scan::utf8_byte_sequence_length_or_1(c);
            continue;
        }
        if c == b'(' {
            parens += 1;
        }
        if c == b')' {
            if parens == 0 {
                break;
            }
            parens -= 1;
        }
        end += 1;
    }
    while end > i && b".,:;!?*_~'".contains(&text.as_bytes()[end - 1]) {
        end -= 1;
    }
    let min_len: usize = if rest.starts_with("www.") { 5 } else { 9 };
    if end - i < min_len {
        return None;
    }
    Some(end)
}

fn email_autolink(text: &str, at_index: usize) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut start = at_index;
    while start > 0 {
        let c = bytes[start - 1];
        if c.is_ascii_alphanumeric() || c == b'.' || c == b'_' || c == b'-' || c == b'+' {
            start -= 1;
        } else {
            break;
        }
    }
    if start == at_index {
        return None;
    }
    let mut end = at_index + 1;
    let mut dot = false;
    while end < text.len() {
        let c = bytes[end];
        if c == b'.' {
            dot = true;
        } else if !(c.is_ascii_alphanumeric() || c == b'-' || c == b'_') {
            break;
        }
        end += 1;
    }
    while end > at_index + 1 && bytes[end - 1] == b'.' {
        end -= 1;
    }
    if !dot || end <= at_index + 1 || bytes[end - 1] == b'-' {
        return None;
    }
    Some((start, end))
}

struct TagStyle {
    name: &'static str,
    apply: fn(&mut Style),
}

fn set_bold(s: &mut Style) {
    s.bold = true;
}
fn set_italic(s: &mut Style) {
    s.italic = true;
}
fn set_strike(s: &mut Style) {
    s.strike = true;
}
fn set_underline(s: &mut Style) {
    s.underline = true;
}
fn set_code(s: &mut Style) {
    s.code = true;
}
fn set_mark(s: &mut Style) {
    s.highlight = true;
}
fn set_sup(s: &mut Style) {
    s.superscript = true;
}
fn set_sub(s: &mut Style) {
    s.subscript = true;
}
fn set_nothing(_: &mut Style) {}

const INLINE_TAGS: [TagStyle; 20] = [
    TagStyle {
        name: "b",
        apply: set_bold,
    },
    TagStyle {
        name: "strong",
        apply: set_bold,
    },
    TagStyle {
        name: "i",
        apply: set_italic,
    },
    TagStyle {
        name: "em",
        apply: set_italic,
    },
    TagStyle {
        name: "s",
        apply: set_strike,
    },
    TagStyle {
        name: "del",
        apply: set_strike,
    },
    TagStyle {
        name: "strike",
        apply: set_strike,
    },
    TagStyle {
        name: "u",
        apply: set_underline,
    },
    TagStyle {
        name: "ins",
        apply: set_underline,
    },
    TagStyle {
        name: "code",
        apply: set_code,
    },
    TagStyle {
        name: "kbd",
        apply: set_code,
    },
    TagStyle {
        name: "mark",
        apply: set_mark,
    },
    TagStyle {
        name: "sup",
        apply: set_sup,
    },
    TagStyle {
        name: "sub",
        apply: set_sub,
    },
    TagStyle {
        name: "span",
        apply: set_nothing,
    },
    TagStyle {
        name: "small",
        apply: set_nothing,
    },
    TagStyle {
        name: "abbr",
        apply: set_nothing,
    },
    TagStyle {
        name: "cite",
        apply: set_nothing,
    },
    TagStyle {
        name: "q",
        apply: set_nothing,
    },
    TagStyle {
        name: "a",
        apply: set_nothing,
    },
];

struct Tag<'a> {
    name: &'a str,
    closing: bool,
    self_closing: bool,
    end: usize,
    href: &'a str,
}

fn parse_tag(text: &str, i: usize) -> Option<Tag<'_>> {
    if at(text, i) != b'<' {
        return None;
    }
    let bytes = text.as_bytes();
    let mut j = i + 1;
    let closing = at(text, j) == b'/';
    if closing {
        j += 1;
    }
    let name_start = j;
    while j < text.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
        j += 1;
    }
    if j == name_start || !bytes[name_start].is_ascii_alphabetic() {
        return None;
    }
    let name = &text[name_start..j];
    let close = index_of_scalar_pos(text, j, b'>')?;
    let attrs = &text[j..close];
    if attrs.as_bytes().contains(&b'<') || attrs.as_bytes().contains(&b'\n') {
        return None;
    }
    let mut href: &str = "";
    if let Some(h) = attrs.find("href=\"") {
        let s = h + 6;
        if let Some(e) = index_of_scalar_pos(attrs, s, b'"') {
            href = &attrs[s..e];
        }
    }
    Some(Tag {
        name,
        closing,
        self_closing: !attrs.is_empty() && attrs.as_bytes()[attrs.len() - 1] == b'/',
        end: close + 1,
        href,
    })
}

fn find_closing_tag(text: &str, from: usize, name: &str) -> Option<(usize, usize)> {
    let mut i = from;
    let mut depth: usize = 0;
    while let Some(lt) = index_of_scalar_pos(text, i, b'<') {
        if let Some(tag) = parse_tag(text, lt) {
            if tag.name.eq_ignore_ascii_case(name) {
                if tag.closing {
                    if depth == 0 {
                        return Some((lt, tag.end));
                    }
                    depth -= 1;
                } else if !tag.self_closing {
                    depth += 1;
                }
            }
            i = tag.end;
        } else {
            i = lt + 1;
        }
    }
    None
}

fn parse_into(b: &mut Builder<'_, '_>, text: &str, base: Style, url: &str) {
    let bytes = text.as_bytes();
    let mut i: usize = 0;
    let mut plain: usize = 0;
    while i < text.len() {
        let c = bytes[i];
        match c {
            b'\\' => {
                if b.ctx.latex_delimiters
                    && i + 1 < text.len()
                    && (bytes[i + 1] == b'(' || bytes[i + 1] == b'[')
                {
                    let close: &str = if bytes[i + 1] == b'(' { "\\)" } else { "\\]" };
                    if let Some(end) = index_of_pos(text, i + 2, close) {
                        let inner = trim_space_newline(&text[i + 2..end]);
                        if !inner.is_empty() {
                            b.push_text(&text[plain..i], base, url);
                            let mut style = base;
                            style.math = true;
                            let collapsed = collapse_newlines(inner);
                            b.push(&collapsed, style, url);
                            i = end + 2;
                            plain = i;
                            continue;
                        }
                    }
                }
                if i + 1 < text.len() && is_ascii_punct(bytes[i + 1]) {
                    b.push_text(&text[plain..i], base, url);
                    b.push(&text[i + 1..i + 2], base, url);
                    i += 2;
                    plain = i;
                    continue;
                }
                if i + 1 < text.len() && bytes[i + 1] == b'\n' {
                    // hard break
                    b.push_text(&text[plain..i], base, url);
                    b.push(&text[i + 1..i + 2], base, url);
                    i += 2;
                    plain = i;
                    continue;
                }
            }
            b'`' => {
                let n = run_length(text, i, b'`');
                if let Some(end) = code_span_end(text, i, n) {
                    b.push_text(&text[plain..i], base, url);
                    let mut body = &text[i + n..end];
                    if body.len() >= 2
                        && body.as_bytes()[0] == b' '
                        && body.as_bytes()[body.len() - 1] == b' '
                        && !body.trim_matches(' ').is_empty()
                    {
                        body = &body[1..body.len() - 1];
                    }
                    let mut style = base;
                    style.code = true;
                    let collapsed = collapse_newlines(body);
                    b.push(&collapsed, style, url);
                    i = end + n;
                    plain = i;
                    continue;
                }
                i += n;
                continue;
            }
            b'$' => {
                // micromark-extension-math (mathText): a run of N dollars
                // closes on a run of exactly N; single `$` only when enabled.
                let n = run_length(text, i, b'$');
                if n == 1
                    && b.ctx.single_dollar_math
                    && let Some(end) = single_dollar_end(text, i)
                {
                    b.push_text(&text[plain..i], base, url);
                    let mut style = base;
                    style.math = true;
                    let collapsed = collapse_newlines(&text[i + 1..end]);
                    b.push(&collapsed, style, url);
                    i = end + 1;
                    plain = i;
                    continue;
                }
                if n >= 2
                    && let Some(end) = math_text_end(text, i, n)
                {
                    b.push_text(&text[plain..i], base, url);
                    let mut body = &text[i + n..end];
                    if body.len() >= 2
                        && (body.as_bytes()[0] == b' ' || body.as_bytes()[0] == b'\n')
                        && (body.as_bytes()[body.len() - 1] == b' '
                            || body.as_bytes()[body.len() - 1] == b'\n')
                        && !trim_space_newline(body).is_empty()
                    {
                        body = &body[1..body.len() - 1];
                    }
                    let mut style = base;
                    style.math = true;
                    let collapsed = collapse_newlines(body);
                    b.push(&collapsed, style, url);
                    i = end + n;
                    plain = i;
                    continue;
                }
                i += n;
                continue;
            }
            b'*' | b'_' => {
                let n = run_length(text, i, c);
                let can_open = if c == b'_' {
                    underscore_can_open(text, i, n)
                } else {
                    left_flanking(text, i, n)
                };
                if can_open {
                    let use_ = n.min(3);
                    if let Some(close) = find_closer(text, i + use_, c, use_) {
                        b.push_text(&text[plain..i], base, url);
                        // emit any extra opening markers literally (****x)
                        if n > use_ {
                            b.push(&text[i..i + n - use_], base, url);
                        }
                        let mut style = base;
                        if use_ >= 2 {
                            style.bold = true;
                        }
                        if use_ != 2 {
                            style.italic = true;
                        }
                        parse_into(b, &text[i + n..close], style, url);
                        i = close + use_;
                        plain = i;
                        continue;
                    }
                    // fall back to shorter runs: **a* → *a* with literal '*'
                    if use_ > 1
                        && let Some(close) = find_closer(text, i + n, c, 1)
                    {
                        let close_len = run_length(text, close, c);
                        let k = close_len.min(use_);
                        b.push_text(&text[plain..i], base, url);
                        b.push(&text[i..i + n - k], base, url);
                        let mut style = base;
                        if k >= 2 {
                            style.bold = true;
                        }
                        if k != 2 {
                            style.italic = true;
                        }
                        parse_into(b, &text[i + n..close], style, url);
                        i = close + k;
                        plain = i;
                        continue;
                    }
                }
                i += n;
                continue;
            }
            b'~' => {
                let n = run_length(text, i, b'~');
                if (n == 1 || n == 2) && left_flanking(text, i, n) {
                    let mut matched = false;
                    let mut search = i + n;
                    while let Some(close) = index_of_scalar_pos(text, search, b'~') {
                        let m = run_length(text, close, b'~');
                        if m == n && right_flanking(text, close) && bytes[close - 1] != b'\\' {
                            b.push_text(&text[plain..i], base, url);
                            let mut style = base;
                            style.strike = true;
                            parse_into(b, &text[i + n..close], style, url);
                            i = close + n;
                            plain = i;
                            matched = true;
                            break;
                        }
                        search = close + m;
                    }
                    if matched {
                        continue;
                    }
                }
                i += n;
                continue;
            }
            b'=' => {
                if at(text, i + 1) == b'='
                    && left_flanking(text, i, 2)
                    && let Some(close) = index_of_pos(text, i + 2, "==")
                    && close > i + 2
                    && right_flanking(text, close)
                {
                    b.push_text(&text[plain..i], base, url);
                    let mut style = base;
                    style.highlight = true;
                    parse_into(b, &text[i + 2..close], style, url);
                    i = close + 2;
                    plain = i;
                    continue;
                }
            }
            b'!' | b'[' => {
                let is_image = c == b'!';
                let open = if is_image { i + 1 } else { i };
                if at(text, open) == b'['
                    && let Some(next) =
                        parse_link_like(b, text, i, open, is_image, base, url, plain)
                {
                    i = next;
                    plain = i;
                    continue;
                }
            }
            b'<' => {
                if let Some(close) = index_of_scalar_pos(text, i + 1, b'>') {
                    let inner = &text[i + 1..close];
                    let is_uri = (inner.contains("://") || inner.starts_with("mailto:"))
                        && !inner
                            .as_bytes()
                            .iter()
                            .any(|b| *b == b' ' || *b == b'<' || *b == b'\n');
                    let is_email = !is_uri
                        && inner.as_bytes().contains(&b'@')
                        && !inner
                            .as_bytes()
                            .iter()
                            .any(|b| *b == b' ' || *b == b'<' || *b == b'\n' || *b == b'/');
                    if is_uri || is_email {
                        b.push_text(&text[plain..i], base, url);
                        let mut style = base;
                        style.link = true;
                        let target = if is_email {
                            format!("mailto:{inner}")
                        } else {
                            inner.to_string()
                        };
                        b.push(inner, style, &target);
                        i = close + 1;
                        plain = i;
                        continue;
                    }
                }
                if let Some(tag) = parse_tag(text, i) {
                    if tag.name.eq_ignore_ascii_case("br") {
                        b.push_text(&text[plain..i], base, url);
                        b.push("\n", base, url);
                        i = tag.end;
                        plain = i;
                        continue;
                    }
                    if !tag.closing && !tag.self_closing {
                        let mut matched = false;
                        for known in INLINE_TAGS.iter() {
                            if !known.name.eq_ignore_ascii_case(tag.name) {
                                continue;
                            }
                            if let Some(close) = find_closing_tag(text, tag.end, tag.name) {
                                b.push_text(&text[plain..i], base, url);
                                let mut style = base;
                                (known.apply)(&mut style);
                                let a_link =
                                    tag.name.eq_ignore_ascii_case("a") && !tag.href.is_empty();
                                if a_link {
                                    style.link = true;
                                }
                                let inner_url = if a_link { tag.href } else { url };
                                if style.code {
                                    b.push(&text[tag.end..close.0], style, inner_url);
                                } else {
                                    parse_into(b, &text[tag.end..close.0], style, inner_url);
                                }
                                i = close.1;
                                plain = i;
                                matched = true;
                                break;
                            }
                        }
                        if matched {
                            continue;
                        }
                    }
                    // unknown or unbalanced tag: drop the tag, keep content
                    b.push_text(&text[plain..i], base, url);
                    i = tag.end;
                    plain = i;
                    continue;
                }
                if text[i..].starts_with("<!--")
                    && let Some(end) = index_of_pos(text, i + 4, "-->")
                {
                    b.push_text(&text[plain..i], base, url);
                    i = end + 3;
                    plain = i;
                    continue;
                }
            }
            b'h' | b'w' => {
                if !base.link
                    && let Some(end) = bare_autolink_end(text, i)
                {
                    b.push_text(&text[plain..i], base, url);
                    let mut style = base;
                    style.link = true;
                    let target = if bytes[i] == b'w' {
                        format!("http://{}", &text[i..end])
                    } else {
                        text[i..end].to_string()
                    };
                    b.push(&text[i..end], style, &target);
                    i = end;
                    plain = i;
                    continue;
                }
            }
            b'@' => {
                if !base.link
                    && let Some((start, end)) = email_autolink(text, i)
                    && start >= plain
                {
                    b.push_text(&text[plain..start], base, url);
                    let mut style = base;
                    style.link = true;
                    let addr = &text[start..end];
                    let target = format!("mailto:{addr}");
                    b.push(addr, style, &target);
                    i = end;
                    plain = i;
                    continue;
                }
            }
            b'\n' => {
                // soft break: drop trailing spaces; between CJK characters no space
                let mut line_end = i;
                while line_end > plain && bytes[line_end - 1] == b' ' {
                    line_end -= 1;
                }
                let hard = i - line_end >= 2;
                b.push_text(&text[plain..line_end], base, url);
                let prev = scan::cp_before(text, line_end);
                let mut next_start = i + 1;
                while next_start < text.len() && bytes[next_start] == b' ' {
                    next_start += 1;
                }
                let next = scan::cp_at(text, next_start);
                if hard {
                    b.push("\n", base, url);
                } else if !(scan::is_cjk(prev) && scan::is_cjk(next)) {
                    b.push(" ", base, url);
                }
                i = next_start;
                plain = i;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    b.push_text(&text[plain..], base, url);
}

fn collapse_newlines(text: &str) -> Cow<'_, str> {
    if !text.as_bytes().contains(&b'\n') {
        return Cow::Borrowed(text);
    }
    Cow::Owned(text.replace('\n', " "))
}

/// `[text](url)`, `[text][ref]`, `[ref][]`, `[ref]`, `[^note]`, images.
/// Returns the index after the construct, or `None` when it is not a link.
#[allow(clippy::too_many_arguments)] // same parameter list as inline.zig
fn parse_link_like(
    b: &mut Builder<'_, '_>,
    text: &str,
    start: usize,
    open: usize,
    is_image: bool,
    base: Style,
    url: &str,
    plain: usize,
) -> Option<usize> {
    let close = match_bracket(text, open)?;
    let label = &text[open + 1..close];

    // footnote reference [^label]
    if !is_image
        && label.len() > 1
        && label.as_bytes()[0] == b'^'
        && at(text, close + 1) != b':'
        && !label
            .as_bytes()
            .iter()
            .any(|c| *c == b' ' || *c == b'\t' || *c == b'\n')
    {
        b.push_text(&text[plain..start], base, url);
        let name = &label[1..];
        let mut number: usize = 0;
        for (idx, existing) in b.ctx.footnote_order.iter().enumerate() {
            if existing == name {
                number = idx + 1;
            }
        }
        if number == 0 {
            b.ctx.footnote_order.push(name.to_string());
            number = b.ctx.footnote_order.len();
        }
        let mut style = base;
        style.footnote_ref = true;
        style.superscript = true;
        let number_text = number.to_string();
        b.push(&number_text, style, name);
        return Some(close + 1);
    }

    let mut target: Option<LinkDef> = None;
    let mut end = close + 1;
    if at(text, close + 1) == b'('
        && let Some(dest) = parse_destination(text, close + 1)
    {
        target = Some(LinkDef {
            url: dest.url.to_string(),
            title: dest.title.to_string(),
        });
        end = dest.end;
    }
    if target.is_none()
        && at(text, close + 1) == b'['
        && let Some(ref_close) = index_of_scalar_pos(text, close + 2, b']')
    {
        let reference = &text[close + 2..ref_close];
        target = lookup_ref(
            b.ctx,
            if reference.is_empty() {
                label
            } else {
                reference
            },
        );
        if target.is_some() {
            end = ref_close + 1;
        }
    }
    if target.is_none() {
        target = lookup_ref(b.ctx, label);
        end = close + 1;
    }
    let t = target?;

    b.push_text(&text[plain..start], base, url);
    let mut style = base;
    if is_image {
        style.image = true;
        // alt text rendered plainly; renderer decides to load the image
        let alt = if !label.is_empty() { label } else { "image" };
        b.push(alt, style, &t.url);
    } else {
        style.link = true;
        let target_url = if t.url == remend::INCOMPLETE_LINK {
            "#incomplete".to_string()
        } else {
            t.url.clone()
        };
        parse_into(b, label, style, &target_url);
    }
    Some(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context<'a>(refs: &'a RefMap, order: &'a mut Vec<String>) -> Context<'a> {
        Context {
            refs,
            footnote_order: order,
            single_dollar_math: true,
            latex_delimiters: false,
        }
    }

    #[test]
    fn emphasis_and_code() {
        let refs = RefMap::new();
        let mut order: Vec<String> = Vec::new();
        let mut ctx = test_context(&refs, &mut order);
        let spans = parse(&mut ctx, "a **b *c*** `d` snake_case_word ~~e~~");
        let mut bold_italic = false;
        let mut code = false;
        let mut strike = false;
        for s in &spans {
            if s.style.bold && s.style.italic && s.text == "c" {
                bold_italic = true;
            }
            if s.style.code {
                code = true;
            }
            if s.style.strike {
                strike = true;
            }
            assert!(!(s.style.italic && s.text == "case"));
        }
        assert!(bold_italic && code && strike);
    }

    #[test]
    fn cjk_emphasis_closes_before_ideographic_text() {
        let refs = RefMap::new();
        let mut order: Vec<String> = Vec::new();
        let mut ctx = test_context(&refs, &mut order);
        let spans = parse(&mut ctx, "**中文。**后文");
        assert!(spans[0].style.bold);
        assert_eq!(spans[0].text, "中文。");
    }

    #[test]
    fn links_and_autolinks() {
        let refs = RefMap::new();
        let mut order: Vec<String> = Vec::new();
        let mut ctx = test_context(&refs, &mut order);
        let spans = parse(
            &mut ctx,
            "[a](http://x.com \"t\") and https://y.com/p, mail me@ex.com",
        );
        let urls = spans.iter().filter(|s| s.style.link).count();
        assert_eq!(urls, 3);
        assert_eq!(spans[0].url, "http://x.com");
    }
}
